//! The texts of opening the chosen images in another program (ADR 0019 §2.3–§2.6, T201;
//! `docs/tasks/README.md` rule 13): whole sentences, each case its own template, the singular and
//! the plural in the same function. `reason` is a system error text and is not followed by a full
//! stop at the end of a text, since it may end in one.

/// The Image menu's item for one program (ADR 0019 E-Q3).
pub fn open_in_text(name: &str) -> String {
    format!("Open in {name}")
}

/// The Image menu's item while no program is set up.
pub fn open_in_another_text() -> &'static str {
    "Open in Another Program…"
}

/// The status line while a request waits for the images' writes (ADR 0019 E-Q6).
pub fn waiting_for_writes_text() -> &'static str {
    "Waiting for writes …"
}

/// The question before more than `MANY_IMAGES` images are opened (ADR 0019 E-Q8).
pub fn open_many_question_text(count: usize, name: &str) -> String {
    if count == 1 {
        format!("Open 1 image in {name}?")
    } else {
        format!("Open {count} images in {name}?")
    }
}

/// The question's button that opens the images.
pub fn open_button_text() -> &'static str {
    "Open"
}

/// The question's button that opens nothing.
pub fn open_cancel_text() -> &'static str {
    "Cancel"
}

/// The status-line text after the program was started with `count` images.
pub fn opened_text(count: usize, name: &str) -> String {
    if count == 1 {
        format!("1 image opened in {name}.")
    } else {
        format!("{count} images opened in {name}.")
    }
}

/// The status-line text when the command line for `count` images would be longer than Windows
/// allows; `fit` is how many of them would fit.
pub fn too_long_text(count: usize, name: &str, fit: usize) -> String {
    if count <= 1 {
        format!(
            "The image cannot be opened in {name}: the command line would be longer than Windows \
             allows."
        )
    } else if fit == 0 {
        format!(
            "{count} images cannot be opened in {name} at once: the command line would be longer \
             than Windows allows. Not even one image fits."
        )
    } else if fit == 1 {
        format!(
            "{count} images cannot be opened in {name} at once: the command line would be longer \
             than Windows allows. At most 1 image fits."
        )
    } else {
        format!(
            "{count} images cannot be opened in {name} at once: the command line would be longer \
             than Windows allows. At most {fit} images fit."
        )
    }
}

/// The status-line text when the program could not be started, with the system's reason.
pub fn start_failed_text(name: &str, reason: &str) -> String {
    format!("{name} could not be started: {reason}")
}

/// The status-line text when the program's file does not exist (ADR 0019 E-Q9).
pub fn program_missing_text(name: &str, path: &str) -> String {
    format!("{name} could not be started: {path} does not exist.")
}

/// The status-line text when an image (its file name `name`) was moved or deleted while its
/// request waited.
pub fn image_gone_text(name: &str) -> String {
    format!("{name} was not opened: it is no longer in its folder.")
}

/// The status-line text when none of the images is left to open.
pub fn nothing_to_open_text() -> &'static str {
    "Nothing was opened: the images are no longer in their folders."
}

/// The name of the command for program `number` when its name is not at hand (not shown in the
/// menu).
pub fn program_number_text(number: usize) -> String {
    format!("Open in Program {number}")
}

/// The status-line text when File Explorer could not be started (ADR 0019 §2.8, T203), with the
/// system's reason.
pub fn explorer_failed_text(reason: &str) -> String {
    format!("File Explorer could not be started: {reason}")
}

/// The status-line text when the current image (its file name `name`) is no longer a file, so
/// File Explorer is not started.
pub fn explorer_missing_text(name: &str) -> String {
    format!("{name} is no longer in its folder.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_texts_without_a_count() {
        assert_eq!(open_in_text("Editor"), "Open in Editor");
        assert_eq!(open_in_another_text(), "Open in Another Program…");
        assert_eq!(waiting_for_writes_text(), "Waiting for writes …");
        assert_eq!(open_button_text(), "Open");
        assert_eq!(open_cancel_text(), "Cancel");
        assert_eq!(
            start_failed_text("Editor", "Access is denied. (os error 5)"),
            "Editor could not be started: Access is denied. (os error 5)"
        );
        assert_eq!(
            program_missing_text("Editor", r"C:\Tools\editor.exe"),
            r"Editor could not be started: C:\Tools\editor.exe does not exist."
        );
        assert_eq!(
            image_gone_text("a.ARW"),
            "a.ARW was not opened: it is no longer in its folder."
        );
        assert_eq!(
            nothing_to_open_text(),
            "Nothing was opened: the images are no longer in their folders."
        );
        assert_eq!(program_number_text(2), "Open in Program 2");
    }

    #[test]
    fn the_question_counts_images() {
        assert_eq!(
            open_many_question_text(1, "Editor"),
            "Open 1 image in Editor?"
        );
        assert_eq!(
            open_many_question_text(150, "Editor"),
            "Open 150 images in Editor?"
        );
    }

    #[test]
    fn opened_counts_images() {
        assert_eq!(opened_text(1, "Editor"), "1 image opened in Editor.");
        assert_eq!(opened_text(3, "Editor"), "3 images opened in Editor.");
    }

    #[test]
    fn too_long_has_four_templates() {
        assert_eq!(
            too_long_text(400, "Editor", 120),
            "400 images cannot be opened in Editor at once: the command line would be longer \
             than Windows allows. At most 120 images fit."
        );
        assert_eq!(
            too_long_text(3, "Editor", 1),
            "3 images cannot be opened in Editor at once: the command line would be longer than \
             Windows allows. At most 1 image fits."
        );
        assert_eq!(
            too_long_text(3, "Editor", 0),
            "3 images cannot be opened in Editor at once: the command line would be longer than \
             Windows allows. Not even one image fits."
        );
        let one = "The image cannot be opened in Editor: the command line would be longer than \
                   Windows allows.";
        assert_eq!(too_long_text(1, "Editor", 0), one);
        assert_eq!(too_long_text(1, "Editor", 1), one);
        assert_eq!(too_long_text(0, "Editor", 0), one);
    }

    #[test]
    fn the_file_explorer_texts() {
        assert_eq!(
            explorer_failed_text("The system cannot find the file specified. (os error 2)"),
            "File Explorer could not be started: The system cannot find the file specified. \
             (os error 2)"
        );
        assert_eq!(
            explorer_missing_text("a.ARW"),
            "a.ARW is no longer in its folder."
        );
    }
}
