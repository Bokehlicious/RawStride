# T285. Reading and saving a template file with ExifTool — Open

- **Why:** ADR 0017 U13, part U13a (U13-Q1, Q2, Q4, Q12; ADR 0007 §10, §17.1, revision 34). A
  template is saved as a **plain XMP file** with only standard values, plus **one RawStride
  property** that names the list fields whose + box is checked. T284 made the model
  (`template_file::TemplateFile`); this task saves it to a `.xmp` file and reads one back, also a
  file another program wrote. The template window and its Save…/Load… dialogs come in U13c.
- **Crates:** `pw-metadata`. **Files:** `crates/pw-metadata/src/template_file.rs`,
  `crates/pw-metadata/src/read.rs` (only visibility: `JSON_OPTIONS`, `XmpRead` and its fields
  `values` and `unreadable`, `xmp_values` and `ExifTool::read_json` made `pub(crate)`), `crates/pw-metadata/tests/template_file.rs` (new),
  `crates/pw-metadata/tests/template.rs` (one test, point 5), `docs/test-inventory.md`.
- **Background (read before starting):** `template_file.rs` as T284 left it (`TemplateFile`,
  `new`, `can_add`, `in_template`, `to_template`). `read.rs`: `JSON_OPTIONS`, `read_json`,
  `xmp_values` and `XmpRead` (the table's properties out of ExifTool's JSON, with values that
  cannot be read listed in `unreadable`). `write_plan.rs`: `plan`, `FileState`, `WritePlan`
  (`options` holds `-ec` when a value has a line break; `args` the `-TAG=value` arguments); with
  `FileKind::Raw` the plan writes XMP only, as for a sidecar. `exiftool.rs`: `ExifTool::start_with`
  (`pre_args` go before `-stay_open`), `execute`. `lib.rs`: `write_succeeded`. The reviewer
  checked with ExifTool 13.64 on 2026-11-23: `exiftool -TAG=value … -o new.xmp` creates an XMP
  file from nothing ("1 image files created"); a property of a namespace ExifTool does not know
  is **read** without any setting (as `XMP-<prefix>:<Name>` in `-G1` JSON) but can only be
  **written** with a user-defined tag in a config file given with `-config <file>` as the first
  arguments of the command line; `-o` never overwrites an existing file.
- **The namespace (proposed by the reviewer, approved by the owner 2026-11-23, ADR 0007 §10;
  never changed later, or older template files lose their + boxes):** prefix `rawstride`, URI
  `https://github.com/Bokehlicious/RawStride/ns/template/1.0/` (an identifier only, never
  fetched), property `rawstride:AddTo`, an unordered list (`rdf:Bag`) of XMP ids such as
  `dc:subject`. It is the namespace's only property (ADR 0007 §17.1 D1).
- **What:**
  1. **The ExifTool config** (`template_file.rs`): a constant with the config text that defines
     the namespace above (a user-defined table under `Image::ExifTool::XMP::Main`, group
     `XMP-rawstride`, `AddTo` writable as a `Bag` of strings), and
     `pub fn write_exiftool_config(folder: &Path) -> std::io::Result<PathBuf>`, which creates
     `folder` when needed and writes the text to `rawstride-exiftool.config` in it, only when the
     file is missing or its content differs, and returns the file's path. Document that an
     `ExifTool` used for templates must be started with `-config <that path>` as its first
     arguments (`ExifTool::start_with(program, &["-config", path])`; with `perl`, after the
     script); who starts it is U13c's choice.
  2. **Errors:** `pub enum TemplateFileError` (`Debug, Clone, PartialEq, Eq`, `Display`,
     `std::error::Error`) with at least: the name does not end in `.xmp` (any case); the
     template has no field (`TemplateFile::new` left nothing: an empty template is not saved);
     a value `Metadata::set` refuses (`ValueError`, for example more than three Image Creators);
     the write plan refused (`PlanError`); ExifTool failed (its text); a file operation failed (the path and the error's text); the
     file read back differs from the template (point 3); and the read errors of point 4.
  3. **Saving:** `ExifTool::write_template_file(&mut self, path: &Path, template: &TemplateFile)
     -> Result<(), TemplateFileError>` (an `impl ExifTool` block in `template_file.rs`, as
     `edit.rs` has):
     - the template is passed through `TemplateFile::new` first, so only checked fields with
       content and their + boxes are written;
     - the values are set on an empty `Metadata` (`Metadata::new()`, then `set`) and turned into
       ExifTool arguments by `write_plan::plan` with a `FileState` built field by field
       (`FileState` has no `Default`): `kind: FileKind::Raw`, `iim_present: false`,
       `iim_charset: CodedCharset::Absent`, `iim_values` empty, `digest: IimDigest::Missing`,
       `exif_present` and `xmp_present` their `Default`; with these the plan gives only the XMP
       arguments (the reviewer's cold read checked `write_plan.rs`); then one
       `-XMP-rawstride:AddTo=<id>` per id of `add`;
     - ExifTool writes them with `-o` to a **new file with a name of its own** in a subfolder
       `.saving` of the template's folder (made when needed; on the same drive, so the rename
       below stays a rename), named `<stem>-<process id>-<a counter>.xmp`, so two saves at once,
       in one program or two, never share a file, and a file left by a crash is not among the
       templates (U13c and U13d list only the files of the folder itself); a file of that exact
       name is removed first; the plan's `options` come first and the path after `-o` last, as
       `safe_write.rs` does;
     - **the write counts only** when ExifTool's answer says exactly one file was created and
       its error output holds no `Warning` and no `Error` line (an ExifTool started without the
       config only warns that `AddTo` is not defined and still creates the file: that is a
       failure); check the answer in `template_file.rs` (as `safe_write.rs`'s private
       `copy_written` does; do not change `safe_write.rs`);
     - the new file is read back (point 4) and compared with the template as passed through
       `new`: values with `==`, except a `Value::Number` (also inside a structure, such as a
       location's `exif:GPSLatitude`), which is compared as a number (both parsed as `f64`,
       equal within `1e-9` of the larger), as XMP stores coordinates in another form. If a date
       comes back in another form than written (for example `Z` against `+00:00`), stop and ask;
       on any difference, or when the write did not count, the error is returned and the file
       at `path` is not touched;
     - **every failure** after ExifTool was asked to write (ExifTool's error, ExifTool not
       answering or ended, the read-back, the comparison, the rename) removes the new file if
       it exists, and the `.saving` folder when it is left empty, before the error is returned;
     - only then the new file replaces `path` (`std::fs::rename`, which replaces an existing file
       on Windows); the question before saving over an existing file is the window's (U13-Q2).
       The rename fails while another program holds the file open (for example an ExifTool
       reading it to apply the template); that is the error, which the window shows (U13c,
       U13d). After a rename the `.saving` folder is removed when it is empty.
  4. **Reading:** `ExifTool::read_template_file(&mut self, path: &Path) ->
     Result<TemplateRead, TemplateFileError>`, where `pub struct TemplateRead { pub template:
     TemplateFile, pub skipped: Vec<String>, pub dropped: Vec<String>, pub unreadable:
     Vec<(String, String)> }` (`Debug, Clone, PartialEq, Eq`), so the window can tell the user
     which fields of the file were not loaded (U13c):
     - ExifTool reads the file with `JSON_OPTIONS` plus `-ExifTool:Error` and `-File:FileType`
       (the names `verify.rs` uses) and the path, through `read_json`; a missing file, an
       `ExifTool:Error`, or a `FileType` other than `XMP` (a text file or a JPEG named `.xmp`)
       is an error;
     - the table's properties come from `xmp_values`; each that is `in_template` becomes a
       checked field; one that is not (Rating, Label, a deprecated IIM property) is named in
       `skipped` by its XMP id, never applied; `unreadable` is passed on as `xmp_values` gives
       it; each value is tried with `Metadata::set` on an empty `Metadata` **as a check only**
       (`set` would give a Lang Alt without `x-default` one, so its stored value is never used;
       the value passed on is the one `xmp_values` gave); one `set` refuses (a kind it cannot
       be, or too many entries) is added to `unreadable` with the error's text and left out;
       keys outside the table are ignored; a property `new` drops as a whole (for example a
       Lang Alt with only `en-US`) is named in `dropped` by its XMP id; a part `new` drops
       (a second Location Created entry, a blank list entry) is not named;
     - `XMP-rawstride:AddTo` (a list, or a single text, as ExifTool gives it) gives `add`;
     - the result is `TemplateFile::new(fields, add)`, so a Lang Alt keeps only `x-default`, a
       second Location Created entry is dropped and an id in `add` that is not a checked list of
       plain text values is dropped (U13-Q12); a file without `rawstride:AddTo`, for example
       one another program wrote, loads with every + unchecked;
     - variables in a text (`{date}` and the others, U13-Q9) are read and kept as written; they
       are turned into values only when an image is written (U13b).
  5. **One whole path** (`tests/template.rs`, Windows only as that file is): a template file
     saved with `dc:subject` `["c"]` and its + box checked, and Headline `X`, read back,
     `to_template()`, and applied with `apply_template_safely` to the small JPEG with keywords
     `["a", "b"]` and Headline `Y` gives keywords `["a", "b", "c"]` and Headline `X`.
- **Tests to add** (`tests/template_file.rs`, against a real ExifTool started with `-config`;
  each test fails with an explanation when ExifTool is missing, as `tests/template.rs` does,
  docs/TESTING.md §4.5; each in its own temporary folder). Each integration test file is its own
  crate, so `tests/template_file.rs` gets its own helper, and `tests/template.rs` a second one
  next to its `exiftool()`: the config made by `write_exiftool_config` in the test's temporary
  folder, then `ExifTool::start_with(program, &["-config", path])`, or with `PW_EXIFTOOL_PL`
  `start_with("perl", &[pl, "-config", path])`; the existing `exiftool()` stays as it is.
  - **Round trip** of one template with: a text with `å ä ö` and a line break, a Lang Alt
    (`dc:description`), a list without + (`dc:creator`), a list with + (`dc:subject`), a date
    (`photoshop:DateCreated`), Location Created with City and Sublocation only, Creator's
    Contact Info with only an e-mail, a Location Shown list of two entries, and a text with
    `{date} {name}`, a Location Shown entry with `exif:GPSLatitude` `59.3293` and City
    `{folder}` (a variable inside a structure field, which the plan's structure syntax escapes),
    and a `photoshop:DateCreated` with a time and a time zone (`2026-11-23T14:30:00+01:00`): the
    file read back equals the template as passed through `new` (the number compared as point 3
    says), and `skipped`, `dropped` and `unreadable` are empty.
  - **Dropped:** another program's file whose `dc:description` has only `en-US` reads with
    `dropped` naming `dc:description`, and `Iptc4xmpCore:IntellectualGenre` in it is named in
    `skipped`.
  - **Not a template:** a `.xmp` file holding plain text, and the small JPEG copied to a name
    ending in `.xmp`, are the error.
  - **Only standard XMP plus the one property:** the saved file's text declares no namespace
    besides `x`, `rdf`, the standard ones of the fields written and `rawstride`, and holds
    `rawstride:AddTo` with `dc:subject` only; a template without + boxes gives a file without
    the `rawstride` namespace.
  - **Not saved:** a checked field without content and a + box on an unchecked field are not in
    the file; an empty template is the error, and no file is made.
  - **Another program's file:** an `.xmp` written by plain `ExifTool::execute` (no config) with
    keywords, `xmp:Rating` and `xmp:Label` reads with the keywords as a checked field, no
    + box, and `skipped` naming `xmp:Rating` and `xmp:Label`.
  - **Saving over a file** replaces it; **a failed save** (an ExifTool started without
    `-config`, so `AddTo` cannot be written) leaves an existing file at `path` byte for byte as
    it was, and leaves no `.saving` folder behind; a name ending in `.txt` is the error; two
    saves one after the other to the same path both succeed and the second's content is the
    file's.
  - **The config file:** `write_exiftool_config` makes the folder and the file, writes the same
    text again without changing the file's modification time, and writes it again when the file
    differs.
  - **A missing file** to read is the error.
- **Not in this task:** where templates are kept and the dialogs (U13c), which process starts
  ExifTool with the config, and a config path with letters outside the Windows code page (the
  path goes on the command line, `tests/safe_write.rs`'s comment on code pages; U13c), variables (U13b), the window, the import and the right-click
  menu (U13c, U13d).
- **Rules:** `docs/tasks/README.md` rules 1–12; no new dependency; no `unwrap`/`expect` in
  non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13a, second of the chain T284 → T285); waits for T284 and the owner's approval of the namespace |
| 2026-11-23 | Blocked | Cold read: the read adds `-ExifTool:Error` and `-File:FileType` and refuses a file that is not XMP; `XmpRead`'s fields made `pub(crate)`; error variants for `ValueError` and `PlanError`; a value `Metadata::set` refuses goes to `unreadable`; `FileState` spelled out; the write counts only with one file created and no warning; numbers compared as numbers in the read-back check (a GPS latitude in the round trip); test helpers with `-config` spelled out; a config path outside the code page left to U13c |
| 2026-11-23 | Blocked | The owner approved the namespace as proposed (asked whether the address matters if the repository moves: no, it is a name only); recorded in ADR 0007 §10. Waits for T284 |
| 2026-11-23 | Blocked | Chain review (with T284): the new file goes to a `.saving` subfolder with a name of its own (two saves at once, a crash leftover not listed as a template); every failure removes it; a locked target is the error; `set` only a check when reading; `TemplateRead::dropped` names what `new` dropped as a whole; round trip with a variable in a structure field and a date with a time zone. T284 got: `in_template` as the window's explicit list, blank entries and duplicates dropped from lists, `check_kind` used |
| 2026-11-23 | Open | T284 done (reviewer, 2026-11-23); `TemplateFile::new` also drops a list of structures over its limit (ADR 0017 U13-Q5) |
