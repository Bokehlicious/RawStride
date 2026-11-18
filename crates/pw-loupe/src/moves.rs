//! Moving rejected and picked images to subfolders (ADR 0017 §2.6, step U9b, U9-Q5–U9-Q9; T172):
//! where an image goes, when its move may be sent, a batch's answers gathered into one undo step,
//! and the undo list. Plain Rust without I/O (GR11); the metadata worker moves the files (T170),
//! the shell connects it (T173, T174). The texts are in `moves_text.rs`.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use pw_metadata::{Moved, Rating};

use crate::deletes::same_path;

/// The folder name rejected images are moved to, in the image's own folder (not translated).
pub const REJECTED_FOLDER: &str = "_rejected";
/// The folder name picked images are moved to, in the image's own folder (not translated).
pub const PICKED_FOLDER: &str = "_picked";

/// The most steps the undo list keeps (U9-Q8).
pub const UNDO_STEPS: usize = 20;

/// Which key moves the image: X (rejected) or P (picked), U9-Q6.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveKind {
    Reject,
    Pick,
}

impl MoveKind {
    /// The name of the subfolder this kind moves to.
    pub fn folder_name(self) -> &'static str {
        match self {
            MoveKind::Reject => REJECTED_FOLDER,
            MoveKind::Pick => PICKED_FOLDER,
        }
    }
}

/// The folder `image` is moved to: its own folder joined with the kind's name. `None` when the
/// image has no folder or an empty one (the shell passes absolute paths), or when its folder's
/// own name is `_rejected` or `_picked`, compared without regard to case (U9-Q9: there the keys
/// only set the rating).
pub fn target_folder(image: &Path, kind: MoveKind) -> Option<PathBuf> {
    let folder = image
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty())?;
    if let Some(name) = folder.file_name() {
        let name = name.to_string_lossy();
        if name.eq_ignore_ascii_case(REJECTED_FOLDER) || name.eq_ignore_ascii_case(PICKED_FOLDER) {
            return None;
        }
    }
    Some(folder.join(kind.folder_name()))
}

/// One image of a finished batch whose move succeeded: the files moved (the image and its
/// companion files) and, for an X, the rating it had before (`RatingStore::current_rating`), so the
/// undo can restore it. `restore` is `None` for P, which changes no rating.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovedImage {
    pub image: PathBuf,
    pub moved: Vec<Moved>,
    pub restore: Option<Rating>,
}

/// What a finished batch gives: the images moved (one undo step) and the images that could not be
/// moved, with the reason.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BatchResult {
    pub moved: Vec<MovedImage>,
    pub failed: Vec<(PathBuf, String)>,
}

#[derive(Clone, Debug)]
struct BatchImage {
    image: PathBuf,
    target: PathBuf,
    restore: Option<Rating>,
    sent: bool,
    /// Not sent and needs no answer (`skip`, T178).
    skipped: bool,
    answer: Option<Result<Vec<Moved>, String>>,
}

/// The moves of one key press (U9-Q7): each image is moved all or nothing on its own, and the
/// whole batch becomes one undo step.
#[derive(Clone, Debug)]
pub struct Batch {
    kind: MoveKind,
    images: Vec<BatchImage>,
}

impl Batch {
    /// A batch of `kind` of `(image, target folder, rating to restore)`, in the order they are
    /// sent.
    pub fn new(kind: MoveKind, images: Vec<(PathBuf, PathBuf, Option<Rating>)>) -> Batch {
        Batch {
            kind,
            images: images
                .into_iter()
                .map(|(image, target, restore)| BatchImage {
                    image,
                    target,
                    restore,
                    sent: false,
                    skipped: false,
                    answer: None,
                })
                .collect(),
        }
    }

    /// The key that made the batch (T178).
    pub fn kind(&self) -> MoveKind {
        self.kind
    }

    /// Whether `image` is one of the batch's images and is not skipped, compared as given (T178).
    /// A skipped image is no longer held, so a later X that rejects it again queues a new batch
    /// for it; an image whose move failed stays held (its answer may be a JPG that its RAW
    /// partner moved along).
    pub fn contains(&self, image: &Path) -> bool {
        self.images
            .iter()
            .any(|entry| !entry.skipped && entry.image == image)
    }

    /// The images not yet sent or skipped whose path has a rating write still waiting
    /// (`is_waiting`; the shell passes `is_pending`: queued, or sent without an answer, T178) left
    /// out, with their targets, in the batch's order. An image with a waiting write waits, so its
    /// move reaches the worker after that write's answer (T170 point 4). Marks nothing: `sent` does.
    pub fn to_send(&self, is_waiting: impl Fn(&Path) -> bool) -> Vec<(PathBuf, PathBuf)> {
        self.images
            .iter()
            .filter(|entry| {
                !entry.sent && !entry.skipped && entry.answer.is_none() && !is_waiting(&entry.image)
            })
            .map(|entry| (entry.image.clone(), entry.target.clone()))
            .collect()
    }

    /// Marks `image` as sent once the worker took its move; a full queue leaves it for the next
    /// `to_send`. An image not in the batch changes nothing.
    pub fn sent(&mut self, image: &Path) {
        if let Some(entry) = self
            .images
            .iter_mut()
            .find(|entry| !entry.sent && !entry.skipped && entry.image == image)
        {
            entry.sent = true;
        }
    }

    /// Leaves `image` out: it is not sent and needs no answer, and `finish` lists it in neither
    /// list (T178). An image not in the batch, or one already sent or answered, changes nothing.
    pub fn skip(&mut self, image: &Path) {
        if let Some(entry) = self.images.iter_mut().find(|entry| {
            !entry.sent && !entry.skipped && entry.answer.is_none() && entry.image == image
        }) {
            entry.skipped = true;
        }
    }

    /// Records the worker's answer for `image`. An image not in the batch, or one that has its
    /// answer already, changes nothing.
    pub fn answered(&mut self, image: &Path, result: Result<Vec<Moved>, String>) {
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

    /// The batch's result, in the batch's order: the images moved and the images that failed. An
    /// image that another image of the batch moved as its partner (its path is a `from` in another
    /// answer's moves, for example the JPG of a marked RAW+JPEG pair) is not a failure: its own
    /// answer is dropped, and it is in `moved` with no moves of its own and its `restore`, so the
    /// undo restores its rating too. An image without an answer yet, or skipped, is in neither
    /// list.
    pub fn finish(self) -> BatchResult {
        let mut result = BatchResult::default();
        let moved_by: Vec<(usize, HashSet<&Path>)> = self
            .images
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| match &entry.answer {
                Some(Ok(moves)) => Some((
                    index,
                    moves.iter().map(|moved| moved.from.as_path()).collect(),
                )),
                _ => None,
            })
            .collect();
        let partner_moved = |index: usize, image: &Path| {
            moved_by
                .iter()
                .any(|(other, froms)| *other != index && froms.contains(image))
        };
        let partners: Vec<bool> = self
            .images
            .iter()
            .enumerate()
            .map(|(index, entry)| partner_moved(index, &entry.image))
            .collect();
        for (entry, partner) in self.images.into_iter().zip(partners) {
            match entry.answer {
                Some(Ok(moved)) => result.moved.push(MovedImage {
                    image: entry.image,
                    moved,
                    restore: entry.restore,
                }),
                // Moved with another image: no files of its own to move back, but its rating
                // before the key is restored by the undo as well (review of T172).
                Some(Err(_)) if partner => result.moved.push(MovedImage {
                    image: entry.image,
                    moved: Vec::new(),
                    restore: entry.restore,
                }),
                Some(Err(reason)) => result.failed.push((entry.image, reason)),
                None => {}
            }
        }
        result
    }
}

/// The folders a step's files went to (ADR 0017 U9b, T183): the parent of every `Moved::to` of
/// every image (the image and its companion files, as moved), an empty parent left out, each
/// folder once, compared as given, in the order first met. The tabs open on them are listed again
/// after a batch and an undo.
pub fn moved_to_folders(images: &[MovedImage]) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = Vec::new();
    for folder in images
        .iter()
        .flat_map(|image| image.moved.iter())
        .filter_map(|moved| moved.to.parent())
        .filter(|folder| !folder.as_os_str().is_empty())
    {
        if !folders.iter().any(|known| known == folder) {
            folders.push(folder.to_path_buf());
        }
    }
    folders
}

/// The moves that can be undone (U9-Q8): at most `UNDO_STEPS` steps, one per batch, for the
/// running session only; one list for the whole program, not per tab.
#[derive(Clone, Debug, Default)]
pub struct UndoMoves {
    steps: VecDeque<Vec<MovedImage>>,
}

impl UndoMoves {
    /// Adds a step; an empty step is not pushed, and a step past `UNDO_STEPS` drops the oldest.
    pub fn push(&mut self, step: Vec<MovedImage>) {
        if step.is_empty() {
            return;
        }
        while self.steps.len() >= UNDO_STEPS {
            let _ = self.steps.pop_front();
        }
        self.steps.push_back(step);
    }

    /// Takes the newest step.
    pub fn pop(&mut self) -> Option<Vec<MovedImage>> {
        self.steps.pop_back()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Forgets the files a delete sent to the Recycle Bin (ADR 0018 §2.10, T187), compared with
    /// `same_path`: in each step, every image with a `Moved::to` among `deleted` is dropped, then
    /// a partner's entry with no moves of its own whose image is a `Moved::from` of an entry
    /// dropped in the same step; a step left empty is dropped. The dropped entries' `restore`
    /// ratings are lost: the files are gone.
    pub fn forget_deleted(&mut self, deleted: &[PathBuf]) {
        for step in &mut self.steps {
            let mut froms: Vec<PathBuf> = Vec::new();
            step.retain(|entry| {
                let gone = entry
                    .moved
                    .iter()
                    .any(|moved| deleted.iter().any(|file| same_path(&moved.to, file)));
                if gone {
                    froms.extend(entry.moved.iter().map(|moved| moved.from.clone()));
                }
                !gone
            });
            step.retain(|entry| {
                !(entry.moved.is_empty() && froms.iter().any(|from| same_path(from, &entry.image)))
            });
        }
        self.steps.retain(|step| !step.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moved(from: &str, to: &str) -> Moved {
        Moved {
            from: PathBuf::from(from),
            to: PathBuf::from(to),
        }
    }

    #[test]
    fn moved_to_folders_gives_each_folder_once() {
        let image = |name: &str, moved: Vec<Moved>| MovedImage {
            image: PathBuf::from(name),
            moved,
            restore: None,
        };
        let one = vec![
            image(
                "D:/x/a.ARW",
                vec![
                    moved("D:/x/a.ARW", "D:/x/_rejected/a.ARW"),
                    moved("D:/x/a.xmp", "D:/x/_rejected/a.xmp"),
                ],
            ),
            image(
                "D:/x/b.ARW",
                vec![
                    moved("D:/x/b.ARW", "D:/x/_rejected/b.ARW"),
                    moved("D:/x/b.JPG", "D:/x/_rejected/b.JPG"),
                ],
            ),
        ];
        assert_eq!(
            moved_to_folders(&one),
            vec![PathBuf::from("D:/x/_rejected")]
        );

        // An image of a second folder adds its own `_rejected` after the first.
        let mut two = one.clone();
        two.push(image(
            "D:/y/c.ARW",
            vec![moved("D:/y/c.ARW", "D:/y/_rejected/c.ARW")],
        ));
        assert_eq!(
            moved_to_folders(&two),
            vec![
                PathBuf::from("D:/x/_rejected"),
                PathBuf::from("D:/y/_rejected")
            ]
        );

        assert!(moved_to_folders(&[]).is_empty());
    }

    #[test]
    fn the_target_is_the_kinds_folder_in_the_images_own_folder() {
        let image = Path::new("D:/Foton/2026/IMG_001.ARW");
        assert_eq!(
            target_folder(image, MoveKind::Reject),
            Some(PathBuf::from("D:/Foton/2026").join("_rejected"))
        );
        assert_eq!(
            target_folder(image, MoveKind::Pick),
            Some(PathBuf::from("D:/Foton/2026").join("_picked"))
        );
    }

    #[test]
    fn inside_a_rejected_or_picked_folder_nothing_is_moved() {
        for folder in ["_rejected", "_Picked", "_REJECTED", "_picked"] {
            let image = PathBuf::from("D:/Foton").join(folder).join("IMG_001.ARW");
            assert_eq!(target_folder(&image, MoveKind::Reject), None, "{folder}");
            assert_eq!(target_folder(&image, MoveKind::Pick), None, "{folder}");
        }
        // A folder whose name only starts with it is an ordinary folder.
        let image = Path::new("D:/Foton/_rejected old/IMG_001.ARW");
        assert_eq!(
            target_folder(image, MoveKind::Pick),
            Some(PathBuf::from("D:/Foton/_rejected old").join("_picked"))
        );
    }

    #[test]
    fn an_image_without_a_folder_has_no_target() {
        assert_eq!(
            target_folder(Path::new("IMG_001.ARW"), MoveKind::Reject),
            None
        );
        assert_eq!(target_folder(Path::new(""), MoveKind::Pick), None);
    }

    #[test]
    fn to_send_waits_for_a_queued_write_and_never_gives_a_sent_image_again() {
        let a = PathBuf::from("D:/x/a.ARW");
        let b = PathBuf::from("D:/x/b.ARW");
        let target = PathBuf::from("D:/x/_rejected");
        let mut batch = Batch::new(
            MoveKind::Reject,
            vec![
                (a.clone(), target.clone(), Some(Rating::default())),
                (b.clone(), target.clone(), Some(Rating::default())),
            ],
        );

        // a has a rating write waiting: only b may go.
        let queued_a = |path: &Path| path == a;
        assert_eq!(batch.to_send(queued_a), vec![(b.clone(), target.clone())]);
        // `to_send` marks nothing: a full queue leaves b for the next call.
        assert_eq!(batch.to_send(queued_a), vec![(b.clone(), target.clone())]);
        batch.sent(&b);
        assert!(batch.to_send(queued_a).is_empty());

        // The write left the queue: a goes now, b never again.
        assert_eq!(batch.to_send(|_| false), vec![(a.clone(), target.clone())]);
        batch.sent(&a);
        assert!(batch.to_send(|_| false).is_empty());
        assert!(!batch.done());
    }

    #[test]
    fn finish_sorts_the_answers_and_drops_a_partner_moved_by_another_image() {
        let raw = PathBuf::from("D:/x/IMG_001.ARW");
        let jpg = PathBuf::from("D:/x/IMG_001.JPG");
        let other = PathBuf::from("D:/x/IMG_002.ARW");
        let locked = PathBuf::from("D:/x/IMG_003.ARW");
        let target = PathBuf::from("D:/x/_rejected");
        let stars = Rating {
            stars: Some(3),
            label: None,
        };
        let jpg_before = Rating {
            stars: Some(2),
            label: None,
        };
        let mut batch = Batch::new(
            MoveKind::Reject,
            vec![
                (raw.clone(), target.clone(), Some(stars.clone())),
                (jpg.clone(), target.clone(), Some(jpg_before.clone())),
                (other.clone(), target.clone(), None),
                (locked.clone(), target.clone(), None),
            ],
        );
        for image in [&raw, &jpg, &other, &locked] {
            batch.sent(image);
        }
        let raw_moves = vec![
            moved("D:/x/IMG_001.ARW", "D:/x/_rejected/IMG_001.ARW"),
            moved("D:/x/IMG_001.JPG", "D:/x/_rejected/IMG_001.JPG"),
            moved("D:/x/IMG_001.ARW.xmp", "D:/x/_rejected/IMG_001.ARW.xmp"),
        ];
        let other_moves = vec![moved("D:/x/IMG_002.ARW", "D:/x/_rejected/IMG_002.ARW")];
        batch.answered(&raw, Ok(raw_moves.clone()));
        assert!(!batch.done());
        // The JPG went with its RAW: its own answer fails, and is not a failure.
        batch.answered(&jpg, Err("the file is missing".to_owned()));
        batch.answered(&other, Ok(other_moves.clone()));
        batch.answered(
            &locked,
            Err("the file is open in another program".to_owned()),
        );
        assert!(batch.done());

        let result = batch.finish();
        assert_eq!(
            result.moved,
            vec![
                MovedImage {
                    image: raw,
                    moved: raw_moves,
                    restore: Some(stars),
                },
                // The JPG has no moves of its own, but the undo restores its rating.
                MovedImage {
                    image: jpg,
                    moved: Vec::new(),
                    restore: Some(jpg_before),
                },
                MovedImage {
                    image: other,
                    moved: other_moves,
                    restore: None,
                },
            ]
        );
        assert_eq!(
            result.failed,
            vec![(locked, "the file is open in another program".to_owned())]
        );
    }

    #[test]
    fn a_failed_image_is_not_dropped_for_its_own_moves() {
        // Only another image's moves make a failed answer a partner.
        let a = PathBuf::from("D:/x/a.ARW");
        let mut batch = Batch::new(
            MoveKind::Pick,
            vec![(a.clone(), PathBuf::from("D:/x/_picked"), None)],
        );
        batch.answered(&a, Err("no".to_owned()));
        assert_eq!(batch.finish().failed, vec![(a, "no".to_owned())]);
    }

    #[test]
    fn kind_gives_the_kind_passed_to_new() {
        for kind in [MoveKind::Reject, MoveKind::Pick] {
            assert_eq!(Batch::new(kind, Vec::new()).kind(), kind);
        }
    }

    #[test]
    fn a_skipped_image_is_not_sent_and_is_in_neither_list() {
        let a = PathBuf::from("D:/x/a.ARW");
        let b = PathBuf::from("D:/x/b.ARW");
        let target = PathBuf::from("D:/x/_rejected");
        let mut batch = Batch::new(
            MoveKind::Reject,
            vec![
                (a.clone(), target.clone(), Some(Rating::default())),
                (b.clone(), target.clone(), Some(Rating::default())),
            ],
        );
        batch.skip(&a);
        assert_eq!(batch.to_send(|_| false), vec![(b.clone(), target.clone())]);
        // A skipped image takes no `sent` and no answer.
        batch.sent(&a);
        batch.answered(&a, Ok(vec![moved("D:/x/a.ARW", "D:/x/_rejected/a.ARW")]));
        assert!(
            batch
                .to_send(|_| false)
                .iter()
                .all(|(image, _)| *image != a)
        );
        batch.sent(&b);
        assert!(!batch.done());
        let b_moves = vec![moved("D:/x/b.ARW", "D:/x/_rejected/b.ARW")];
        batch.answered(&b, Ok(b_moves.clone()));
        assert!(batch.done());

        let result = batch.finish();
        assert_eq!(
            result.moved,
            vec![MovedImage {
                image: b,
                moved: b_moves,
                restore: Some(Rating::default()),
            }]
        );
        assert!(result.failed.is_empty());
    }

    #[test]
    fn skip_of_an_image_already_sent_changes_nothing() {
        let a = PathBuf::from("D:/x/a.ARW");
        let mut batch = Batch::new(
            MoveKind::Pick,
            vec![(a.clone(), PathBuf::from("D:/x/_picked"), None)],
        );
        batch.sent(&a);
        batch.skip(&a);
        assert!(!batch.done(), "still waits for its answer");
        batch.answered(&a, Err("no".to_owned()));
        assert!(batch.done());
        assert_eq!(batch.finish().failed, vec![(a, "no".to_owned())]);
    }

    #[test]
    fn contains_is_true_for_each_image_of_the_batch_only() {
        let a = PathBuf::from("D:/x/a.ARW");
        let b = PathBuf::from("D:/x/b.ARW");
        let target = PathBuf::from("D:/x/_picked");
        let batch = Batch::new(
            MoveKind::Pick,
            vec![(a.clone(), target.clone(), None), (b.clone(), target, None)],
        );
        assert!(batch.contains(&a));
        assert!(batch.contains(&b));
        assert!(!batch.contains(Path::new("D:/x/c.ARW")));
    }

    #[test]
    fn contains_is_false_for_a_skipped_image_and_true_for_a_failed_one() {
        let a = PathBuf::from("D:/x/a.ARW");
        let b = PathBuf::from("D:/x/b.ARW");
        let c = PathBuf::from("D:/x/c.ARW");
        let target = PathBuf::from("D:/x/_rejected");
        let mut batch = Batch::new(
            MoveKind::Reject,
            vec![
                (a.clone(), target.clone(), Some(Rating::default())),
                (b.clone(), target.clone(), Some(Rating::default())),
                (c.clone(), target, Some(Rating::default())),
            ],
        );
        batch.skip(&a);
        assert!(!batch.contains(&a), "a skipped image is no longer held");
        assert!(batch.contains(&b));
        assert!(batch.contains(&c));

        batch.sent(&b);
        batch.answered(&b, Err("the file is open in another program".to_owned()));
        assert!(batch.contains(&b), "a failed image stays held");
        assert!(batch.contains(&c));
    }

    fn step(name: usize) -> Vec<MovedImage> {
        vec![MovedImage {
            image: PathBuf::from(format!("D:/x/{name}.ARW")),
            moved: Vec::new(),
            restore: None,
        }]
    }

    #[test]
    fn the_undo_list_keeps_twenty_steps_and_drops_the_oldest() {
        let mut undo = UndoMoves::default();
        assert!(undo.is_empty());
        for name in 1..=UNDO_STEPS + 1 {
            undo.push(step(name));
        }
        let mut popped = Vec::new();
        while let Some(step) = undo.pop() {
            popped.push(step);
        }
        assert_eq!(popped.len(), UNDO_STEPS);
        assert_eq!(popped.first(), Some(&step(UNDO_STEPS + 1)));
        // Step 1 was dropped; step 2 is the oldest kept.
        assert_eq!(popped.last(), Some(&step(2)));
        assert!(undo.is_empty());
    }

    #[test]
    fn an_empty_step_is_not_pushed() {
        let mut undo = UndoMoves::default();
        undo.push(Vec::new());
        assert!(undo.is_empty());
        undo.push(step(1));
        undo.push(Vec::new());
        assert_eq!(undo.pop(), Some(step(1)));
        assert_eq!(undo.pop(), None);
    }

    #[test]
    fn forget_deleted_drops_a_deleted_image_keeps_the_others_and_drops_an_empty_step() {
        let entry = |name: &str| MovedImage {
            image: PathBuf::from(format!("D:/x/{name}")),
            moved: vec![moved(
                &format!("D:/x/{name}"),
                &format!("D:/x/_rejected/{name}"),
            )],
            restore: None,
        };
        let mut undo = UndoMoves::default();
        undo.push(vec![entry("a.ARW")]);
        undo.push(vec![entry("b.ARW"), entry("c.ARW")]);

        // In other case, as the delete's answer may give it.
        undo.forget_deleted(&[
            PathBuf::from("d:/x/_REJECTED/a.arw"),
            PathBuf::from("D:/x/_rejected/b.ARW"),
        ]);
        assert_eq!(undo.pop(), Some(vec![entry("c.ARW")]));
        assert_eq!(undo.pop(), None, "a's step was left empty and dropped");
    }

    #[test]
    fn forget_deleted_drops_a_raw_and_jpg_step_whole() {
        let raw = PathBuf::from("D:/x/IMG_001.ARW");
        let jpg = PathBuf::from("D:/x/IMG_001.JPG");
        let target = PathBuf::from("D:/x/_rejected");
        let mut batch = Batch::new(
            MoveKind::Reject,
            vec![
                (raw.clone(), target.clone(), Some(Rating::default())),
                (jpg.clone(), target, Some(Rating::default())),
            ],
        );
        batch.answered(
            &raw,
            Ok(vec![
                moved("D:/x/IMG_001.ARW", "D:/x/_rejected/IMG_001.ARW"),
                moved("D:/x/IMG_001.JPG", "D:/x/_rejected/IMG_001.JPG"),
            ]),
        );
        batch.answered(&jpg, Err("the file is missing".to_owned()));
        let step = batch.finish().moved;
        assert_eq!(step.len(), 2, "the JPG's entry has no moves");

        let mut undo = UndoMoves::default();
        undo.push(step);
        undo.forget_deleted(&[
            PathBuf::from("D:/x/_rejected/IMG_001.ARW"),
            PathBuf::from("D:/x/_rejected/IMG_001.JPG"),
        ]);
        assert!(undo.is_empty());
    }
}
