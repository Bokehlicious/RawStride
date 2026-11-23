# T287. The variables of a metadata text: reading and filling them — Open

- **Why:** ADR 0017 U13, part U13b (U13-Q9). A text field or a list entry in the metadata window
  or in a template can hold **variables** such as `{date}` or `{name}`; they are turned into each
  image's own values when the image is written, so one template gives each image its own date.
  This task makes the variables in plain Rust in `pw-metadata`: checking a text, and filling it
  with one image's values. T288 fills them when an image is written; T289 checks them in the
  metadata window before Save.
- **Crates:** `pw-metadata`. **Files:** `crates/pw-metadata/src/variables.rs` (new),
  `crates/pw-metadata/src/lib.rs` (only `pub mod variables;`), `docs/test-inventory.md`.
- **Background (read before starting):** `value.rs` (`Value`: `Text`, `LangAlt`, `List`, `Date`,
  `Number`, `Struct`, `StructList`; `StructValue`; `LangAlt::entries`, `LangAlt::from_entries`).
  `date.rs`: `IptcDate` (`year`, `month`, `day`, `time` with `hour`, `minute`, `second`) and
  `IptcDate::from_exif`, which reads Exif DateTimeOriginal (`YYYY:MM:DD hh:mm:ss`). The import's
  names have variables of their own in `crates/pw-ingest/src/template.rs` (read it for the
  spelling and the `{{`/`}}` rule; `pw-metadata` does not depend on `pw-ingest` and must not
  start to, `AGENTS.md` §3).
- **The variables (U13-Q9, owner 2026-11-22), exactly these nine:**

  | Variable | Example | Value |
  |---|---|---|
  | `{date}` | `2026-11-22` | the capture date, `YYYY-MM-DD` |
  | `{year}` | `2026` | four digits |
  | `{month}` `{day}` | `11`, `22` | two digits each |
  | `{time}` | `143005` | the capture time, `hhmmss` |
  | `{make}` `{model}` | `SONY`, `ILCE-1` | the camera; empty when unknown |
  | `{name}` | `DSC01234` | the image's file name without its extension |
  | `{folder}` | `2026-11-22 Trip` | the name of the folder the image is in |

  `{{` gives `{` and `}}` gives `}`. A name is matched after `str::trim` and
  `to_ascii_lowercase` (`{ Date }` is `{date}`), as the import's names are. Any other name, also
  the import's own `{year2}`, `{hour}`, `{seq}` or `{job}`, is unknown. The text is read exactly
  as the import's tokenizer reads it (`Template::parse` in `pw-ingest`), so `check` and `fill`
  always agree: outside a variable `{{` and `}}` are braces; a `{` starts a name that runs to
  the next `}` (a `{` inside it is part of the name, so `{a{b}` is the unknown name `a{b`); a
  lone `}` is an error; `{date}}` is `{date}` followed by a lone `}`.
- **What:**
  1. **The values of one image:** `pub struct VariableValues { pub capture_time:
     Option<IptcDate>, pub make: Option<String>, pub model: Option<String>, pub name: String,
     pub folder: String }` (`Debug, Clone, Default, PartialEq, Eq`) and `pub fn
     VariableValues::of(image: &Path, date_time_original: Option<&str>, make: Option<&str>,
     model: Option<&str>) -> VariableValues`:
     - `capture_time` from `IptcDate::from_exif(text, None, None)` on the text with spaces and
       NULs trimmed at both ends (`from_exif` takes exactly 19 characters); an error is `None`;
     - `make` and `model` trimmed; a blank one is `None`;
     - `name` is `image`'s file stem (`a.b.jpg` gives `a.b`), `folder` the file name of its
       parent folder; each is empty when the path has none (`D:\x.jpg` has no folder name), and a
       name that is not Unicode is taken lossily.
  2. **Errors:** `pub enum VariableError` (`Debug, Clone, PartialEq, Eq`, `Display`,
     `std::error::Error`):
     - `Unclosed(String)`: a `{` without its `}`; holds the text from that `{`, the brace
       included, to the end (`{date`);
     - `UnexpectedClose`: a single `}` that does not close a variable;
     - `Unknown(String)`: a name that is not one of the nine; holds the name exactly as written
       between the braces, without them (`job`, ` Job `);
     - `NoCaptureTime(String)`: a date variable (`{date}`, `{year}`, `{month}`, `{day}`,
       `{time}`) on an image without a capture time, or with a `capture_time` that lacks the
       part the variable needs (month, day, or a time with seconds; the fields are public, so a
       caller can build one); holds the name as `Unknown` does.

     `Display` gives these English texts, the name exactly as held: `'{date' is not closed`,
     `a '}' without its '{'`, `unknown variable { Job }`, `the image has no capture time for
     {date}`. The metadata worker passes them on as the reason a write failed, as it does the
     other `pw-metadata` errors' `Display` texts (the existing practice, so rule 13's text module
     does not apply here); the window makes its own texts from the variants (T289).
  3. **Checking a text:** `pub fn check(text: &str) -> Result<(), VariableError>`: the first error
     of `text` by syntax and names, without any image (so never `NoCaptureTime`). A text without
     `{` and `}` is fine.
  4. **Filling a text:** `pub fn fill(text: &str, values: &VariableValues) -> Result<String,
     VariableError>`: each variable replaced by its value, `{{` and `}}` by one brace, the rest
     kept exactly (no trimming, no characters replaced; unlike the import's names, a value may
     hold `/` or `:`). The first error, in the order of the text, is returned and nothing is
     filled. `{make}` and `{model}` without a value give an empty text, not an error.
  5. **Filling a value:** `pub fn fill_value(value: &Value, values: &VariableValues) ->
     Result<Value, VariableError>`: `fill` on a `Text`, on every language of a `LangAlt` (its
     languages kept, in order), on every entry of a `List`, and on every field of a `Struct` and
     of each entry of a `StructList`, at any depth; a `Date` and a `Number` are returned as they
     are (U13-Q9: not in dates and numbers). Nothing is cleaned here: an entry or a text that is
     empty after filling stays, as T288's callers clean the result by their own rules.
  6. **Checking a value:** `pub fn check_value(value: &Value) -> Result<(), VariableError>`:
     `check` on every text `fill_value` would fill, in the same order, so the first error is the
     one `fill_value` would give (never `NoCaptureTime`). A template is checked with it before
     anything is written (T288's `TemplateFile::check_variables`, used by U13c and U13d).
  7. **Escaping:** `pub fn escape(text: &str) -> String`: every `{` doubled to `{{` and every
     `}` to `}}`, so `fill(&escape(t), any)` is `Ok(t)` for every text `t`. The window uses it on
     texts the user did not type (T289), so a brace another program wrote is kept as it is.
  8. A module comment that says what the variables are, where they are filled (T288: when an
     image is written, so the image gets the value and a template file keeps the variable) and
     that the capture time is the embedded Exif DateTimeOriginal until ADR 0020 C2 reads the
     corrected one.
- **Tests to add** (unit tests in `variables.rs`):
  - every one of the nine variables filled for a capture time `2026:11:22 14:30:05`, make
    `SONY`, model `ILCE-1`, `D:\Photos\2026-11-22 Trip\DSC01234.ARW`: `{date}` `2026-11-22`,
    `{year}` `2026`, `{month}` `11`, `{day}` `22`, `{time}` `143005`, and so on, also two in
    one text with literal text around them (`{name} from {folder}`);
  - `{ DATE }` and `{Make}` are known; `{{date}}` gives `{date}`; `a}}b` gives `a}b`;
  - errors: `{year2}`, `{job}`, `{seq:4}`, `{}` and `{a{b}` are `Unknown`; `{date` is
    `Unclosed("{date")`; `a}b` and `{date}}` are `UnexpectedClose`; the `Display` texts as
    point 2 gives them; a `capture_time` without seconds gives `NoCaptureTime` for `{time}`; the first of two errors is the one returned; `check` gives the same errors
    without values and is `Ok` for plain text and for `{{`;
  - `NoCaptureTime` for each of the five date variables without a capture time, while `{make}`
    and `{name}` work without one; `{make}` without a make is empty;
  - `VariableValues::of`: a DateTimeOriginal that is blank (`    :  :     :  :  `), all zeros or
    not a date gives no capture time; ` SONY ` is trimmed; a blank model is `None`; a path
    without a parent folder name gives an empty `folder`; `a.b.jpg` gives `a.b`;
  - `fill_value`: a Lang Alt with `x-default` and `sv` filled in both; a list's entries; a
    Location Shown entry (`StructList`) with City `{folder}` and `exif:GPSLatitude` as a
    `Number` (the number unchanged); a `Date` unchanged; `{make}` without a make gives an empty
    entry that is kept;
  - `check_value`: `Ok` for the Location Shown value above; a `{job}` in a structure's field and
    in a list entry is `Unknown`; the first of two errors is the one `fill_value` gives;
  - `escape`: `{x}`, `a}b`, `{{` and `{date` each come back unchanged through `fill`, and
    `check` accepts each escaped text.
- **Not in this task:** filling when an image is written and reading make and model (T288); the
  metadata window's check (T289); the template window and the import (U13c); the right-click
  menu (U13d).
- **Rules:** `docs/tasks/README.md` rules 1–12; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13b, first of the chain T287 → T288 → T289); waits for the cold read and the chain review |
| 2026-11-23 | Blocked | Cold read: the parsing pinned to the import's tokenizer (a `{` inside a name, `{date}}`), `str::trim` and `to_ascii_lowercase`; DateTimeOriginal trimmed of spaces and NULs before `from_exif`; `Unclosed` holds the brace; `NoCaptureTime` also for a capture time missing the part needed; every `Display` text given, and why rule 13 does not apply to them; tests added for these |
| 2026-11-23 | Blocked | Chain review (T287–T289): `check_value` (point 6) and `escape` (point 7, for T289) added, so a template can be checked before anything is written (T288's `TemplateFile::check_variables`, U13c, U13d) |
| 2026-11-23 | Open | Cold read and chain review done and worked in |
