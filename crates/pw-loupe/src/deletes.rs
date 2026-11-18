//! Deleting images to the Recycle Bin (ADR 0018 §2.1, §2.5, §2.7, step B3; T187): the
//! confirmation's content from the worker's `DeletePrepared` answer, one batch of deletes sent
//! behind queued rating writes and gathered when every answer has come, and where a delete is.
//! Plain Rust without I/O (GR11); the metadata worker deletes the files (T185, T186), the shell
//! connects it (T188, T189). The texts are in `deletes_text.rs`.

use std::path::{Path, PathBuf};

use pw_metadata::{DeleteError, DeleteRefusal, PreparedDelete};

use crate::deletes_text::partner_not_rejected_text;

/// Whether `a` and `b` are the same path without regard to case: the same number of components,
/// each pair equal after `to_string_lossy().to_lowercase()` (Unicode, so å and Å match). The same
/// on every platform and without I/O.
pub fn same_path(a: &Path, b: &Path) -> bool {
    let mut a = a.components();
    let mut b = b.components();
    loop {
        match (a.next(), b.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) => {
                if x.as_os_str().to_string_lossy().to_lowercase()
                    != y.as_os_str().to_string_lossy().to_lowercase()
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

fn contains_path(paths: &[PathBuf], path: &Path) -> bool {
    paths.iter().any(|known| same_path(known, path))
}

/// What the confirmation shows and the batch sends (ADR 0018 §2.5): the images to delete, the
/// number of distinct files of their groups and their total size, the image files of the same
/// name that go along without being marked (B-Q18), and the images that cannot be deleted with
/// the reason as text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletePlan {
    pub images: Vec<PathBuf>,
    pub files: usize,
    pub bytes: u64,
    pub partners: Vec<PathBuf>,
    pub refused: Vec<(PathBuf, String)>,
}

/// The plan of the worker's answers, in their order. An image that is a file of an earlier kept
/// group (the JPG of a marked RAW+JPEG pair) is left out entirely, also when its own answer is an
/// error: it goes with that group. `is_image` tells an image file (one of the image types) from a
/// companion file; `partners` are the image files of the kept groups that were not sent as
/// images themselves (owner, 2026-11-18: a marked JPG left out is not a partner), each once.
pub fn plan_delete(prepared: Vec<PreparedDelete>, is_image: impl Fn(&Path) -> bool) -> DeletePlan {
    let sent: Vec<PathBuf> = prepared.iter().map(|answer| answer.image.clone()).collect();
    let mut plan = DeletePlan {
        images: Vec::new(),
        files: 0,
        bytes: 0,
        partners: Vec::new(),
        refused: Vec::new(),
    };
    let mut kept_files: Vec<PathBuf> = Vec::new();
    for answer in prepared {
        if contains_path(&kept_files, &answer.image) {
            continue;
        }
        match answer.result {
            Ok(group) => {
                plan.images.push(answer.image);
                plan.bytes = plan.bytes.saturating_add(group.bytes);
                for file in group.files {
                    if !contains_path(&kept_files, &file) {
                        kept_files.push(file);
                    }
                }
            }
            Err(error) => plan.refused.push((answer.image, error.reason.to_string())),
        }
    }
    plan.files = kept_files.len();
    for file in kept_files {
        if is_image(&file)
            && !contains_path(&plan.images, &file)
            && !contains_path(&sent, &file)
            && !contains_path(&plan.partners, &file)
        {
            plan.partners.push(file);
        }
    }
    plan
}

/// The targets of "Delete Rejected Images" (ADR 0018 §2.1, B-Q9, T190): the files for which
/// `is_rejected` is true, in the list's order. The shell gives the active tab's whole list, so a
/// filter does not hide a rejected image from the command.
pub fn rejected_images(files: &[PathBuf], is_rejected: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    files
        .iter()
        .filter(|file| is_rejected(file))
        .cloned()
        .collect()
}

/// The image files that go along with an image in its group (ADR 0018 B-Q3, B-Q16, T190 point
/// 6): the files of `files` after the first, which is the image itself (`DeleteGroup`'s order),
/// for which `is_image` is true; companion files such as sidecars are not.
pub fn image_partners(files: &[PathBuf], is_image: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    files
        .iter()
        .skip(1)
        .filter(|file| is_image(file))
        .cloned()
        .collect()
}

/// A rejected image whose RAW+JPEG partner is not rejected is refused (ADR 0018 B-Q16, T190): an
/// `Ok` group with another image file (a file of `group.files` after the first, which is the image
/// itself, for which `is_image` is true) that `is_rejected` says is not rejected becomes an error
/// with the reason `partner_not_rejected_text`, so `plan_delete` lists it under `refused`.
/// Everything else is kept as it is, in its order.
pub fn refuse_unrejected_partners(
    prepared: Vec<PreparedDelete>,
    is_rejected: impl Fn(&Path) -> bool,
    is_image: impl Fn(&Path) -> bool,
) -> Vec<PreparedDelete> {
    prepared
        .into_iter()
        .map(|answer| {
            let refuse = match &answer.result {
                Ok(group) => image_partners(&group.files, &is_image)
                    .iter()
                    .any(|file| !is_rejected(file)),
                Err(_) => false,
            };
            if refuse {
                PreparedDelete {
                    result: Err(DeleteError {
                        file: answer.image.clone(),
                        reason: DeleteRefusal::Failed(partner_not_rejected_text().to_owned()),
                        left: Vec::new(),
                        deleted: Vec::new(),
                    }),
                    image: answer.image,
                }
            } else {
                answer
            }
        })
        .collect()
}

/// What a finished batch of deletes gives: the images deleted, every file that went to the
/// Recycle Bin (also those of a group deleted only in part), and the images that failed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeleteResult {
    pub images: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, DeleteError)>,
}

#[derive(Clone, Debug)]
struct DeleteEntry {
    image: PathBuf,
    sent: bool,
    /// Not sent and needs no answer (`skip`, T190).
    skipped: bool,
    answer: Option<Result<Vec<PathBuf>, DeleteError>>,
}

/// The deletes of one confirmation (ADR 0018 §2.3, B-Q10), as `Batch` is for the moves: each
/// image is deleted on its own, sent once no rating write of its path waits. Images are compared
/// as given (they come from the plan).
#[derive(Clone, Debug)]
pub struct DeleteBatch {
    images: Vec<DeleteEntry>,
}

impl DeleteBatch {
    /// A batch of `images`, in the order they are sent.
    pub fn new(images: Vec<PathBuf>) -> DeleteBatch {
        DeleteBatch {
            images: images
                .into_iter()
                .map(|image| DeleteEntry {
                    image,
                    sent: false,
                    skipped: false,
                    answer: None,
                })
                .collect(),
        }
    }

    /// Whether `image` is one of the batch's images and is not skipped.
    pub fn contains(&self, image: &Path) -> bool {
        self.images
            .iter()
            .any(|entry| !entry.skipped && entry.image == image)
    }

    /// The images not yet sent, skipped or answered whose path has no rating write waiting
    /// (`is_waiting`; the shell passes the metadata cache's `is_pending`), in the batch's order.
    /// Marks nothing: `sent` does.
    pub fn to_send(&self, is_waiting: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
        self.images
            .iter()
            .filter(|entry| {
                !entry.sent && !entry.skipped && entry.answer.is_none() && !is_waiting(&entry.image)
            })
            .map(|entry| entry.image.clone())
            .collect()
    }

    /// Marks `image` as sent once the worker took its delete. An image not in the batch changes
    /// nothing.
    pub fn sent(&mut self, image: &Path) {
        if let Some(entry) = self
            .images
            .iter_mut()
            .find(|entry| !entry.sent && !entry.skipped && entry.image == image)
        {
            entry.sent = true;
        }
    }

    /// Leaves `image` out: it is not sent and needs no answer, and `finish` lists it nowhere. An
    /// image not in the batch, or one already sent or answered, changes nothing.
    pub fn skip(&mut self, image: &Path) {
        if let Some(entry) = self.images.iter_mut().find(|entry| {
            !entry.sent && !entry.skipped && entry.answer.is_none() && entry.image == image
        }) {
            entry.skipped = true;
        }
    }

    /// Records the worker's answer for `image`: the files deleted, or why not. An image not in
    /// the batch, or one that has its answer already, changes nothing.
    pub fn answered(&mut self, image: &Path, result: Result<Vec<PathBuf>, DeleteError>) {
        if let Some(entry) = self
            .images
            .iter_mut()
            .find(|entry| !entry.skipped && entry.answer.is_none() && entry.image == image)
        {
            entry.sent = true;
            entry.answer = Some(result);
        }
    }

    /// Whether every image has its answer or was skipped.
    pub fn done(&self) -> bool {
        self.images
            .iter()
            .all(|entry| entry.skipped || entry.answer.is_some())
    }

    /// The batch's result, in the batch's order. A failed answer whose image is among another
    /// answer's files (the JPG its RAW took along) is not a failure and is dropped entirely; a
    /// failed group's `deleted` files are in `files`. An image without an answer yet, or skipped,
    /// is in no list.
    pub fn finish(self) -> DeleteResult {
        let gone: Vec<(usize, &[PathBuf])> = self
            .images
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| match &entry.answer {
                Some(Ok(files)) => Some((index, files.as_slice())),
                Some(Err(error)) => Some((index, error.deleted.as_slice())),
                None => None,
            })
            .collect();
        let taken_along: Vec<bool> = self
            .images
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                gone.iter()
                    .any(|(other, files)| *other != index && contains_path(files, &entry.image))
            })
            .collect();
        let mut result = DeleteResult::default();
        for (entry, taken) in self.images.into_iter().zip(taken_along) {
            match entry.answer {
                Some(Ok(files)) => {
                    result.images.push(entry.image);
                    result.files.extend(files);
                }
                Some(Err(_)) if taken => {}
                Some(Err(error)) => {
                    result.files.extend(error.deleted.iter().cloned());
                    result.failed.push((entry.image, error));
                }
                None => {}
            }
        }
        result
    }
}

/// Where a delete is (ADR 0018 §2.10); one delete at a time, T189 drives it. `Preparing` holds the
/// images until the worker took the request (`id` is `None` while its queue is full).
#[derive(Clone, Debug, Default)]
pub enum DeleteStep {
    #[default]
    Idle,
    Preparing {
        images: Vec<PathBuf>,
        id: Option<pw_metadata::RequestId>,
    },
    Confirming(DeletePlan),
    Running(DeleteBatch),
}

impl DeleteStep {
    /// Whether a delete is under way (anything but `Idle`).
    pub fn busy(&self) -> bool {
        !matches!(self, DeleteStep::Idle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_metadata::DeleteGroup;

    fn path(text: &str) -> PathBuf {
        PathBuf::from(text)
    }

    fn is_image(file: &Path) -> bool {
        file.extension()
            .map(|extension| extension.to_string_lossy().to_lowercase())
            .is_some_and(|extension| pw_raw::SUPPORTED_EXTENSIONS.contains(&extension.as_str()))
    }

    fn ok(image: &str, files: &[&str], bytes: u64) -> PreparedDelete {
        PreparedDelete {
            image: path(image),
            result: Ok(DeleteGroup {
                files: files.iter().map(|file| path(file)).collect(),
                bytes,
            }),
        }
    }

    fn error(file: &str, reason: DeleteRefusal) -> DeleteError {
        DeleteError {
            file: path(file),
            reason,
            left: Vec::new(),
            deleted: Vec::new(),
        }
    }

    fn refused(image: &str, reason: DeleteRefusal) -> PreparedDelete {
        PreparedDelete {
            image: path(image),
            result: Err(error(image, reason)),
        }
    }

    #[test]
    fn same_path_ignores_case_also_for_a_and_a_ring() {
        assert!(same_path(
            Path::new("D:/Foton/IMG_001.ARW"),
            Path::new("d:/foton/img_001.arw")
        ));
        assert!(same_path(
            Path::new("D:/Bilder/Å/a.ARW"),
            Path::new("D:/bilder/å/A.arw")
        ));
        assert!(!same_path(Path::new("D:/x/a.ARW"), Path::new("D:/x/b.ARW")));
        assert!(!same_path(
            Path::new("D:/x/a.ARW"),
            Path::new("D:/x/y/a.ARW")
        ));
        assert!(!same_path(Path::new("D:/x"), Path::new("D:/x/a.ARW")));
        assert!(same_path(Path::new(""), Path::new("")));
    }

    #[test]
    fn the_plan_of_two_images_counts_their_files_and_size() {
        let plan = plan_delete(
            vec![
                ok("D:/x/a.ARW", &["D:/x/a.ARW", "D:/x/a.xmp"], 30_000_000),
                ok("D:/x/b.ARW", &["D:/x/b.ARW"], 25_000_000),
            ],
            is_image,
        );
        assert_eq!(
            plan,
            DeletePlan {
                images: vec![path("D:/x/a.ARW"), path("D:/x/b.ARW")],
                files: 3,
                bytes: 55_000_000,
                partners: Vec::new(),
                refused: Vec::new(),
            }
        );
    }

    #[test]
    fn a_marked_raw_and_its_jpg_are_one_image_counted_once() {
        let group = ["D:/x/IMG_001.ARW", "D:/x/IMG_001.JPG", "D:/x/IMG_001.xmp"];
        let jpg_group = ["D:/x/IMG_001.JPG", "D:/x/IMG_001.ARW", "D:/x/IMG_001.xmp"];
        let plan = plan_delete(
            vec![
                ok("D:/x/IMG_001.ARW", &group, 40_000_000),
                // In other case, as the shell may hold it: still the same file.
                ok("D:/x/img_001.jpg", &jpg_group, 40_000_000),
            ],
            is_image,
        );
        assert_eq!(
            plan,
            DeletePlan {
                images: vec![path("D:/x/IMG_001.ARW")],
                files: 3,
                bytes: 40_000_000,
                partners: Vec::new(),
                refused: Vec::new(),
            }
        );
    }

    #[test]
    fn a_marked_jpg_left_out_is_left_out_also_when_its_own_answer_is_an_error() {
        let plan = plan_delete(
            vec![
                ok(
                    "D:/x/IMG_001.ARW",
                    &["D:/x/IMG_001.ARW", "D:/x/IMG_001.JPG"],
                    10,
                ),
                refused("D:/x/IMG_001.JPG", DeleteRefusal::OpenElsewhere),
            ],
            is_image,
        );
        assert_eq!(plan.images, vec![path("D:/x/IMG_001.ARW")]);
        assert!(plan.refused.is_empty());
        assert!(plan.partners.is_empty());
    }

    #[test]
    fn a_raw_alone_names_its_jpg_and_tiff_as_partners() {
        let plan = plan_delete(
            vec![ok(
                "D:/x/IMG_001.ARW",
                &[
                    "D:/x/IMG_001.ARW",
                    "D:/x/IMG_001.JPG",
                    "D:/x/IMG_001.ARW.xmp",
                    "D:/x/IMG_001.TIF",
                ],
                50,
            )],
            is_image,
        );
        assert_eq!(plan.images, vec![path("D:/x/IMG_001.ARW")]);
        assert_eq!(plan.files, 4);
        assert_eq!(plan.bytes, 50);
        assert_eq!(
            plan.partners,
            vec![path("D:/x/IMG_001.JPG"), path("D:/x/IMG_001.TIF")]
        );
    }

    #[test]
    fn a_refused_image_beside_a_good_one() {
        let plan = plan_delete(
            vec![
                refused("E:/DCIM/a.ARW", DeleteRefusal::Card),
                ok("D:/x/b.ARW", &["D:/x/b.ARW"], 7),
            ],
            is_image,
        );
        assert_eq!(plan.images, vec![path("D:/x/b.ARW")]);
        assert_eq!(plan.files, 1);
        assert_eq!(plan.bytes, 7);
        assert_eq!(
            plan.refused,
            vec![(
                path("E:/DCIM/a.ARW"),
                "the drive is a memory card".to_owned()
            )]
        );
    }

    #[test]
    fn only_refused_images_give_no_images() {
        let plan = plan_delete(
            vec![
                refused("E:/DCIM/a.ARW", DeleteRefusal::Card),
                refused("D:/x/b.ARW", DeleteRefusal::Missing),
            ],
            is_image,
        );
        assert!(plan.images.is_empty());
        assert_eq!(plan.files, 0);
        assert_eq!(plan.bytes, 0);
        assert_eq!(plan.refused.len(), 2);
    }

    #[test]
    fn to_send_waits_for_a_pending_write_and_never_gives_a_sent_image_again() {
        let a = path("D:/x/a.ARW");
        let b = path("D:/x/b.ARW");
        let mut batch = DeleteBatch::new(vec![a.clone(), b.clone()]);
        assert!(batch.contains(&a));
        assert!(!batch.contains(Path::new("D:/x/c.ARW")));

        let pending_a = |image: &Path| image == a;
        assert_eq!(batch.to_send(pending_a), vec![b.clone()]);
        // `to_send` marks nothing.
        assert_eq!(batch.to_send(pending_a), vec![b.clone()]);
        batch.sent(&b);
        assert!(batch.to_send(pending_a).is_empty());

        // The write was answered: a goes now, b never again.
        assert_eq!(batch.to_send(|_| false), vec![a.clone()]);
        batch.sent(&a);
        assert!(batch.to_send(|_| false).is_empty());
        assert!(!batch.done());
    }

    #[test]
    fn a_skipped_image_is_not_sent_and_needs_no_answer() {
        let a = path("D:/x/a.ARW");
        let b = path("D:/x/b.ARW");
        let mut batch = DeleteBatch::new(vec![a.clone(), b.clone()]);
        batch.skip(&a);
        assert!(!batch.contains(&a));
        assert_eq!(batch.to_send(|_| false), vec![b.clone()]);
        batch.answered(&a, Ok(vec![a.clone()]));
        batch.answered(&b, Ok(vec![b.clone()]));
        assert!(batch.done());
        let result = batch.finish();
        assert_eq!(result.images, vec![b.clone()]);
        assert_eq!(result.files, vec![b]);
    }

    #[test]
    fn finish_sorts_the_answers_and_drops_a_partner_deleted_by_another_image() {
        let raw = path("D:/x/IMG_001.ARW");
        let jpg = path("D:/x/IMG_001.JPG");
        let other = path("D:/x/IMG_002.ARW");
        let locked = path("D:/x/IMG_003.ARW");
        let partly = path("D:/x/IMG_004.ARW");
        let mut batch = DeleteBatch::new(vec![
            raw.clone(),
            jpg.clone(),
            other.clone(),
            locked.clone(),
            partly.clone(),
        ]);
        for image in [&raw, &jpg, &other, &locked, &partly] {
            batch.sent(image);
        }
        let raw_files = vec![raw.clone(), jpg.clone(), path("D:/x/IMG_001.ARW.xmp")];
        batch.answered(&raw, Ok(raw_files.clone()));
        assert!(!batch.done());
        // The JPG went with its RAW: its own answer fails, and is not a failure.
        batch.answered(&jpg, Err(error("D:/x/img_001.jpg", DeleteRefusal::Missing)));
        batch.answered(&other, Ok(vec![other.clone()]));
        let locked_error = error("D:/x/IMG_003.xmp", DeleteRefusal::OpenElsewhere);
        batch.answered(&locked, Err(locked_error.clone()));
        let partly_error = DeleteError {
            file: path("D:/x/IMG_004.xmp"),
            reason: DeleteRefusal::Failed("Access is denied.".to_owned()),
            left: vec![path("D:/x/IMG_004.xmp")],
            deleted: vec![partly.clone()],
        };
        batch.answered(&partly, Err(partly_error.clone()));
        // A second answer changes nothing.
        batch.answered(
            &other,
            Err(error("D:/x/IMG_002.ARW", DeleteRefusal::Missing)),
        );
        assert!(batch.done());

        let result = batch.finish();
        assert_eq!(result.images, vec![raw.clone(), other.clone()]);
        let mut files = raw_files;
        files.push(other);
        files.push(partly.clone());
        assert_eq!(result.files, files);
        assert_eq!(
            result.failed,
            vec![(locked, locked_error), (partly, partly_error)]
        );
    }

    #[test]
    fn rejected_images_keeps_the_order_and_leaves_out_the_others() {
        let files = vec![
            path("D:/x/a.ARW"),
            path("D:/x/b.ARW"),
            path("D:/x/c.JPG"),
            path("D:/x/d.ARW"),
        ];
        let rejected = [path("D:/x/d.ARW"), path("D:/x/a.ARW")];
        let images = rejected_images(&files, |file| rejected.iter().any(|known| known == file));
        assert_eq!(images, vec![path("D:/x/a.ARW"), path("D:/x/d.ARW")]);
        assert!(rejected_images(&files, |_| false).is_empty());
        assert!(rejected_images(&[], |_| true).is_empty());
    }

    #[test]
    fn image_partners_are_the_image_files_after_the_image_itself() {
        let files = [
            path("D:/x/IMG_001.ARW"),
            path("D:/x/IMG_001.ARW.xmp"),
            path("D:/x/IMG_001.tif"),
            path("D:/x/IMG_001.xmp"),
            path("D:/x/IMG_001.JPG"),
        ];
        assert_eq!(
            image_partners(&files, is_image),
            vec![path("D:/x/IMG_001.tif"), path("D:/x/IMG_001.JPG")]
        );
        // The image itself is never its own partner, and a group of the image alone has none.
        assert!(image_partners(&files[..2], is_image).is_empty());
        assert!(image_partners(&[], is_image).is_empty());
    }

    /// The tab's list as `main.rs`'s closure reads it: the file compared with `same_path`, and a
    /// file not in the list counts as not rejected.
    fn rejected_in(tab: &[(PathBuf, bool)]) -> impl Fn(&Path) -> bool + '_ {
        move |file| {
            tab.iter()
                .find(|(known, _)| same_path(known, file))
                .is_some_and(|(_, rejected)| *rejected)
        }
    }

    fn partner_refused(image: &str) -> PreparedDelete {
        refused(
            image,
            DeleteRefusal::Failed("its RAW+JPEG partner is not rejected".to_owned()),
        )
    }

    #[test]
    fn refuse_unrejected_partners_refuses_a_raw_whose_jpg_is_not_rejected() {
        let tab = [
            (path("D:/x/IMG_001.ARW"), true),
            (path("D:/x/IMG_001.JPG"), false),
            (path("D:/x/IMG_002.ARW"), true),
        ];
        let prepared = refuse_unrejected_partners(
            vec![
                // The worker's path of the partner in other case: still the tab's file.
                ok(
                    "D:/x/IMG_001.ARW",
                    &["D:/x/IMG_001.ARW", "D:/x/img_001.jpg", "D:/x/IMG_001.xmp"],
                    10,
                ),
                ok("D:/x/IMG_002.ARW", &["D:/x/IMG_002.ARW"], 5),
            ],
            rejected_in(&tab),
            is_image,
        );
        assert_eq!(
            prepared,
            vec![
                partner_refused("D:/x/IMG_001.ARW"),
                ok("D:/x/IMG_002.ARW", &["D:/x/IMG_002.ARW"], 5),
            ]
        );
        let plan = plan_delete(prepared, is_image);
        assert_eq!(plan.images, vec![path("D:/x/IMG_002.ARW")]);
        assert_eq!(
            plan.refused,
            vec![(
                path("D:/x/IMG_001.ARW"),
                "its RAW+JPEG partner is not rejected".to_owned()
            )]
        );
    }

    #[test]
    fn refuse_unrejected_partners_keeps_a_pair_where_both_are_rejected() {
        let tab = [
            (path("D:/x/IMG_001.ARW"), true),
            (path("D:/x/IMG_001.JPG"), true),
        ];
        let answers = vec![
            ok(
                "D:/x/IMG_001.ARW",
                &["D:/x/IMG_001.ARW", "D:/x/IMG_001.JPG", "D:/x/IMG_001.xmp"],
                10,
            ),
            ok(
                "D:/x/IMG_001.JPG",
                &["D:/x/IMG_001.JPG", "D:/x/IMG_001.ARW", "D:/x/IMG_001.xmp"],
                10,
            ),
        ];
        let prepared = refuse_unrejected_partners(answers.clone(), rejected_in(&tab), is_image);
        assert_eq!(prepared, answers);
    }

    #[test]
    fn refuse_unrejected_partners_leaves_a_group_without_a_partner_as_it_is() {
        let answers = vec![
            ok("D:/x/a.ARW", &["D:/x/a.ARW", "D:/x/a.ARW.xmp"], 10),
            refused("E:/DCIM/b.ARW", DeleteRefusal::Card),
        ];
        // Not even the images are rejected here: only image files after the first are asked
        // about, and an error stays as it is.
        let prepared = refuse_unrejected_partners(answers.clone(), |_| false, is_image);
        assert_eq!(prepared, answers);
    }

    #[test]
    fn refuse_unrejected_partners_refuses_a_raw_whose_jpg_is_not_in_the_tab() {
        let tab = [(path("D:/x/IMG_001.ARW"), true)];
        let prepared = refuse_unrejected_partners(
            vec![ok(
                "D:/x/IMG_001.ARW",
                &["D:/x/IMG_001.ARW", "D:/x/IMG_001.JPG"],
                10,
            )],
            rejected_in(&tab),
            is_image,
        );
        assert_eq!(prepared, vec![partner_refused("D:/x/IMG_001.ARW")]);
    }

    #[test]
    fn busy_is_true_for_every_step_but_idle() {
        assert!(!DeleteStep::default().busy());
        assert!(!DeleteStep::Idle.busy());
        assert!(
            DeleteStep::Preparing {
                images: vec![path("D:/x/a.ARW")],
                id: None,
            }
            .busy()
        );
        assert!(
            DeleteStep::Preparing {
                images: Vec::new(),
                id: Some(7),
            }
            .busy()
        );
        assert!(DeleteStep::Confirming(plan_delete(Vec::new(), |_| true)).busy());
        assert!(DeleteStep::Running(DeleteBatch::new(Vec::new())).busy());
    }
}
