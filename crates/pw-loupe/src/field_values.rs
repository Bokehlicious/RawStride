//! The recent values and the user's own values per field of the metadata window (ADR 0017
//! §2.3.1 "Speed while captioning", §6.1 U12a, T141): what a write that succeeded put in a field
//! (the last [`RECENT_LIMIT`], newest first) and a list the user keeps ("Add to My Values"),
//! offered by the "▼" beside each one-line text field and list field ([`EditorField::has_values`]).
//!
//! They are kept in `field-values.txt` beside `settings.txt` (`%LOCALAPPDATA%\RawStride\`), for
//! this user on this computer only; nothing is sent anywhere. The file is UTF-8 text with one value
//! per line, `recent.<field> = <text>` and `own.<field> = <text>`, `<field>` the field's XMP id (the
//! sub-field's for the contact fields), each list in its order. The text after `= ` is kept as it
//! is (an `=` in it, spaces at either end). Loading never fails: an unreadable line is skipped with
//! a problem text in English for the log, as for `settings.txt`. Saving writes a temporary file
//! and renames it onto the file, so a crash never leaves half a file.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::editor::EditorField;
use crate::settings::settings_path;

/// How many recent values a field keeps (U12a-Q2).
pub const RECENT_LIMIT: usize = 10;

/// The file's name, in the folder of `settings.txt` (U12a-Q6).
pub const FIELD_VALUES_FILE: &str = "field-values.txt";

/// The key prefix of a recent value.
const RECENT: &str = "recent";
/// The key prefix of an own value.
const OWN: &str = "own";

/// `%LOCALAPPDATA%\RawStride\field-values.txt`; `None` without `LOCALAPPDATA`.
pub fn field_values_path() -> Option<PathBuf> {
    settings_path().map(|path| path.with_file_name(FIELD_VALUES_FILE))
}

/// The id a field's values are kept under: its XMP id, the sub-field's for a contact field.
pub fn field_id(field: EditorField) -> &'static str {
    field.struct_field().unwrap_or(field.property())
}

/// The field with a "▼" whose id is `id`.
fn field_of_id(id: &str) -> Option<EditorField> {
    EditorField::ALL
        .into_iter()
        .find(|field| field.has_values() && field_id(*field) == id)
}

/// A value that can be kept: not only spaces, and one line (the file has one value per line).
fn keepable(value: &str) -> bool {
    !value.trim().is_empty() && !value.contains(['\n', '\r'])
}

/// The recent and own values of every field with a "▼" (U12a-Q2, U12a-Q3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldValues {
    recent: HashMap<EditorField, Vec<String>>,
    own: HashMap<EditorField, Vec<String>>,
}

impl FieldValues {
    /// The recent values of `field`, newest first.
    pub fn recent(&self, field: EditorField) -> &[String] {
        self.recent.get(&field).map_or(&[], Vec::as_slice)
    }

    /// The own values of `field`, alphabetically (case ignored).
    pub fn own(&self, field: EditorField) -> &[String] {
        self.own.get(&field).map_or(&[], Vec::as_slice)
    }

    /// The values of a write that succeeded (`EditorForm::written_values`): each field's values
    /// become its newest recent values. True when anything changed.
    pub fn add_written(&mut self, written: &[(EditorField, Vec<String>)]) -> bool {
        let mut changed = false;
        for (field, values) in written {
            changed |= self.add_recent(*field, values);
        }
        changed
    }

    /// Adds `values` (a list field's entries in their order, or a text field's one text) as the
    /// newest recent values of `field`: they come first in their order, a value already there
    /// moves up instead of being kept twice, and only the newest [`RECENT_LIMIT`] are kept. A value
    /// that is only spaces adds nothing; nothing for a field without a "▼". True when the list
    /// changed.
    pub fn add_recent(&mut self, field: EditorField, values: &[String]) -> bool {
        if !field.has_values() {
            return false;
        }
        let before = self.recent(field).to_vec();
        let mut list = before.clone();
        for value in values.iter().rev().filter(|value| keepable(value)) {
            list.retain(|kept| kept != value);
            list.insert(0, value.clone());
        }
        list.truncate(RECENT_LIMIT);
        let changed = list != before;
        if changed {
            self.recent.insert(field, list);
        }
        changed
    }

    /// "Add to My Values" (U12a-Q3): adds `values` (the field's present text, each entry on its own
    /// for a list field) to the own values of `field`, each once, kept alphabetically (case
    /// ignored). True when the list changed.
    pub fn add_own(&mut self, field: EditorField, values: &[String]) -> bool {
        if !field.has_values() {
            return false;
        }
        let mut list = self.own(field).to_vec();
        let mut changed = false;
        for value in values.iter().filter(|value| keepable(value)) {
            if !list.contains(value) {
                list.push(value.clone());
                changed = true;
            }
        }
        if changed {
            sort_own(&mut list);
            self.own.insert(field, list);
        }
        changed
    }

    /// "×" beside an own value: removes `value` from the own values of `field`. True when it was
    /// there.
    pub fn remove_own(&mut self, field: EditorField, value: &str) -> bool {
        match self.own.get_mut(&field) {
            Some(list) => {
                let before = list.len();
                list.retain(|kept| kept != value);
                let removed = list.len() != before;
                if list.is_empty() {
                    self.own.remove(&field);
                }
                removed
            }
            None => false,
        }
    }

    /// The file's text: a comment line, then per field (form order) its recent values, then its
    /// own values, one per line in list order.
    fn file_text(&self) -> String {
        let mut text = String::from("# RawStride field values\n");
        for field in EditorField::ALL {
            for (prefix, values) in [(RECENT, self.recent(field)), (OWN, self.own(field))] {
                for value in values.iter().filter(|value| keepable(value)) {
                    text.push_str(&format!("{prefix}.{id} = {value}\n", id = field_id(field)));
                }
            }
        }
        text
    }
}

/// Sorts own values alphabetically ignoring case; values that differ only in case keep a fixed
/// order.
fn sort_own(list: &mut [String]) {
    list.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
}

/// Reads `field-values.txt`. A missing file gives no values and no problem; a file that cannot be
/// read gives no values and one problem. A line without `=`, with an unknown key or field, or
/// with an empty value is skipped with a problem naming its line number. The problems are
/// English, for the log (`eprintln!` by the caller).
pub fn load_field_values(path: &Path) -> (FieldValues, Vec<String>) {
    let mut values = FieldValues::default();
    let mut problems = Vec::new();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return (values, problems),
        Err(error) => {
            problems.push(format!(
                "field values file {} could not be read, no values used: {error}",
                path.display()
            ));
            return (values, problems);
        }
    };
    // A byte order mark written by an editor is not part of the first key.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let problem = |what: &str| {
            format!(
                "field values file {}, line {number}: {what} in \"{trimmed}\", line ignored",
                path.display()
            )
        };
        let Some((key, value)) = line.split_once('=') else {
            problems.push(problem("no '='"));
            continue;
        };
        // The text after `= ` is kept as it is.
        let value = value.strip_prefix(' ').unwrap_or(value);
        let parsed = key
            .trim()
            .split_once('.')
            .and_then(|(prefix, id)| Some((prefix, field_of_id(id)?)));
        let Some((prefix, field)) = parsed else {
            problems.push(problem("unknown key"));
            continue;
        };
        if !keepable(value) {
            problems.push(problem("empty value"));
            continue;
        }
        match prefix {
            RECENT => {
                let list = values.recent.entry(field).or_default();
                if !list.iter().any(|kept| kept == value) && list.len() < RECENT_LIMIT {
                    list.push(value.to_string());
                }
            }
            OWN => {
                values.add_own(field, &[value.to_string()]);
            }
            _ => problems.push(problem("unknown key")),
        }
    }
    (values, problems)
}

/// Writes `field-values.txt`: creates the folder, writes the values to `<file name>.tmp` in the
/// same folder, then renames it onto the file (which replaces it on Windows).
pub fn save_field_values(path: &Path, values: &FieldValues) -> io::Result<()> {
    let Some(file_name) = path.file_name() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("field values path {} has no file name", path.display()),
        ));
    };
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut temp_name = file_name.to_os_string();
    temp_name.push(".tmp");
    let temp = path.with_file_name(temp_name);
    let written = write_synced(&temp, values.file_text().as_bytes())
        .and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        // Best effort: the error of the write or the rename is the one reported.
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// Writes the bytes to a new file and waits until they are on the disk.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// One entry of an open "▼" list that can be chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueChoice {
    pub value: String,
    /// True for an own value (it has a "×").
    pub own: bool,
}

/// The entries of the "▼" list of `field`: its recent values, then its own values (U12a-Q5).
pub fn value_choices(values: &FieldValues, field: EditorField) -> Vec<ValueChoice> {
    let recent = values.recent(field).iter().map(|value| ValueChoice {
        value: value.clone(),
        own: false,
    });
    let own = values.own(field).iter().map(|value| ValueChoice {
        value: value.clone(),
        own: true,
    });
    recent.chain(own).collect()
}

/// The open "▼" list of the metadata window (U12a-Q5): its field and the entry ↑/↓ have moved to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueList {
    pub field: EditorField,
    /// The index into [`value_choices`] of the highlighted entry; none until ↑ or ↓.
    pub highlighted: Option<usize>,
}

impl ValueList {
    /// The list of `field`, opened with nothing highlighted.
    pub fn new(field: EditorField) -> Self {
        Self {
            field,
            highlighted: None,
        }
    }

    /// ↓ (`down`) or ↑ among `count` entries: from nothing ↓ goes to the first and ↑ to the last;
    /// it stops at either end.
    pub fn step(&mut self, down: bool, count: usize) {
        let Some(last) = count.checked_sub(1) else {
            self.highlighted = None;
            return;
        };
        self.highlighted = Some(match (self.highlighted, down) {
            (None, true) => 0,
            (None, false) => last,
            (Some(index), true) => (index + 1).min(last),
            (Some(index), false) => index.saturating_sub(1).min(last),
        });
    }

    /// The value Enter chooses: the highlighted entry of `choices`, if any.
    pub fn chosen<'a>(&self, choices: &'a [ValueChoice]) -> Option<&'a str> {
        self.highlighted
            .and_then(|index| choices.get(index))
            .map(|choice| choice.value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "pw-loupe-field-values-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn with_file(text: &str) -> (Self, PathBuf) {
            let dir = Self::new();
            let path = dir.0.join(FIELD_VALUES_FILE);
            std::fs::write(&path, text).unwrap();
            (dir, path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn a_write_adds_recent_values_newest_first() {
        let mut values = FieldValues::default();
        assert!(values.add_written(&[(EditorField::City, strings(&["Oslo"]))]));
        assert!(values.add_written(&[(EditorField::City, strings(&["Bergen"]))]));
        assert_eq!(
            values.recent(EditorField::City),
            strings(&["Bergen", "Oslo"])
        );
        assert!(values.recent(EditorField::Headline).is_empty());
    }

    #[test]
    fn recent_values_are_at_most_ten() {
        let mut values = FieldValues::default();
        for n in 0..15 {
            values.add_recent(EditorField::Event, &[format!("Event {n}")]);
        }
        let recent = values.recent(EditorField::Event);
        assert_eq!(recent.len(), RECENT_LIMIT);
        assert_eq!(recent.first().map(String::as_str), Some("Event 14"));
        assert_eq!(recent.last().map(String::as_str), Some("Event 5"));
    }

    #[test]
    fn a_recent_value_written_again_moves_first_and_is_kept_once() {
        let mut values = FieldValues::default();
        for city in ["Oslo", "Bergen", "Tromsø"] {
            values.add_recent(EditorField::City, &strings(&[city]));
        }
        assert!(values.add_recent(EditorField::City, &strings(&["Oslo"])));
        assert_eq!(
            values.recent(EditorField::City),
            strings(&["Oslo", "Tromsø", "Bergen"])
        );
        // The newest written again changes nothing.
        assert!(!values.add_recent(EditorField::City, &strings(&["Oslo"])));
    }

    #[test]
    fn a_list_fields_entries_are_recent_values_one_by_one() {
        let mut values = FieldValues::default();
        values.add_written(&[(EditorField::Keywords, strings(&["sky", "sea"]))]);
        values.add_written(&[(EditorField::Keywords, strings(&["boat", "sky"]))]);
        assert_eq!(
            values.recent(EditorField::Keywords),
            strings(&["boat", "sky", "sea"])
        );
    }

    #[test]
    fn an_emptied_field_adds_nothing() {
        let mut values = FieldValues::default();
        assert!(!values.add_written(&[(EditorField::City, strings(&["   "]))]));
        assert!(!values.add_written(&[(EditorField::Keywords, Vec::new())]));
        assert!(values.recent(EditorField::City).is_empty());
        assert_eq!(values, FieldValues::default());
    }

    #[test]
    fn a_field_without_a_list_keeps_no_values() {
        let mut values = FieldValues::default();
        assert!(!values.add_recent(EditorField::Description, &strings(&["text"])));
        assert!(!values.add_own(EditorField::DateCreated, &strings(&["2026"])));
        assert!(values.recent(EditorField::Description).is_empty());
        assert!(values.own(EditorField::DateCreated).is_empty());
    }

    #[test]
    fn own_values_are_added_sorted_ignoring_case_and_kept_once() {
        let mut values = FieldValues::default();
        assert!(values.add_own(EditorField::Creator, &strings(&["zoe", "Anna"])));
        assert!(values.add_own(EditorField::Creator, &strings(&["bo", "Anna"])));
        assert!(!values.add_own(EditorField::Creator, &strings(&["zoe"])));
        assert_eq!(
            values.own(EditorField::Creator),
            strings(&["Anna", "bo", "zoe"])
        );
    }

    #[test]
    fn own_values_are_removed() {
        let mut values = FieldValues::default();
        values.add_own(EditorField::Credit, &strings(&["A", "B"]));
        assert!(values.remove_own(EditorField::Credit, "A"));
        assert!(!values.remove_own(EditorField::Credit, "A"));
        assert!(!values.remove_own(EditorField::City, "A"));
        assert_eq!(values.own(EditorField::Credit), strings(&["B"]));
    }

    #[test]
    fn the_values_are_saved_and_loaded_in_order() {
        let dir = TempDir::new();
        let path = dir.0.join("RawStride").join(FIELD_VALUES_FILE);
        let mut values = FieldValues::default();
        for city in ["Oslo", "Bergen", "Tromsø"] {
            values.add_recent(EditorField::City, &strings(&[city]));
        }
        values.add_recent(EditorField::Keywords, &strings(&["sky", "sea"]));
        values.add_own(
            EditorField::Email,
            &strings(&["b@example.com", "a@example.com"]),
        );
        values.add_own(EditorField::City, &strings(&["Lund"]));
        save_field_values(&path, &values).unwrap();
        let (loaded, problems) = load_field_values(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(loaded, values);
        assert_eq!(
            loaded.recent(EditorField::City),
            strings(&["Tromsø", "Bergen", "Oslo"])
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains(
                "recent.photoshop:City = Tromsø\nrecent.photoshop:City = Bergen\n\
                 recent.photoshop:City = Oslo\nown.photoshop:City = Lund\n"
            ),
            "{text}"
        );
        // A contact field is kept under its sub-field's id.
        assert!(
            text.contains("own.Iptc4xmpCore:CiEmailWork = a@example.com\n"),
            "{text}"
        );
        // No temporary file is left.
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from(FIELD_VALUES_FILE)]);
    }

    #[test]
    fn a_value_with_an_equals_sign_or_spaces_is_kept_as_it_is() {
        let dir = TempDir::new();
        let path = dir.0.join(FIELD_VALUES_FILE);
        let mut values = FieldValues::default();
        values.add_recent(EditorField::Headline, &strings(&["a = b", "  indented  "]));
        values.add_own(EditorField::Source, &strings(&["x=y"]));
        save_field_values(&path, &values).unwrap();
        let (loaded, problems) = load_field_values(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            loaded.recent(EditorField::Headline),
            strings(&["a = b", "  indented  "])
        );
        assert_eq!(loaded.own(EditorField::Source), strings(&["x=y"]));
    }

    #[test]
    fn a_bad_line_is_warned_about_and_skipped() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride field values\n\
             recent.photoshop:City = Oslo\n\
             no equals sign\n\
             recent.unknown:Field = x\n\
             later.photoshop:City = y\n\
             recent.dc:description = a multi-line field\n\
             own.photoshop:City =\n\
             own.photoshop:City = Lund\n",
        );
        let (values, problems) = load_field_values(&path);
        assert_eq!(values.recent(EditorField::City), strings(&["Oslo"]));
        assert_eq!(values.own(EditorField::City), strings(&["Lund"]));
        assert_eq!(problems.len(), 5, "{problems:?}");
        for (problem, line) in problems.iter().zip([3, 4, 5, 6, 7]) {
            assert!(problem.contains(&format!("line {line}:")), "{problem}");
        }
    }

    #[test]
    fn a_missing_file_gives_no_values_and_no_problem() {
        let dir = TempDir::new();
        let (values, problems) = load_field_values(&dir.0.join(FIELD_VALUES_FILE));
        assert_eq!(values, FieldValues::default());
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn a_file_that_cannot_be_read_gives_one_problem() {
        let dir = TempDir::new();
        // A folder in the file's place cannot be read as text.
        let path = dir.0.join(FIELD_VALUES_FILE);
        std::fs::create_dir_all(&path).unwrap();
        let (values, problems) = load_field_values(&path);
        assert_eq!(values, FieldValues::default());
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn the_file_is_beside_the_settings_file() {
        match (field_values_path(), settings_path()) {
            (Some(values), Some(settings)) => {
                assert_eq!(values.parent(), settings.parent());
                assert_eq!(values.file_name(), Some(FIELD_VALUES_FILE.as_ref()));
            }
            (None, None) => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_list_offers_recent_then_own_values() {
        let mut values = FieldValues::default();
        values.add_recent(EditorField::City, &strings(&["Oslo"]));
        values.add_own(EditorField::City, &strings(&["Lund"]));
        assert_eq!(
            value_choices(&values, EditorField::City),
            [
                ValueChoice {
                    value: "Oslo".into(),
                    own: false
                },
                ValueChoice {
                    value: "Lund".into(),
                    own: true
                },
            ]
        );
        assert!(value_choices(&values, EditorField::Event).is_empty());
    }

    #[test]
    fn up_and_down_move_in_the_list_and_enter_chooses() {
        let mut values = FieldValues::default();
        values.add_recent(EditorField::City, &strings(&["B", "A"]));
        values.add_own(EditorField::City, &strings(&["C"]));
        let choices = value_choices(&values, EditorField::City);
        let mut list = ValueList::new(EditorField::City);
        assert_eq!(list.chosen(&choices), None);
        list.step(true, choices.len());
        assert_eq!(list.chosen(&choices), Some("B"));
        list.step(true, choices.len());
        list.step(true, choices.len());
        list.step(true, choices.len());
        assert_eq!(list.chosen(&choices), Some("C"));
        list.step(false, choices.len());
        assert_eq!(list.chosen(&choices), Some("A"));
        let mut from_end = ValueList::new(EditorField::City);
        from_end.step(false, choices.len());
        assert_eq!(from_end.chosen(&choices), Some("C"));
        let mut empty = ValueList::new(EditorField::Event);
        empty.step(true, 0);
        assert_eq!(empty.highlighted, None);
    }

    #[test]
    fn the_fields_with_a_list_have_unique_ids() {
        let fields: Vec<_> = EditorField::ALL
            .into_iter()
            .filter(|field| field.has_values())
            .collect();
        for field in &fields {
            assert_eq!(field_of_id(field_id(*field)), Some(*field));
        }
    }
}
