//! RawStride's settings file (ADR 0007 §5.2 and §5.4/Q10; owner decision D10): the setting "Skapa
//! IPTC-IIM för kompatibilitet med äldre system" (off by default) and the fallback encoding for IIM
//! and Exif text (Windows-1252 by default). The file is `%LOCALAPPDATA%\RawStride\settings.txt`,
//! next to the write journal, for this user on this computer only: UTF-8 text with one
//! `key = value` per line. Loading never fails: what cannot be read keeps its default and gives a
//! problem text in English for the log. Saving writes a temporary file and renames it onto the
//! settings file, so a crash never leaves half a file. Nothing uses the file yet (T106).

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use pw_metadata::edit::EditOptions;
use pw_metadata::iim::Fallback;
use pw_metadata::write_plan::WriteSettings;

/// The key of the setting "Skapa IPTC-IIM för kompatibilitet med äldre system".
const CREATE_IIM: &str = "create_iim";
/// The key of the fallback encoding.
const FALLBACK_ENCODING: &str = "fallback_encoding";
/// The value of [`FALLBACK_ENCODING`] for Windows-1252.
const WINDOWS_1252: &str = "windows-1252";
/// The value of [`FALLBACK_ENCODING`] for ISO 8859-1 (Latin-1).
const ISO_8859_1: &str = "iso-8859-1";

/// RawStride's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Create IIM in a JPEG, TIFF or DNG that has none (§5.2).
    pub create_iim: bool,
    /// The encoding for IIM and Exif text that is neither ASCII nor UTF-8 (§5.4, Q10).
    pub fallback: Fallback,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            create_iim: false,
            fallback: Fallback::Windows1252,
        }
    }
}

impl Settings {
    /// The options the metadata writes and reads use with these settings.
    pub fn edit_options(&self) -> EditOptions {
        EditOptions {
            settings: WriteSettings {
                create_iim: self.create_iim,
            },
            fallback: self.fallback,
        }
    }
}

/// `%LOCALAPPDATA%\RawStride\settings.txt`; `None` without `LOCALAPPDATA` (or when it is empty).
pub fn settings_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|data| !data.is_empty())
        .map(|data| PathBuf::from(data).join("RawStride").join("settings.txt"))
}

/// Reads the settings file. A missing file gives the defaults and no problem. An unknown key, a
/// bad value or a line without `=` keeps the default for it and gives one problem naming the line
/// number; a file that cannot be read gives the defaults and one problem. The problems are English,
/// for the log (`eprintln!` by the caller).
pub fn load_settings(path: &Path) -> (Settings, Vec<String>) {
    let mut settings = Settings::default();
    let mut problems = Vec::new();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return (settings, problems),
        Err(error) => {
            problems.push(format!(
                "settings file {} could not be read, defaults used: {error}",
                path.display()
            ));
            return (settings, problems);
        }
    };
    // A byte order mark written by an editor is not part of the first key.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            problems.push(format!(
                "settings file {}, line {number}: no '=' in \"{line}\", line ignored",
                path.display()
            ));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            CREATE_IIM => match value {
                "true" => settings.create_iim = true,
                "false" => settings.create_iim = false,
                _ => problems.push(format!(
                    "settings file {}, line {number}: bad value \"{value}\" for {CREATE_IIM} \
                     (expected true or false), default used",
                    path.display()
                )),
            },
            FALLBACK_ENCODING => match parse_fallback(value) {
                Some(fallback) => settings.fallback = fallback,
                None => problems.push(format!(
                    "settings file {}, line {number}: bad value \"{value}\" for \
                     {FALLBACK_ENCODING} (expected {WINDOWS_1252} or {ISO_8859_1}), default used",
                    path.display()
                )),
            },
            _ => problems.push(format!(
                "settings file {}, line {number}: unknown key \"{key}\", line ignored",
                path.display()
            )),
        }
    }
    (settings, problems)
}

/// Writes the settings: creates the folder, writes both keys (after the comment line
/// `# RawStride`) to `<file name>.tmp` in the same folder, then renames it onto the settings file
/// (which replaces it on Windows), so a crash never leaves half a file.
pub fn save_settings(path: &Path, settings: &Settings) -> io::Result<()> {
    let Some(file_name) = path.file_name() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("settings path {} has no file name", path.display()),
        ));
    };
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut temp_name = file_name.to_os_string();
    temp_name.push(".tmp");
    let temp = path.with_file_name(temp_name);
    let text = format!(
        "# RawStride\n{CREATE_IIM} = {}\n{FALLBACK_ENCODING} = {}\n",
        settings.create_iim,
        fallback_value(settings.fallback)
    );
    let written = write_synced(&temp, text.as_bytes()).and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        // Best effort: the error of the write or the rename is the one reported.
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// Writes the bytes to a new file and waits until they are on the disk, so the rename that follows
/// never puts an incomplete file in place.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The file value of an encoding.
fn fallback_value(fallback: Fallback) -> &'static str {
    match fallback {
        Fallback::Windows1252 => WINDOWS_1252,
        Fallback::Latin1 => ISO_8859_1,
    }
}

/// The encoding of a file value; `None` for an unknown value.
fn parse_fallback(value: &str) -> Option<Fallback> {
    match value {
        WINDOWS_1252 => Some(Fallback::Windows1252),
        ISO_8859_1 => Some(Fallback::Latin1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use pw_metadata::edit::EditOptions;
    use pw_metadata::iim::Fallback;
    use pw_metadata::write_plan::WriteSettings;

    use super::{Settings, load_settings, save_settings, settings_path};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    /// A new temporary folder (not created), removed with its contents when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("pw-loupe-settings-{}-{id}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }

        /// Creates the folder and writes `settings.txt` in it.
        fn with_file(text: &str) -> (Self, PathBuf) {
            let dir = Self::new();
            std::fs::create_dir_all(&dir.0).unwrap();
            let path = dir.0.join("settings.txt");
            std::fs::write(&path, text).unwrap();
            (dir, path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn file_names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Asserts exactly one problem, naming the line.
    fn assert_one_problem_on_line(problems: &[String], line: usize) {
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains(&format!("line {line}:")),
            "{:?} does not name line {line}",
            problems[0]
        );
    }

    #[test]
    fn defaults_are_no_iim_and_windows_1252() {
        let settings = Settings::default();
        assert!(!settings.create_iim);
        assert_eq!(settings.fallback, Fallback::Windows1252);
    }

    #[test]
    fn missing_file_gives_defaults_and_no_problem() {
        let dir = TempDir::new();
        let (settings, problems) = load_settings(&dir.0.join("settings.txt"));
        assert_eq!(settings, Settings::default());
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn save_then_load_gives_the_same_settings_for_all_four_combinations() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        // The same file each time: every save after the first replaces an existing file.
        for create_iim in [false, true] {
            for fallback in [Fallback::Windows1252, Fallback::Latin1] {
                let settings = Settings {
                    create_iim,
                    fallback,
                };
                save_settings(&path, &settings).unwrap();
                let (loaded, problems) = load_settings(&path);
                assert_eq!(loaded, settings);
                assert!(problems.is_empty(), "{problems:?}");
            }
        }
    }

    #[test]
    fn saved_file_has_the_comment_line_and_both_keys() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let settings = Settings {
            create_iim: true,
            fallback: Fallback::Latin1,
        };
        save_settings(&path, &settings).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# RawStride\ncreate_iim = true\nfallback_encoding = iso-8859-1\n"
        );
    }

    #[test]
    fn comments_empty_lines_and_spaces_are_allowed() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\r\n\r\n   \n  # another comment\n\tcreate_iim\t=   true  \n\n  \
             fallback_encoding=iso-8859-1\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings,
            Settings {
                create_iim: true,
                fallback: Fallback::Latin1,
            }
        );
    }

    #[test]
    fn unknown_key_gives_one_problem_and_the_other_keys_are_read() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\ncreate_iim = true\ncolour = blue\nfallback_encoding = iso-8859-1\n",
        );
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 3);
        assert!(problems[0].contains("colour"), "{problems:?}");
        assert_eq!(
            settings,
            Settings {
                create_iim: true,
                fallback: Fallback::Latin1,
            }
        );
    }

    #[test]
    fn bad_create_iim_value_keeps_its_default_and_the_other_key_is_read() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\ncreate_iim = yes\nfallback_encoding = iso-8859-1\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 2);
        assert!(problems[0].contains("yes"), "{problems:?}");
        assert!(!settings.create_iim);
        assert_eq!(settings.fallback, Fallback::Latin1);
    }

    #[test]
    fn bad_fallback_value_keeps_its_default_and_the_other_key_is_read() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\n\ncreate_iim = true\nfallback_encoding = utf-16\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 4);
        assert!(problems[0].contains("utf-16"), "{problems:?}");
        assert!(settings.create_iim);
        assert_eq!(settings.fallback, Fallback::Windows1252);
    }

    #[test]
    fn line_without_equals_gives_one_problem_and_the_other_key_is_read() {
        let (_dir, path) = TempDir::with_file("create_iim true\nfallback_encoding = iso-8859-1\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 1);
        assert!(!settings.create_iim);
        assert_eq!(settings.fallback, Fallback::Latin1);
    }

    #[test]
    fn byte_order_mark_is_not_part_of_the_first_key() {
        let (_dir, path) = TempDir::with_file("\u{feff}create_iim = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(settings.create_iim);
    }

    #[test]
    fn file_that_cannot_be_read_gives_defaults_and_one_problem() {
        // A folder where the file should be: it exists but cannot be read as text.
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        std::fs::create_dir_all(&path).unwrap();
        let (settings, problems) = load_settings(&path);
        assert_eq!(settings, Settings::default());
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn save_creates_a_missing_folder() {
        let dir = TempDir::new();
        let path = dir.0.join("RawStride").join("settings.txt");
        assert!(!dir.0.exists());
        save_settings(&path, &Settings::default()).unwrap();
        assert!(path.is_file());
        assert_eq!(load_settings(&path), (Settings::default(), Vec::new()));
    }

    #[test]
    fn save_leaves_no_temporary_file() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        save_settings(&path, &Settings::default()).unwrap();
        save_settings(
            &path,
            &Settings {
                create_iim: true,
                fallback: Fallback::Latin1,
            },
        )
        .unwrap();
        assert_eq!(file_names(&dir.0), vec!["settings.txt".to_string()]);
    }

    #[test]
    fn edit_options_carry_both_settings() {
        assert_eq!(Settings::default().edit_options(), EditOptions::default());
        let settings = Settings {
            create_iim: true,
            fallback: Fallback::Latin1,
        };
        assert_eq!(
            settings.edit_options(),
            EditOptions {
                settings: WriteSettings { create_iim: true },
                fallback: Fallback::Latin1,
            }
        );
    }

    #[test]
    fn settings_path_is_in_the_rawstride_folder_of_local_app_data() {
        // Read only: the environment is not changed (other tests run at the same time).
        match std::env::var_os("LOCALAPPDATA").filter(|data| !data.is_empty()) {
            Some(data) => assert_eq!(
                settings_path(),
                Some(PathBuf::from(data).join("RawStride").join("settings.txt"))
            ),
            None => assert_eq!(settings_path(), None),
        }
    }
}
