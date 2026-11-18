//! The texts of deleting images to the Recycle Bin (ADR 0018 §2.4, §2.5, §2.7, T187;
//! `docs/tasks/README.md` rule 13): whole sentences, each case its own template, the singular and
//! the plural in the same function. `reason` is a `DeleteRefusal`'s phrase and is not followed by
//! a full stop at the end of a text, since a system error text may end in one.

use pw_core::format_bytes;

/// The title of the confirmation window and the menu item (key Delete).
pub fn delete_title_text() -> &'static str {
    "Delete to Recycle Bin"
}

/// The confirmation's question: `images` images with `files` files of `bytes` in all.
pub fn delete_confirm_text(images: usize, files: usize, bytes: u64) -> String {
    let size = format_bytes(bytes);
    if images == 1 && files == 1 {
        format!(
            "Move 1 image (1 file, {size}) to the Recycle Bin? You can restore it from the \
             Recycle Bin in Windows."
        )
    } else if images == 1 {
        format!(
            "Move 1 image ({files} files, {size}) to the Recycle Bin? You can restore it from \
             the Recycle Bin in Windows."
        )
    } else {
        format!(
            "Move {images} images ({files} files, {size}) to the Recycle Bin? You can restore \
             them from the Recycle Bin in Windows."
        )
    }
}

/// The images that cannot be deleted, naming the first (under the question, or on the status line
/// when nothing can be deleted).
pub fn delete_refused_text(count: usize, name: &str, reason: &str) -> String {
    if count <= 1 {
        format!("{name} cannot be deleted: {reason}")
    } else {
        format!("{count} images cannot be deleted. The first: {name}: {reason}")
    }
}

/// The image files of the same name that go along without being marked (ADR 0018 B-Q18).
pub fn delete_partners_text(names: &[String]) -> String {
    if let [name] = names {
        format!("This image file of the same name goes along: {name}.")
    } else {
        let names = names.join(", ");
        format!("These image files of the same name go along: {names}.")
    }
}

/// The confirmation's button that deletes.
pub fn delete_button_text() -> &'static str {
    "Delete"
}

/// The confirmation's button that deletes nothing.
pub fn delete_cancel_text() -> &'static str {
    "Cancel"
}

/// The status-line text after `count` images went to the Recycle Bin.
pub fn deleted_text(count: usize) -> String {
    if count == 1 {
        "1 image moved to the Recycle Bin.".to_owned()
    } else {
        format!("{count} images moved to the Recycle Bin.")
    }
}

/// The status-line text when one image could not be deleted.
pub fn delete_failed_text(name: &str, reason: &str) -> String {
    format!("{name} could not be moved to the Recycle Bin: {reason}")
}

/// The status-line text when an image was deleted only in part (ADR 0018 §2.4): `left` are the
/// paths of the files still in place.
pub fn delete_partly_failed_text(name: &str, reason: &str, left: &[String]) -> String {
    let left = left.join(", ");
    format!(
        "{name} could not be moved to the Recycle Bin: {reason}. Some of its files are already \
         in the Recycle Bin; these were left: {left}."
    )
}

/// The status-line text when `count` images of a batch could not be deleted, naming the last one;
/// for one image, the same sentence as `delete_failed_text`.
pub fn deletes_failed_text(count: usize, name: &str, reason: &str) -> String {
    if count <= 1 {
        format!("{name} could not be moved to the Recycle Bin: {reason}")
    } else {
        format!("{count} images could not be moved to the Recycle Bin. The last: {name}: {reason}")
    }
}

/// The status-line text when a delete is asked for while images are being moved.
pub fn delete_while_moving_text() -> &'static str {
    "Images are still being moved. Delete again when the move is done."
}

/// The status-line text when a delete is asked for while another delete runs.
pub fn delete_running_text() -> &'static str {
    "A delete is still running. Try again when it is done."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_confirmation_has_four_templates() {
        assert_eq!(
            delete_confirm_text(1, 1, 500),
            "Move 1 image (1 file, 500 B) to the Recycle Bin? You can restore it from the \
             Recycle Bin in Windows."
        );
        assert_eq!(
            delete_confirm_text(1, 3, 2048),
            "Move 1 image (3 files, 2.0 KB) to the Recycle Bin? You can restore it from the \
             Recycle Bin in Windows."
        );
        assert_eq!(
            delete_confirm_text(4, 9, 3 * 1024 * 1024),
            "Move 4 images (9 files, 3.0 MB) to the Recycle Bin? You can restore them from the \
             Recycle Bin in Windows."
        );
        assert_eq!(
            delete_confirm_text(0, 0, 0),
            "Move 0 images (0 files, 0 B) to the Recycle Bin? You can restore them from the \
             Recycle Bin in Windows."
        );
    }

    #[test]
    fn the_texts_with_a_count_with_one_and_with_more() {
        assert_eq!(
            delete_refused_text(1, "IMG_001.ARW", "the drive is a memory card"),
            "IMG_001.ARW cannot be deleted: the drive is a memory card"
        );
        assert_eq!(
            delete_refused_text(3, "IMG_001.ARW", "the drive is a memory card"),
            "3 images cannot be deleted. The first: IMG_001.ARW: the drive is a memory card"
        );
        assert_eq!(
            delete_partners_text(&["IMG_001.JPG".to_owned()]),
            "This image file of the same name goes along: IMG_001.JPG."
        );
        assert_eq!(
            delete_partners_text(&["IMG_001.JPG".to_owned(), "IMG_001.TIF".to_owned()]),
            "These image files of the same name go along: IMG_001.JPG, IMG_001.TIF."
        );
        assert_eq!(deleted_text(1), "1 image moved to the Recycle Bin.");
        assert_eq!(deleted_text(2), "2 images moved to the Recycle Bin.");
        assert_eq!(deleted_text(12), "12 images moved to the Recycle Bin.");
        assert_eq!(
            deletes_failed_text(1, "IMG_001.ARW", "the file is open in another program"),
            "IMG_001.ARW could not be moved to the Recycle Bin: the file is open in another \
             program"
        );
        assert_eq!(
            deletes_failed_text(3, "IMG_003.ARW", "the file does not exist"),
            "3 images could not be moved to the Recycle Bin. The last: IMG_003.ARW: the file \
             does not exist"
        );
    }

    #[test]
    fn the_texts_without_a_count() {
        assert_eq!(delete_title_text(), "Delete to Recycle Bin");
        assert_eq!(delete_button_text(), "Delete");
        assert_eq!(delete_cancel_text(), "Cancel");
        assert_eq!(
            delete_failed_text("IMG_001.xmp", "the file is open in another program"),
            "IMG_001.xmp could not be moved to the Recycle Bin: the file is open in another \
             program"
        );
        assert_eq!(
            delete_partly_failed_text(
                "IMG_001.xmp",
                "the file is open in another program",
                &[
                    "D:\\x\\IMG_001.xmp".to_owned(),
                    "D:\\x\\IMG_001.JPG".to_owned()
                ]
            ),
            "IMG_001.xmp could not be moved to the Recycle Bin: the file is open in another \
             program. Some of its files are already in the Recycle Bin; these were left: \
             D:\\x\\IMG_001.xmp, D:\\x\\IMG_001.JPG."
        );
        assert_eq!(
            delete_while_moving_text(),
            "Images are still being moved. Delete again when the move is done."
        );
        assert_eq!(
            delete_running_text(),
            "A delete is still running. Try again when it is done."
        );
    }
}
