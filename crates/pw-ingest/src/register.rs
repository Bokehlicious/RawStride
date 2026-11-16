//! The ingest register: RawStride's own text file in the root of the card, listing the files an
//! earlier import already took (ADR 0005 choice 6). Files in it are left out of the next import,
//! whatever the destination and the names were then, and a line is added after each verified
//! file. It is the one file RawStride writes on the source.
//!
//! The file is UTF-8. Its first line is the version line, then one line per file:
//! `{path}\t{size}\t{modified}`, with the path below the register's root (`/` between the parts),
//! the size in bytes and the modification time in whole seconds since 1970 (UTC).

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use crate::{COMPANION_EXTENSIONS, Shot, SourceFile, drive_of, secs};

/// The register's file name, in the root of the card.
pub const REGISTER_FILE: &str = ".rawstride-ingest.txt";

/// The first line of a register this version reads and writes.
const VERSION_LINE: &str = "RawStride ingest register 1";

/// The text for a register that could not be opened, read or written.
fn mark_error(error: &dyn std::fmt::Display) -> String {
    format!("The card could not be marked as imported: {error}.")
}

/// The text for a register file with another version line.
fn foreign_error() -> String {
    format!(
        "The card could not be marked as imported: its file {REGISTER_FILE} was not made by this version of RawStride."
    )
}

/// The folder that holds the register of `source`: the root of its drive (`E:\` for `E:\DCIM`).
/// A source without a drive letter has no register.
pub fn register_root(source: &Path) -> Option<PathBuf> {
    drive_of(source).map(|drive| PathBuf::from(format!("{drive}\\")))
}

/// The path of `file` below `root` as the register writes it (`/` between the parts), or `None`
/// for a file that has no place in the register: one not below `root`, or with a name that is
/// not Unicode or holds a tab or a line break.
fn path_below(root: &Path, file: &Path) -> Option<String> {
    let mut rest = file.components();
    for part in root.components() {
        if !same_part(part, rest.next()?) {
            return None;
        }
    }
    let mut parts = Vec::new();
    for part in rest {
        let Component::Normal(name) = part else {
            return None;
        };
        parts.push(name.to_str()?);
    }
    if parts.is_empty() {
        return None;
    }
    let path = parts.join("/");
    if path.contains(['\t', '\n', '\r']) {
        return None;
    }
    Some(path)
}

/// Whether two parts of a path are the same without regard to case (Windows).
fn same_part(a: Component, b: Component) -> bool {
    a.as_os_str().to_string_lossy().to_lowercase() == b.as_os_str().to_string_lossy().to_lowercase()
}

/// One line of the register as it is compared: lower-case path, size, seconds.
type Entry = (String, u64, u64);

fn parse_line(line: &str) -> Option<Entry> {
    let mut fields = line.split('\t');
    let (Some(path), Some(size), Some(modified), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return None;
    };
    if path.is_empty() {
        return None;
    }
    Some((
        path.to_lowercase(),
        size.parse().ok()?,
        modified.parse().ok()?,
    ))
}

/// What the register on a card says was imported earlier.
#[derive(Debug, Default)]
pub struct Register {
    entries: HashSet<Entry>,
    foreign: bool,
    read_error: Option<String>,
}

impl Register {
    /// True when the file's first line is not this version's: it is then neither read nor
    /// written.
    pub fn is_foreign(&self) -> bool {
        self.foreign
    }

    /// The error of a register file that exists but could not be read.
    pub fn read_error(&self) -> Option<&str> {
        self.read_error.as_deref()
    }

    /// The text [`RegisterWriter::open`] would refuse this register with (foreign or unreadable),
    /// without opening the file.
    pub fn problem(&self) -> Option<String> {
        if self.foreign {
            Some(foreign_error())
        } else {
            self.read_error.as_ref().map(|error| mark_error(error))
        }
    }

    /// Whether `file` is in the register: the same path below `root` (without regard to case),
    /// the same size and the same modification time to the second. A file not below `root`, or
    /// with a time before 1970, is never in it.
    pub fn contains(&self, root: &Path, file: &SourceFile) -> bool {
        match (path_below(root, &file.path), secs(file.modified)) {
            (Some(path), Some(modified)) => {
                self.entries
                    .contains(&(path.to_lowercase(), file.size, modified))
            }
            _ => false,
        }
    }
}

/// Reads the register in `root`. No file, or an empty one, gives an empty register; so does a
/// file that cannot be read ([`Register::read_error`]) or has another version line
/// ([`Register::is_foreign`]). A line that does not parse is skipped.
pub fn read_register(root: &Path) -> Register {
    let bytes = match fs::read(root.join(REGISTER_FILE)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Register::default(),
        Err(error) => {
            return Register {
                read_error: Some(error.to_string()),
                ..Register::default()
            };
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text.lines();
    match lines.next() {
        None => Register::default(),
        Some(VERSION_LINE) => Register {
            entries: lines.filter_map(parse_line).collect(),
            ..Register::default()
        },
        Some(_) => Register {
            foreign: true,
            ..Register::default()
        },
    }
}

fn is_companion(file: &SourceFile) -> bool {
    COMPANION_EXTENSIONS.contains(&file.ext.to_ascii_lowercase().as_str())
}

/// Leaves out the shots an earlier import took: gives the shots that are not in the register (in
/// the same order) and the number of files left out. A shot is left out only when every image of
/// it is in the register; its companions go with it and are counted.
pub fn new_shots(shots: &[Shot], root: &Path, register: &Register) -> (Vec<Shot>, usize) {
    let mut new = Vec::new();
    let mut left_out = 0;
    for shot in shots {
        let mut images = shot.files.iter().filter(|f| !is_companion(f)).peekable();
        if images.peek().is_some() && images.all(|f| register.contains(root, f)) {
            left_out += shot.files.len();
        } else {
            new.push(shot.clone());
        }
    }
    (new, left_out)
}

/// Adds lines to the register of one card.
#[derive(Debug)]
pub struct RegisterWriter {
    root: PathBuf,
    file: File,
}

/// Whether the start of a register file is this version's line.
fn starts_with_version(head: &[u8]) -> bool {
    match head.strip_prefix(VERSION_LINE.as_bytes()) {
        Some(rest) => matches!(rest.first(), None | Some(b'\n' | b'\r')),
        None => false,
    }
}

/// Opens the register file for appending, ready for a new line: created with its version line
/// when there is none (or it is empty), and ended with a line break. `Ok(None)` is a file with
/// another version line, which is left as it is.
fn open_file(path: &Path) -> io::Result<Option<File>> {
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)?;
    if file.metadata()?.len() == 0 {
        file.write_all(format!("{VERSION_LINE}\n").as_bytes())?;
    } else {
        let mut head = Vec::new();
        file.seek(SeekFrom::Start(0))?;
        Read::by_ref(&mut file)
            .take(VERSION_LINE.len() as u64 + 1)
            .read_to_end(&mut head)?;
        if !starts_with_version(&head) {
            return Ok(None);
        }
        let mut last = [0u8; 1];
        file.seek(SeekFrom::End(-1))?;
        file.read_exact(&mut last)?;
        if last != *b"\n" {
            file.write_all(b"\n")?;
        }
    }
    file.flush()?;
    file.sync_data()?;
    Ok(Some(file))
}

impl RegisterWriter {
    /// Opens the register in `root` for adding lines. Refuses a foreign or unreadable register
    /// (the text of [`Register::problem`]), and fails when the file cannot be created or opened
    /// for appending (a read-only file, a locked card): the card then cannot be marked.
    pub fn open(root: &Path, register: &Register) -> Result<RegisterWriter, String> {
        if let Some(problem) = register.problem() {
            return Err(problem);
        }
        match open_file(&root.join(REGISTER_FILE)) {
            Ok(Some(file)) => Ok(RegisterWriter {
                root: root.to_path_buf(),
                file,
            }),
            Ok(None) => Err(foreign_error()),
            Err(error) => Err(mark_error(&error)),
        }
    }

    /// Adds one file and makes sure the line is on the card, so that a card pulled out loses at
    /// most the line being written. Writes nothing (and is `Ok`) for a file that has no place in
    /// the register: not below the root, a tab or line break in its path, or a time before 1970.
    pub fn add(&mut self, file: &Path, size: u64, modified: SystemTime) -> Result<(), String> {
        let (Some(path), Some(modified)) = (path_below(&self.root, file), secs(modified)) else {
            return Ok(());
        };
        self.write_line(&format!("{path}\t{size}\t{modified}\n"))
            .map_err(|error| mark_error(&error))
    }

    fn write_line(&mut self, line: &str) -> io::Result<()> {
        self.file.write_all(line.as_bytes())?;
        self.file.flush()?;
        self.file.sync_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, UNIX_EPOCH};

    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

    /// A temporary folder used as the register's root, removed when dropped.
    struct Root(PathBuf);

    impl Root {
        fn new() -> Root {
            let id = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("pw-ingest-register-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Root(path)
        }

        fn with(text: &str) -> Root {
            let root = Root::new();
            fs::write(root.file(), text).unwrap();
            root
        }

        fn file(&self) -> PathBuf {
            self.0.join(REGISTER_FILE)
        }

        fn text(&self) -> String {
            fs::read_to_string(self.file()).unwrap()
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn source(root: &Path, rel: &str, size: u64, secs: u64) -> SourceFile {
        let path = root.join(rel);
        SourceFile {
            ext: path
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            size,
            modified: at(secs),
        }
    }

    fn shot(stem: &str, files: Vec<SourceFile>) -> Shot {
        Shot {
            stem: stem.into(),
            files,
            time: None,
            time_from_exif: false,
            make: None,
            model: None,
        }
    }

    const GOOD: &str = "RawStride ingest register 1\n\
        DCIM/100MSDCF/A001.ARW\t5000\t100\n\
        DCIM/100MSDCF/A002.JPG\t3000\t160\n";

    #[test]
    fn register_root_is_the_root_of_the_drive() {
        assert_eq!(
            register_root(Path::new("E:\\DCIM")),
            Some(PathBuf::from("E:\\"))
        );
        assert_eq!(register_root(Path::new("e:")), Some(PathBuf::from("E:\\")));
        assert_eq!(register_root(Path::new("\\\\server\\share")), None);
        assert_eq!(register_root(Path::new("/media/card")), None);
    }

    #[test]
    fn no_file_is_an_empty_register() {
        let root = Root::new();
        let register = read_register(&root.0);
        assert!(!register.is_foreign());
        assert_eq!(register.read_error(), None);
        assert_eq!(register.problem(), None);
        assert!(!register.contains(&root.0, &source(&root.0, "DCIM/A001.ARW", 1, 1)));
        assert!(!root.file().exists(), "reading writes nothing");
    }

    #[test]
    fn a_good_file_is_read() {
        let root = Root::with(GOOD);
        let register = read_register(&root.0);
        assert!(!register.is_foreign());
        assert_eq!(register.problem(), None);
        assert!(register.contains(
            &root.0,
            &source(&root.0, "DCIM/100MSDCF/A001.ARW", 5000, 100)
        ));
        assert!(register.contains(
            &root.0,
            &source(&root.0, "DCIM/100MSDCF/A002.JPG", 3000, 160)
        ));
        assert!(!register.contains(
            &root.0,
            &source(&root.0, "DCIM/100MSDCF/A003.ARW", 5000, 100)
        ));
    }

    #[test]
    fn a_bad_line_in_the_middle_is_skipped() {
        let root = Root::with(
            "RawStride ingest register 1\n\
             DCIM/A001.ARW\t5000\t100\n\
             DCIM/A002.ARW\tmany\t100\n\
             DCIM/A003.ARW\t5000\n\
             \n\
             DCIM/A004.ARW\t5000\t100\textra\n\
             DCIM/A005.ARW\t7000\t220\n",
        );
        let register = read_register(&root.0);
        assert!(!register.is_foreign());
        assert!(register.contains(&root.0, &source(&root.0, "DCIM/A001.ARW", 5000, 100)));
        assert!(register.contains(&root.0, &source(&root.0, "DCIM/A005.ARW", 7000, 220)));
        assert!(!register.contains(&root.0, &source(&root.0, "DCIM/A002.ARW", 5000, 100)));
        assert!(!register.contains(&root.0, &source(&root.0, "DCIM/A003.ARW", 5000, 100)));
        assert!(!register.contains(&root.0, &source(&root.0, "DCIM/A004.ARW", 5000, 100)));
    }

    #[test]
    fn another_first_line_is_foreign() {
        let root = Root::with("RawStride ingest register 2\nDCIM/A001.ARW\t5000\t100\n");
        let register = read_register(&root.0);
        assert!(register.is_foreign());
        assert_eq!(register.read_error(), None);
        assert!(
            !register.contains(&root.0, &source(&root.0, "DCIM/A001.ARW", 5000, 100)),
            "a foreign register is empty"
        );
        assert_eq!(
            register.problem().as_deref(),
            Some(
                "The card could not be marked as imported: its file .rawstride-ingest.txt was not made by this version of RawStride."
            )
        );
    }

    #[test]
    fn an_empty_file_is_an_empty_register_not_a_foreign_one() {
        let root = Root::with("");
        let register = read_register(&root.0);
        assert!(!register.is_foreign());
        assert_eq!(register.problem(), None);
    }

    #[test]
    fn an_unreadable_file_gives_the_general_text() {
        // A folder with the register's name exists but cannot be read as a file.
        let root = Root::new();
        fs::create_dir(root.file()).unwrap();
        let register = read_register(&root.0);
        assert!(!register.is_foreign());
        let error = register.read_error().unwrap().to_string();
        assert_eq!(
            register.problem(),
            Some(format!(
                "The card could not be marked as imported: {error}."
            ))
        );
        assert_eq!(
            RegisterWriter::open(&root.0, &register).unwrap_err(),
            format!("The card could not be marked as imported: {error}.")
        );
    }

    #[test]
    fn the_path_matches_without_regard_to_case() {
        let root = Root::with(GOOD);
        let register = read_register(&root.0);
        assert!(register.contains(
            &root.0,
            &source(&root.0, "dcim/100msdcf/a001.arw", 5000, 100)
        ));
        assert!(register.contains(
            &root.0,
            &source(&root.0, "Dcim/100MSDCF/A001.Arw", 5000, 100)
        ));
    }

    #[test]
    fn another_size_or_time_does_not_match() {
        let root = Root::with(GOOD);
        let register = read_register(&root.0);
        assert!(!register.contains(
            &root.0,
            &source(&root.0, "DCIM/100MSDCF/A001.ARW", 5001, 100)
        ));
        assert!(!register.contains(
            &root.0,
            &source(&root.0, "DCIM/100MSDCF/A001.ARW", 5000, 101)
        ));
    }

    #[test]
    fn a_file_outside_the_root_or_before_1970_is_never_contained() {
        let root = Root::with("RawStride ingest register 1\nA001.ARW\t5000\t0\n");
        let register = read_register(&root.0);
        assert!(register.contains(&root.0, &source(&root.0, "A001.ARW", 5000, 0)));
        let other = Root::new();
        assert!(!register.contains(&root.0, &source(&other.0, "A001.ARW", 5000, 0)));
        let mut early = source(&root.0, "A001.ARW", 5000, 0);
        early.modified = UNIX_EPOCH - Duration::from_secs(10);
        assert!(!register.contains(&root.0, &early));
    }

    #[test]
    fn a_shot_is_left_out_when_all_its_images_are_in() {
        let root = Root::with(GOOD);
        let register = read_register(&root.0);
        let shots = [shot(
            "A001",
            vec![
                source(&root.0, "DCIM/100MSDCF/A001.ARW", 5000, 100),
                source(&root.0, "DCIM/100MSDCF/A001.xmp", 12, 105),
            ],
        )];
        let (new, left_out) = new_shots(&shots, &root.0, &register);
        assert!(new.is_empty());
        assert_eq!(left_out, 2, "the companion is counted");
    }

    #[test]
    fn a_shot_is_kept_when_one_of_its_images_is_missing() {
        // The RAW is in the register, its JPEG is not.
        let root = Root::with(GOOD);
        let register = read_register(&root.0);
        let shots = [shot(
            "A001",
            vec![
                source(&root.0, "DCIM/100MSDCF/A001.ARW", 5000, 100),
                source(&root.0, "DCIM/100MSDCF/A001.JPG", 2000, 100),
            ],
        )];
        let (new, left_out) = new_shots(&shots, &root.0, &register);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].files.len(), 2, "the whole shot is kept");
        assert_eq!(left_out, 0);
    }

    #[test]
    fn a_companion_alone_does_not_keep_a_shot() {
        // The sidecar is not in the register (and has an upper-case extension); the image is.
        let root = Root::with(GOOD);
        let register = read_register(&root.0);
        let shots = [shot(
            "A002",
            vec![
                source(&root.0, "DCIM/100MSDCF/A002.JPG", 3000, 160),
                source(&root.0, "DCIM/100MSDCF/A002.XMP", 99, 999),
            ],
        )];
        let (new, left_out) = new_shots(&shots, &root.0, &register);
        assert!(new.is_empty());
        assert_eq!(left_out, 2);
    }

    #[test]
    fn new_shots_keep_their_order_and_count_the_files_left_out() {
        let root = Root::with(GOOD);
        let register = read_register(&root.0);
        let shots = [
            shot(
                "B001",
                vec![source(&root.0, "DCIM/100MSDCF/B001.ARW", 1, 50)],
            ),
            shot(
                "A001",
                vec![
                    source(&root.0, "DCIM/100MSDCF/A001.ARW", 5000, 100),
                    source(&root.0, "DCIM/100MSDCF/A001.xmp", 12, 105),
                ],
            ),
            shot(
                "B002",
                vec![source(&root.0, "DCIM/100MSDCF/B002.ARW", 1, 120)],
            ),
            shot(
                "A002",
                vec![source(&root.0, "DCIM/100MSDCF/A002.JPG", 3000, 160)],
            ),
            shot(
                "B003",
                vec![source(&root.0, "DCIM/100MSDCF/B003.ARW", 1, 200)],
            ),
        ];
        let (new, left_out) = new_shots(&shots, &root.0, &register);
        let stems: Vec<_> = new.iter().map(|s| s.stem.as_str()).collect();
        assert_eq!(stems, vec!["B001", "B002", "B003"]);
        assert_eq!(left_out, 3);
    }

    #[test]
    fn a_new_file_gets_the_version_line() {
        let root = Root::new();
        let mut writer = RegisterWriter::open(&root.0, &read_register(&root.0)).unwrap();
        assert_eq!(root.text(), "RawStride ingest register 1\n");
        writer
            .add(&root.0.join("DCIM").join("A001.ARW"), 5000, at(100))
            .unwrap();
        assert_eq!(
            root.text(),
            "RawStride ingest register 1\nDCIM/A001.ARW\t5000\t100\n"
        );
        let register = read_register(&root.0);
        assert!(register.contains(&root.0, &source(&root.0, "DCIM/A001.ARW", 5000, 100)));
    }

    #[test]
    fn an_empty_file_gets_the_version_line() {
        let root = Root::with("");
        let mut writer = RegisterWriter::open(&root.0, &read_register(&root.0)).unwrap();
        writer.add(&root.0.join("A001.ARW"), 1, at(2)).unwrap();
        assert_eq!(root.text(), "RawStride ingest register 1\nA001.ARW\t1\t2\n");
    }

    #[test]
    fn lines_are_appended_to_an_existing_file() {
        let root = Root::with(GOOD);
        let mut writer = RegisterWriter::open(&root.0, &read_register(&root.0)).unwrap();
        assert_eq!(root.text(), GOOD, "opening changes nothing");
        writer
            .add(&root.0.join("DCIM/100MSDCF/A003.ARW"), 7000, at(220))
            .unwrap();
        // The same line twice is harmless.
        writer
            .add(&root.0.join("DCIM/100MSDCF/A003.ARW"), 7000, at(220))
            .unwrap();
        assert_eq!(
            root.text(),
            format!("{GOOD}DCIM/100MSDCF/A003.ARW\t7000\t220\nDCIM/100MSDCF/A003.ARW\t7000\t220\n")
        );
        let register = read_register(&root.0);
        assert!(register.contains(
            &root.0,
            &source(&root.0, "DCIM/100MSDCF/A001.ARW", 5000, 100)
        ));
        assert!(register.contains(
            &root.0,
            &source(&root.0, "DCIM/100MSDCF/A003.ARW", 7000, 220)
        ));
    }

    #[test]
    fn a_file_without_a_final_line_break_gets_one_first() {
        let root = Root::with("RawStride ingest register 1\nA001.ARW\t5000\t100");
        let mut writer = RegisterWriter::open(&root.0, &read_register(&root.0)).unwrap();
        writer.add(&root.0.join("A002.ARW"), 7, at(8)).unwrap();
        assert_eq!(
            root.text(),
            "RawStride ingest register 1\nA001.ARW\t5000\t100\nA002.ARW\t7\t8\n"
        );
    }

    #[test]
    fn a_foreign_register_is_refused_and_left_as_it_is() {
        let foreign = "Something else\nA001.ARW\t5000\t100\n";
        let root = Root::with(foreign);
        let text = "The card could not be marked as imported: its file .rawstride-ingest.txt was not made by this version of RawStride.";
        assert_eq!(
            RegisterWriter::open(&root.0, &read_register(&root.0)).unwrap_err(),
            text
        );
        // Also when the register was read before the file became foreign.
        assert_eq!(
            RegisterWriter::open(&root.0, &Register::default()).unwrap_err(),
            text
        );
        assert_eq!(root.text(), foreign);
    }

    #[test]
    fn a_path_with_a_tab_or_outside_the_root_is_not_written() {
        let root = Root::new();
        let other = Root::new();
        let mut writer = RegisterWriter::open(&root.0, &read_register(&root.0)).unwrap();
        writer.add(&root.0.join("A\t001.ARW"), 1, at(2)).unwrap();
        writer.add(&root.0.join("A\n001.ARW"), 1, at(2)).unwrap();
        writer.add(&other.0.join("A001.ARW"), 1, at(2)).unwrap();
        writer.add(&root.0, 1, at(2)).unwrap();
        writer
            .add(
                &root.0.join("A001.ARW"),
                1,
                UNIX_EPOCH - Duration::from_secs(10),
            )
            .unwrap();
        assert_eq!(root.text(), "RawStride ingest register 1\n");
    }
}
