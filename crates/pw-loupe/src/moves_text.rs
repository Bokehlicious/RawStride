//! The texts of moving rejected and picked images to subfolders and of undoing it (ADR 0017
//! U9-Q5–U9-Q9, T172; `docs/tasks/README.md` rule 13): whole sentences, the singular and the plural
//! in the same function.

/// The status-line text when one image could not be moved.
pub fn move_failed_text(name: &str, reason: &str) -> String {
    format!("{name} could not be moved: {reason}")
}

/// The status-line text when `count` images of a batch could not be moved, naming the last one;
/// for one image, the same sentence as `move_failed_text`.
pub fn moves_failed_text(count: usize, name: &str, reason: &str) -> String {
    if count == 1 {
        format!("{name} could not be moved: {reason}")
    } else {
        format!("{count} images could not be moved. The last: {name}: {reason}")
    }
}

/// The status-line text when an image could not be moved back by an undo.
pub fn undo_failed_text(name: &str, reason: &str) -> String {
    format!("{name} could not be moved back: {reason}")
}

/// The status-line text after an undo moved `count` images back.
pub fn undone_text(count: usize) -> String {
    if count == 1 {
        "1 image moved back.".to_owned()
    } else {
        format!("{count} images moved back.")
    }
}

/// The status-line text when Undo Move is chosen with nothing to undo.
pub fn nothing_to_undo_text() -> &'static str {
    "There is no move to undo."
}

/// The status-line text when Undo Move is chosen while a batch of moves or an undo runs or waits
/// (U9-Q11, T183): nothing is queued.
pub fn move_running_text() -> &'static str {
    "A move is still running. Try Undo Move again when it is done."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_move_texts_with_one_and_with_more() {
        assert_eq!(
            move_failed_text("IMG_001.ARW", "the file is open in another program"),
            "IMG_001.ARW could not be moved: the file is open in another program"
        );
        assert_eq!(
            moves_failed_text(1, "IMG_001.ARW", "no access"),
            "IMG_001.ARW could not be moved: no access"
        );
        assert_eq!(
            moves_failed_text(3, "IMG_003.ARW", "no access"),
            "3 images could not be moved. The last: IMG_003.ARW: no access"
        );
        assert_eq!(
            undo_failed_text("IMG_001.ARW", "the name is taken"),
            "IMG_001.ARW could not be moved back: the name is taken"
        );
        assert_eq!(undone_text(1), "1 image moved back.");
        assert_eq!(undone_text(2), "2 images moved back.");
        assert_eq!(undone_text(12), "12 images moved back.");
        assert_eq!(nothing_to_undo_text(), "There is no move to undo.");
    }

    #[test]
    fn the_text_while_a_move_runs() {
        assert_eq!(
            move_running_text(),
            "A move is still running. Try Undo Move again when it is done."
        );
    }
}
