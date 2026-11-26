# T343. ADR 0022 S4a: the "Filter…" window's model, parsing and texts — Done

- **Why:** ADR 0022 S4 (the "Filter…" window), decided by the owner on 2026-11-26 (ADR 0022
  §3.3, S4-Q1–S4-Q14). This task is the plain-Rust part: the rows being edited in the window,
  the parsing of the typed numbers and dates into S1's rows, the value lists' counts, the
  status line's count over the groups a filter needs, and the window's texts. Nothing changes on
  screen. The first task of the chain **T343** → T344 (the window in the shell) → T345 (the
  value lists' reading of the whole folder); after T345 the owner's hand check of S4 (S4-Q14).
- **What the reviewer checked (2026-11-26):**
  - `crates/pw-loupe/src/meta_filter.rs`: `Field` and `Field::ALL` (name, Exif, IPTC order),
    `Field::group`, the private `Field::allows` (which conditions each field takes, about line
    108), `Value`, `Condition` (`Contains`, `Is`, `IsNot`, `OneOf`, `AtLeast`, `AtMost`,
    `Between`, `HasNo`), `Row::new` (validates; swaps `Between`'s bounds), `ImageValues`
    (`is_read`, `value`, the private `texts`), the private `normalized` (the case-blind form
    used in matching), `Filter` (`set_search`, `push`, `remove`, `clear`, `rows`,
    `condition_count`, `needs`, `outcome`). `Filter` has no way to replace its rows and keep
    its search.
  - `crates/pw-loupe/src/filter_values.rs`: `exif_values` (File Type is the text `RAW`, `DNG`,
    `JPEG` or `TIFF`; Exposure Time a number of seconds, `1/250` is `0.004`; Capture Date
    `Value::Date`, whole seconds from 1970-01-01 without a time zone), the private
    `capture_seconds` (validates the calendar and counts the days from March, about line 40),
    `TabValues` (`values`, `counts(group, files)` → read, total, unreadable, where only IPTC
    can be unreadable), `search_counts` and `SearchCounts` (used by the shell's status line
    today, IPTC only).
  - `crates/pw-loupe/src/info_panel.rs`: `property_label(xmp_id, Language)` (re-exported from
    `lib.rs`), the only source of IPTC property names (`docs/tasks/README.md` rule 13). The
    filter's IPTC fields are read from (`pw-metadata`'s `filter_values.rs`): Title `dc:title`,
    Headline `photoshop:Headline`, Description `dc:description`, Keywords `dc:subject`, Person
    Shown `Iptc4xmpExt:PersonInImage`, Creator `dc:creator`, Copyright Notice `dc:rights`,
    City `photoshop:City`, Country `photoshop:Country`, Location `Iptc4xmpCore:Location`
    (`property_label` gives "Sublocation" for it).
  - `crates/pw-loupe/src/loupe_text.rs`: `reading_metadata_text`, `unreadable_images_text`
    (reused by T344 and T345, not changed here).
  - `docs/glossary.md` §2 (ratings, labels, marking and filters): "Filter, Show All", the
    search's status texts (T341), "File Name, … File Type" of the sort (T138); §3: "Search"
    (T342); §4 (Viewer and zoom) "Camera, Lens, Focal Length, Aperture, Exposure Time, ISO,
    Taken" (Info). ADR 0022 §2.1 names the date field "Capture Date".
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/filter_window.rs` (new),
  `crates/pw-loupe/src/filter_text.rs` (new), `crates/pw-loupe/src/lib.rs`,
  `crates/pw-loupe/src/meta_filter.rs`, `crates/pw-loupe/src/filter_values.rs`,
  `docs/glossary.md`, `docs/test-inventory.md` (rule 12), and this task's own file (status and
  log).
- **Model:** Opus — the owner's choice for the whole chain (S4-Q14: state); one layer, seven
  files.
- **What:**
  1. **The condition kinds** (`meta_filter.rs`): a public `enum ConditionKind { Contains, Is,
     IsNot, OneOf, AtLeast, AtMost, Between, IsEmpty }` (`IsEmpty` is `Condition::HasNo`), with
     `Condition::kind(&self) -> ConditionKind`, and `pub fn kinds(self) -> &'static
     [ConditionKind]` on `Field`: the kinds `allows` takes for the field, in this order:
     Contains, Is, IsNot, OneOf, IsEmpty for the text fields (each only where `allows` takes
     it), AtLeast, AtMost, Between for the numbers and Capture Date, OneOf alone for File Type.
     `allows` stays the one rule: a test builds one sample `Condition` per kind and checks that
     `kinds` lists exactly what `allows` takes for every field. Also `pub fn
     has_value_list(self) -> bool` (Camera, Lens, Keywords, City, Country: S4-Q6) and, at
     module level, `pub const FILE_TYPES: [&str; 4] = ["RAW", "JPEG", "TIFF", "DNG"]` (S4-Q6's
     fixed list; the texts `exif_values` gives). Note: at the crate root `crate::Filter` is the
     star and label filter and `crate::Field` is the import's; the new modules import
     `crate::meta_filter::{Field, Filter, …}`.
  2. **Replacing the rows** (`meta_filter.rs`): `Filter::set_rows(&mut self, rows: Vec<Row>)`,
     which keeps the search. `normalized` becomes `pub(crate)` so the value lists merge values
     as the matching does.
  3. **Parsing** (`filter_window.rs`, S4-Q4), plain functions:
     - `parse_number(field: Field, text: &str) -> Option<f64>`: the text trimmed. A plain
       number is ASCII digits with at most one decimal point or comma (the comma counts as a
       point): no sign, no exponent, no `inf` or `NaN`. Aperture may begin with `f/` or `F/`,
       spaces after it allowed; Exposure Time is a fraction `a/b` of two plain numbers (spaces
       around `/` allowed) or a plain number of seconds; ISO is ASCII digits only (`1600.0` is
       refused); Focal Length a plain number. Every value must be above 0. Anything else is
       `None`. Only for the four number fields; other fields give `None`.
     - `parse_date(text: &str) -> Option<i64>`: `YYYY-MM-DD` exactly (trimmed; four, two and
       two ASCII digits, no sign), a real calendar day, giving the seconds at 00:00:00 of that
       day in `Value::Date`'s count. The day count of `capture_seconds` (the last-day check and
       the count from March) is moved into a `pub(crate) fn day_number(year: i64, month: i64,
       day: i64) -> Option<i64>` (days from 1970-01-01; `None` for a day that does not exist)
       in `filter_values.rs`, used by both (`capture_seconds`' behaviour and tests unchanged);
       and its inverse `pub(crate) fn civil_date(day: i64) -> (i64, i64, i64)` for point 4.
  4. **A row being edited** (`filter_window.rs`, S4-Q3): `pub struct DraftRow` with the
     field, the kind, the typed text, the second text (the upper bound of "between") and the
     picked values (`Vec<String>`, for "is one of"), all private, with this API (T344 draws
     with it): `new(field)` (the field's first kind), `field()`, `kind()`, `text() -> &str`,
     `text2() -> &str`, `picks() -> &[String]`, `set_field(Field)` (keeps the kind if the new field takes it,
     otherwise the first kind; empties the texts and picks when the field changes),
     `set_kind(ConditionKind)` (ignored when the field does not take the kind; the texts and
     picks are kept), `text_mut() -> &mut String` and `text2_mut() -> &mut String` (for egui's
     text fields), `toggle_pick(&str)` (adds the value, or removes a pick equal to it after
     `normalized`). The kind is so always one the field takes. `DraftRow::to_row(&self) ->
     Result<Row, Missing>`, where `pub enum Missing { Value, Pick, Number, Date }` (an error of
     `Row::new` maps to `EmptyValue` → `Value`, `NotANumber` → `Number`, and `NotAllowed`,
     which the API above cannot reach, → `Value`; no panic):
     - Contains, Is, IsNot: the text trimmed must not be empty (`Value`); the row keeps the
       text as typed.
     - OneOf: at least one pick (`Pick`).
     - IsEmpty: always a row.
     - AtLeast, AtMost, Between on a number field: `parse_number` (`Number` if either text
       fails).
     - On Capture Date: `parse_date` (`Date`): AtLeast from 00:00:00 of the day, AtMost to
       23:59:59 of the day, Between from the earlier day's 00:00:00 to the later day's
       23:59:59 (the days are put in order first, S4-Q4).
     - `DraftRow::from_row(&Row) -> DraftRow`, so an applied filter opens again in the window
       as it was typed in meaning: texts as they are; numbers written back (Exposure Time
       under 1 s whose inverse is a whole number within 1e-6 as `1/n`, otherwise the number
       with Rust's `{}`); dates as `YYYY-MM-DD` of the day (`civil_date` of the seconds
       divided by 86400, rounding down). `from_row(r).to_row()` gives `r` again for every kind
       (a test).
  5. **The window's rows** (`filter_window.rs`): `pub struct FilterDraft` with
     `from_filter(&Filter)` (one `DraftRow` per row, in order), `rows() -> &[DraftRow]`,
     `rows_mut() -> &mut [DraftRow]`,
     `add()` (a new row of File Name, its first kind), `remove(index)`, `clear()`,
     `result(&self) -> Result<Vec<Row>, (usize, Missing)>` (the first row that is not complete,
     S4-Q3: OK is enabled only on `Ok`), and `list_groups(&self) -> Vec<Group>`: the groups of
     the rows whose kind is OneOf on a field with `has_value_list` (for T345's reading,
     S4-Q7; each group once).
  6. **The value lists' counts** (`filter_values.rs`, S4-Q6):
     `TabValues::value_counts(&self, field: Field, files: &[PathBuf]) -> Vec<(String, usize)>`:
     over the images of `files` whose field's group is read, each value of the field from
     `ImageValues::value` (a text, or each text of a list) counted once per image; values equal
     after `normalized` are one value, shown with the trimmed spelling met first in `files`'
     order; empty and blank texts left out; sorted by the `normalized` form. A field without a
     text value (File Name, the numbers, Capture Date) gives an empty list. In
     `filter_window.rs`, `pub fn list_entries(counts: &[(String, usize)], picked: &[String]) ->
     Vec<(String, usize, bool)>`: every counted value, in the counts' order and with the
     counted spelling, with whether a pick equals it after `normalized`; then each picked value
     not among them, in the picks' order, with the count 0 (S4-Q7).
  7. **The status line's count** (`filter_values.rs`, S4-Q8): `TabValues::filter_counts(&self,
     filter: &Filter, files: &[PathBuf]) -> SearchCounts`: an image is done when, for every
     group the filter needs (`needs`), that group is read or (IPTC only) could not be read; so
     an image with IPTC unreadable and Exif not yet read is not done. `unreadable` counts the
     images whose IPTC could not be read, only while the filter needs IPTC; `total` is
     `files.len()`. A
     filter that needs no group (File Name rows only) gives every image done. For a filter
     with only a search it gives the same as `search_counts` (a test); `search_counts` stays
     (T344 changes the shell's call).
  8. **The texts** (`filter_text.rs`, rule 13; every function makes a whole text):
     - `field_label(field: Field) -> String`: "File Name", "Camera", "Lens", "Focal Length",
       "Aperture", "Exposure Time", "ISO", "Capture Date", "File Type"; the IPTC fields through
       `property_label(<id>, Language::En)` with the ids above, never a literal of their own.
     - `group_label(group: Group) -> String`: "File", "Exif", "IPTC" (the field list's
       headings, S4-Q3).
     - `condition_text(kind: ConditionKind) -> String`: "contains", "is", "is not", "is one
       of", "≥", "≤", "between", "is empty" (S4-Q13).
     - `between_template() -> String`: "{low} and {high}", one template so another language
       can change the order, and `pub enum BetweenPart { Text(String), Low, High }` with
       `between_parts() -> Vec<BetweenPart>`, the template split at its two placeholders
       (here `[Low, Text(" and "), High]`); the shell draws the texts and the two value fields
       in this order.
     - `value_hint(field: Field) -> Option<String>`: the hint text in an empty value field:
       Exposure Time "1/250", Aperture "2.8", Focal Length "35", ISO "1600", Capture Date
       "YYYY-MM-DD"; the text fields `None`.
     - `missing_text(missing: Missing) -> String`: "Enter a value", "Pick at least one value",
       "Enter a number", "Enter a date as YYYY-MM-DD".
     - `filter_button_text(conditions: usize) -> String` (S4-Q10): 0 "Filter…", 1 "Filter: 1
       condition", otherwise "Filter: {n} conditions", made in one function.
     - `value_count_text(value: &str, count: usize) -> String`: "{value} ({count})".
     - `FilterText` with `filter_text(FilterText) -> String`: `Title` "Filter", `AddCondition`
       "Add Condition", `Remove` "Remove", `Clear` "Clear", `Ok` "OK", `Cancel` "Cancel",
       `NoConditions` "No conditions: every image is shown.", `NoValues` "No values in the
       folder".
  9. **`lib.rs`:** `pub mod filter_window;` and `pub mod filter_text;` (the shell in `main.rs`
     uses them through `pw_loupe::…`).
  10. **Glossary** (`docs/glossary.md` §2): one row for the window's texts (the title, the
      buttons, the condition words, "{low} and {high}", the hints, the missing texts,
      "Filter: {n} conditions"), one for the field and group names not yet in it ("Capture
      Date" as the filter's field, "Exif", "IPTC" as the field list's headings; "File Name"
      and "File Type" are already in T138's row in §2 and "File" is a menu title, so the row
      refers to them instead of repeating them), each "added by T343". "Filter…" as a menu item and
      key action is T344's.
- **Tests to add** (in the modules' own test blocks):
  - `meta_filter.rs`: `kinds` matches `allows` for every field and kind, in the order of point
    1; `has_value_list` for exactly the five fields; `set_rows` keeps the search and replaces
    the rows; `Condition::kind` for each condition.
  - `filter_window.rs`: `parse_number` (`1/250`, `0.5`, `2`, `2,8`, `f/2.8`, `F/4`, `35`,
    `1600`; refused: empty, `abc`, `0`, `-1`, `1/0`, `2.5` for ISO, `f/2.8` for Focal Length,
    any text for a text field); `parse_date` (`2024-02-29` yes, `2023-02-29`, `2024-13-01`,
    `24-01-01`, `2024-1-1` no); `to_row` for each kind and each `Missing`; the date bounds
    (AtMost to 23:59:59, Between with the days in either order); `from_row` round-trips for
    every kind, Exposure Time `1/250` and `0.3`; changing the field keeps or resets the kind
    and empties the texts; `FilterDraft::result` gives the first incomplete row's index;
    `list_groups` (Camera OneOf → Exif; Keywords OneOf and City OneOf → IPTC once; Camera
    Contains → none); `list_entries` keeps a picked value missing from the counts with 0 and
    matches picks case-blind.
  - `filter_values.rs`: `value_counts` (one image with a keyword twice counts once; "Canon"
    and "canon " are one value with the first spelling; unread images and empty texts left
    out; the order); `filter_counts` (Exif and IPTC both needed: done only when both are read;
    IPTC unreadable counts as done and unreadable; File Name only: all done; a search only:
    equal to `search_counts`); `capture_seconds`' tests unchanged and passing; `day_number`
    and `civil_date` round-trip over a leap day and 1969-12-31.
  - `filter_text.rs`: every text above; the IPTC labels equal `property_label`'s answer for
    their ids; `filter_button_text` for 0, 1 and 2.
- **Not in this task:** the window, the command bar's button, the menu item and the key (T344);
  the reading for the value lists (T345); saved filters (S5); any change to `outcome`,
  `matches`, `Row::new` or the shell.
- **Rules:** `docs/tasks/README.md` rules 1–13. No `git commit`, `git add`, `git stash`,
  `git reset`, `git checkout` and no new branch (rule 8). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK, `git diff --stat` shows only the files above and
  `git status --short` shows no new file other than `filter_window.rs` and `filter_text.rs`.
  No hand check of its own (nothing on screen changes); S4's hand check follows T345.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Open | Added (ADR 0022 §3.3, S4-Q3, S4-Q4, S4-Q6–S4-Q8, S4-Q13, S4-Q14) |
| 2026-11-26 | Open | Cold read: `DraftRow`'s API written out; the parsing's accepted forms exact (no sign, exponent or `inf`; ISO digits only); `day_number`/`civil_date` types; `filter_counts` per group; `value_counts` through `ImageValues::value`, blanks skipped; `list_entries`' order and spelling; "between" as one template with `between_parts` (rule 13); `Missing` from `RowError`; the crate root's `Filter` and `Field` names; the glossary's sections; new files in "Done when" |
| 2026-11-26 | Open | Chain review (a fresh agent read T343–T345 together): names and signatures match across the chain (return types added here); Exif answers never refilter, so an Exif-only row kept every image hidden (T344 point 4a); the window closes as Cancel when its tab stops being active and no folder drop while it is open (T344); the tab check shared with T345; F5 corrected in T344; the hand check extended (T345); leftovers listed in T345's log for its Done. Enter in the window stays undecided (does nothing) |
| 2026-11-26 | Ready for review | Implemented the draft model, parsers, metadata counts and English texts with 22 new unit tests. The new tests passed in the sandbox. The full check still needs the reviewer's run because the audit database could not be locked and ExifTool could not start in the sandbox. The reviewer updates the test inventory. |
| 2026-11-26 | Done | Reviewed (done by Codex): every point and test as written; `.\tools\check.ps1` run by the reviewer, all OK, no clippy warnings. The agent had added a hidden `original_bounds` to `DraftRow` (to give back an applied row whose bounds the window's text could not show exactly); every row the window makes already round-trips without it, so the reviewer removed it with its test case (owner's choice). `day_number` counts in `i128` and cannot overflow (accepted). No leftovers. `docs/test-inventory.md` updated by the reviewer (+22). No hand check of its own; S4's follows T345 |
