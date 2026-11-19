//! Opening the chosen images in another program (ADR 0019 §2.3–§2.6, step E2; T201): how long the
//! command line would be and how many images fit in it, the plan (start, ask first, or too long),
//! the request that waits for the images' writes, and the start of the program. Plain Rust,
//! tested without a window (GR11); the shell wires it to the menu, Ctrl+E and the question
//! (T202). The texts are in `external_text.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::settings::ExternalProgram;

/// Above this many images a question is asked before the program is started (ADR 0019 E-Q8).
pub const MANY_IMAGES: usize = 100;

/// Windows' limit of a command line in UTF-16 units, the ending NUL included.
pub const COMMAND_LINE_LIMIT: usize = 32_767;

/// A path's UTF-16 units, the same on every platform (an unpaired surrogate becomes U+FFFD,
/// still one unit).
fn utf16_units(path: &Path) -> Vec<u16> {
    path.to_string_lossy().encode_utf16().collect()
}

/// The program's part of the command line: the path in double quotes with nothing escaped, and
/// the ending NUL.
fn program_length(program: &Path) -> usize {
    utf16_units(program).len() + 2 + 1
}

/// One image's part of the command line as `std::process::Command` builds it on Windows: one
/// space, then the path, in double quotes when it is empty or holds a space or a tab; a `"`
/// costs n + 1 more units (n the backslashes right before it), and a quoted path's backslashes
/// at its end count twice.
fn argument_length(image: &Path) -> usize {
    let units = utf16_units(image);
    let space = u16::from(b' ');
    let tab = u16::from(b'\t');
    let quote = u16::from(b'"');
    let backslash = u16::from(b'\\');
    let quoted = units.is_empty() || units.iter().any(|&unit| unit == space || unit == tab);
    let mut length = 1 + units.len();
    let mut backslashes = 0;
    for &unit in &units {
        if unit == backslash {
            backslashes += 1;
        } else {
            if unit == quote {
                length += backslashes + 1;
            }
            backslashes = 0;
        }
    }
    if quoted {
        length += 2 + backslashes;
    }
    length
}

/// The length in UTF-16 units of the command line that starts `program` with `images`, each its
/// own argument, as `std::process::Command` builds it on Windows, the ending NUL included. No
/// I/O. A `.bat` or `.cmd` program is measured the same way, though `std` starts it otherwise
/// (Known limit of T201).
pub fn command_line_length(program: &Path, images: &[PathBuf]) -> usize {
    program_length(program)
        + images
            .iter()
            .map(|image| argument_length(image))
            .sum::<usize>()
}

/// The largest `n` such that `program` with the first `n` of `images` fits in
/// `COMMAND_LINE_LIMIT`; 0 when not even the program alone fits.
pub fn images_that_fit(program: &Path, images: &[PathBuf]) -> usize {
    let mut length = program_length(program);
    if length > COMMAND_LINE_LIMIT {
        return 0;
    }
    let mut fit = 0;
    for image in images {
        length += argument_length(image);
        if length > COMMAND_LINE_LIMIT {
            break;
        }
        fit += 1;
    }
    fit
}

/// The images to open in a program: the shell's resolved, absolute paths, in the tab's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenRequest {
    pub program: ExternalProgram,
    pub images: Vec<PathBuf>,
}

impl OpenRequest {
    /// True when no image has a write waiting (ADR 0019 §2.4); `is_waiting` tells it for one
    /// image.
    pub fn ready(&self, is_waiting: impl Fn(&Path) -> bool) -> bool {
        !self.images.iter().any(|image| is_waiting(image))
    }
}

/// What "Open in {name}" does with the chosen images (ADR 0019 §2.3, E-Q8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenPlan {
    /// The program can be started at once (after the images' writes).
    Start(OpenRequest),
    /// More than `MANY_IMAGES` images: the user is asked first.
    Ask(OpenRequest),
    /// The command line would be longer than `COMMAND_LINE_LIMIT`: nothing is started. `count`
    /// is the number of images and `fit` how many of them would fit.
    TooLong { count: usize, fit: usize },
}

/// The plan for opening `images` in `program`: `TooLong` when the command line would be longer
/// than `COMMAND_LINE_LIMIT`, whatever the number; otherwise `Ask` above `MANY_IMAGES` images;
/// otherwise `Start` (also with no images, which the shell never asks for).
pub fn plan_open(program: ExternalProgram, images: Vec<PathBuf>) -> OpenPlan {
    if command_line_length(&program.path, &images) > COMMAND_LINE_LIMIT {
        let fit = images_that_fit(&program.path, &images);
        return OpenPlan::TooLong {
            count: images.len(),
            fit,
        };
    }
    let request = OpenRequest { program, images };
    if request.images.len() > MANY_IMAGES {
        OpenPlan::Ask(request)
    } else {
        OpenPlan::Start(request)
    }
}

/// Where "Open in {name}" is: nothing, the question waiting for the user, or a request waiting
/// for the images' writes. One request at a time; a new command while one waits replaces it
/// (ADR 0019 §2.4; the shell does the replacing, T202).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OpenStep {
    #[default]
    Idle,
    Confirming(OpenRequest),
    Waiting(OpenRequest),
}

impl OpenStep {
    /// Whether a request is held (not `Idle`).
    pub fn busy(&self) -> bool {
        !matches!(self, Self::Idle)
    }
}

/// Why a program could not be started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartError {
    /// The program's path is not a file (ADR 0019 E-Q9).
    Missing,
    /// The start failed; the system's reason.
    Failed(String),
}

/// Starts the request's program with every image its own argument (ADR 0019 E-Q7), without a
/// working folder or window flags. The program is not waited for or watched. `Missing` when the
/// program's path is not a file (a folder is missing too). The length is not checked again: the
/// request comes from `plan_open`.
pub fn start_program(request: &OpenRequest) -> Result<(), StartError> {
    if !request.program.path.is_file() {
        return Err(StartError::Missing);
    }
    Command::new(&request.program.path)
        .args(&request.images)
        .spawn()
        .map(|_child| ())
        .map_err(|error| StartError::Failed(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM: &str = r"C:\p.exe";

    fn program(path: &str) -> ExternalProgram {
        ExternalProgram {
            name: "Editor".to_owned(),
            path: PathBuf::from(path),
        }
    }

    /// `count` paths of `length` UTF-16 units each, without spaces.
    fn long_paths(count: usize, length: usize) -> Vec<PathBuf> {
        (0..count)
            .map(|index| {
                let name = format!(r"C:\{index:05}");
                let pad = "x".repeat(length.saturating_sub(name.len()));
                PathBuf::from(format!("{name}{pad}"))
            })
            .collect()
    }

    #[test]
    fn the_program_alone_is_quoted_and_ends_in_a_nul() {
        assert_eq!(command_line_length(Path::new(PROGRAM), &[]), 8 + 2 + 1);
    }

    #[test]
    fn a_path_without_spaces_is_not_quoted() {
        let images = [PathBuf::from(r"C:\photos\a.jpg")];
        assert_eq!(
            command_line_length(Path::new(PROGRAM), &images),
            11 + 1 + 15
        );
    }

    #[test]
    fn a_path_with_a_space_is_quoted() {
        let images = [PathBuf::from(r"C:\my photos\a.jpg")];
        assert_eq!(
            command_line_length(Path::new(PROGRAM), &images),
            11 + 1 + 18 + 2
        );
    }

    #[test]
    fn a_non_ascii_letter_is_one_unit_and_a_character_outside_the_bmp_two() {
        let letter = [PathBuf::from(r"C:\å.jpg")];
        assert_eq!(command_line_length(Path::new(PROGRAM), &letter), 11 + 1 + 8);
        let outside = [PathBuf::from("C:\\\u{1F600}.jpg")];
        assert_eq!(
            command_line_length(Path::new(PROGRAM), &outside),
            11 + 1 + 9
        );
    }

    #[test]
    fn a_quote_and_the_backslashes_at_the_end_of_a_quoted_path_are_escaped() {
        // `a\"b`: the quote costs 1 + 1 (one backslash before it), not quoted
        assert_eq!(argument_length(Path::new("a\\\"b")), 1 + 4 + 2);
        // `a b\\`: quoted, its two ending backslashes count twice
        assert_eq!(argument_length(Path::new("a b\\\\")), 1 + 5 + 2 + 2);
        // an empty path is quoted
        assert_eq!(argument_length(Path::new("")), 1 + 2);
    }

    #[test]
    fn all_images_fit() {
        let images = long_paths(10, 100);
        assert_eq!(images_that_fit(Path::new(PROGRAM), &images), 10);
    }

    #[test]
    fn exactly_n_images_fit() {
        // each image costs 10 001 units; three of them and a program of 2 761 units reach the
        // limit exactly
        let images = long_paths(5, 10_000);
        let program = format!(r"C:\{}", "p".repeat(2_761 - 3));
        let program = Path::new(&program);
        assert_eq!(
            command_line_length(program, &images[..3]),
            COMMAND_LINE_LIMIT
        );
        assert!(command_line_length(program, &images[..4]) > COMMAND_LINE_LIMIT);
        assert_eq!(images_that_fit(program, &images), 3);
    }

    #[test]
    fn no_image_fits_when_the_program_alone_is_too_long() {
        let program = format!(r"C:\{}", "p".repeat(COMMAND_LINE_LIMIT));
        let images = long_paths(2, 10);
        assert_eq!(images_that_fit(Path::new(&program), &images), 0);
        assert_eq!(images_that_fit(Path::new(&program), &[]), 0);
    }

    #[test]
    fn a_hundred_images_start_and_more_ask_first() {
        let hundred = long_paths(100, 20);
        assert_eq!(
            plan_open(program(PROGRAM), hundred.clone()),
            OpenPlan::Start(OpenRequest {
                program: program(PROGRAM),
                images: hundred,
            })
        );
        let more = long_paths(101, 20);
        assert_eq!(
            plan_open(program(PROGRAM), more.clone()),
            OpenPlan::Ask(OpenRequest {
                program: program(PROGRAM),
                images: more,
            })
        );
        assert_eq!(
            plan_open(program(PROGRAM), Vec::new()),
            OpenPlan::Start(OpenRequest {
                program: program(PROGRAM),
                images: Vec::new(),
            })
        );
    }

    #[test]
    fn a_command_line_too_long_is_refused_with_how_many_fit() {
        // 50 images of 1 001 units: (32 767 - 11) / 1 001 = 32 fit
        assert_eq!(
            plan_open(program(PROGRAM), long_paths(50, 1_000)),
            OpenPlan::TooLong { count: 50, fit: 32 }
        );
        // above 100 images too: 150 of 301 units, (32 767 - 11) / 301 = 108 fit
        assert_eq!(
            plan_open(program(PROGRAM), long_paths(150, 300)),
            OpenPlan::TooLong {
                count: 150,
                fit: 108,
            }
        );
    }

    #[test]
    fn a_request_is_ready_when_no_image_waits() {
        let images = long_paths(3, 20);
        let waiting = images.get(1).cloned();
        let request = OpenRequest {
            program: program(PROGRAM),
            images,
        };
        assert!(!request.ready(|image| Some(image) == waiting.as_deref()));
        assert!(request.ready(|_| false));
    }

    #[test]
    fn open_step_is_busy_unless_idle() {
        let request = OpenRequest {
            program: program(PROGRAM),
            images: long_paths(1, 20),
        };
        assert!(!OpenStep::default().busy());
        assert!(OpenStep::Confirming(request.clone()).busy());
        assert!(OpenStep::Waiting(request).busy());
    }

    fn test_folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("pw-loupe-external-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&folder)
            .unwrap_or_else(|error| panic!("could not create {}: {error}", folder.display()));
        folder
    }

    #[test]
    fn a_missing_program_or_a_folder_is_missing() {
        let folder = test_folder("missing");
        let gone = OpenRequest {
            program: ExternalProgram {
                name: "Gone".to_owned(),
                path: folder.join("no-such-program.exe"),
            },
            images: Vec::new(),
        };
        assert_eq!(start_program(&gone), Err(StartError::Missing));
        let a_folder = OpenRequest {
            program: ExternalProgram {
                name: "Folder".to_owned(),
                path: folder.clone(),
            },
            images: Vec::new(),
        };
        assert_eq!(start_program(&a_folder), Err(StartError::Missing));
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_file_that_is_not_a_program_fails_to_start() {
        let folder = test_folder("not-a-program");
        let text = folder.join("notes.txt");
        std::fs::write(&text, "not a program")
            .unwrap_or_else(|error| panic!("could not write {}: {error}", text.display()));
        let request = OpenRequest {
            program: ExternalProgram {
                name: "Notes".to_owned(),
                path: text,
            },
            images: Vec::new(),
        };
        assert!(matches!(
            start_program(&request),
            Err(StartError::Failed(reason)) if !reason.is_empty()
        ));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
