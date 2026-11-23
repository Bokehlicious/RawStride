# T293. The template window — Done

- **Why:** ADR 0017 U13, part U13c (U13-Q2, U13-Q3, U13-Q4, U13-Q12, U13-Q13,
  U13-Q15), third of the chain T291 → T292 → T293 → T294 → T295 → T296. The user
  fills in a metadata template in a window of its own, in two columns, with a check box per field
  and a + box on the lists of plain text values, and saves it as an XMP file with **Save…** or
  opens one with **Load…**. Opened from **File ▸ Metadata Template…** (and, from T295, from the
  import dialog). T291 gives the form, its rules and texts; T292 reads and saves the files.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/shell_text.rs`,
  `crates/pw-loupe/src/lib.rs` (the `pub use shell::{…}` line only, for point 5's size function
  and constant), `docs/test-inventory.md`.
- **Background (read before starting):** `template_form.rs` (T291: `TemplateForm`, its rows,
  check boxes, `notice_typing`, `changed`, `load`, `template`, `mark_saved`, the column split,
  `templates_folder`, `with_xmp_extension`, its texts); `template_io.rs` (T292: `TemplateIo`,
  `TemplateEvent`, `SaveError`); in `main.rs` the metadata window's drawing
  (`draw_window_form`, `draw_window_field`, `draw_values_field` and `ValueUi`,
  `draw_choice_list`, `draw_structure_list`), the Settings window (opened centred and larger,
  U8-Q14), the existing `rfd::FileDialog` and `rfd::MessageDialog` uses, how the journal folder
  is found from `LOCALAPPDATA`, and how a menu `Command` is run; in `shell.rs` `Command` and
  `MENUS` and the File menu's tests; `shell_text.rs`'s command names.
- **What:**
  1. **The menu item:** `Command::MetadataTemplate` in `shell.rs`, in `MENUS`' File menu right
     after Import…, without a key; its name `Metadata Template…` in `shell_text.rs`. `keymap.rs`
     needs no change (a command without a key gives `None` there); `run_command` gets its arm.
     Like the other menu commands, it does nothing while another dialog is open (`run_command`
     returns early while `dialog_open()`).
  2. **The thread:** `main.rs` starts one `TemplateIo` (T292) with `TemplateIo::start(None, …)`
     (the program `ExifTool::start` uses, as the metadata worker) and `%LOCALAPPDATA%\RawStride`
     (`None` without `LOCALAPPDATA`), at the program's start, and reads its answers each frame
     (with a repaint while a request is waiting). When `start` fails, the program keeps no
     `TemplateIo`, and every Load and Save answers at once as T292's `TemplateIoStopped` (T291's
     text).
  3. **One template window** (`TemplateWindow` in `main.rs`, or a name of the agent's choice):
     the `TemplateForm`, the file it was loaded from or saved to (`None` at first), the status
     lines of the last load or save (each with an error flag, drawn in the theme's error colour
     when set, as T290 does), the errors of the last refused save by item, and the id of a
     request waiting for its answer. `open_template_window(path: Option<PathBuf>)` opens it, or
     brings the open one to the front; with a path it then loads that file (Load's steps, point
     7, without the open dialog), only into a window it newly opened (an open window is modal,
     so this case does not arise; a path is never loaded over an open form without U13-Q15's
     question). T295 calls it from the import dialog. So that the tests need
     no `Loupe` (no test builds one), the logic is on what can be built alone: for example
     `TemplateWindow::answer(&mut self, event: TemplateEvent) -> Option<Answered>`, which tells
     what happened (at least `Answered::Saved(PathBuf)` for a successful save; T295 uses it), and
     a function `open_template(slot: &mut Option<TemplateWindow>, path: Option<PathBuf>) ->
     Option<PathBuf>`, which gives the path to load; `Loupe::open_template_window` is a thin
     wrapper that sends that path to `TemplateIo::load` and keeps the id in the window.
  4. **Modality and keys:** the window is modal like the metadata window: a backdrop,
     `egui::Order::Foreground`, and every key goes to it (in the key loop after the delete and
     Open-in confirmations' branches and before the import dialog's, so an open import dialog
     behind it gets no key); it counts in `other_dialog_open()`, so no other dialog or menu
     command opens over it, and `open_window_shown()` (which lists its windows itself) gets
     `self.template_window.is_none()` too. Its backdrop is in `Order::Foreground` below the
     window (not `Order::Middle` as the metadata window's), so an import dialog (a window at
     `Order::Middle`) can never be clicked through it. Escape closes an open ▼ list first,
     otherwise it is the window's Close (point 9).
  5. **Drawing** (U13-Q13): an `egui::Window` with T291's title and a fixed `.id()` per opening
     (as the Settings window, so a changed title does not reset it), resizable, opened centred
     over the program window. Its size comes from a function of its own in `shell.rs` (for
     example `template_window_size`, with its constants: the program window less a margin, by
     default at least wide enough for two columns, a minimum height; the Settings window's size
     is capped below two columns), with a test there.
     Inside, the buttons **Clear Form**, **Load…**, **Save…** and **Close** stay in one row at the
     bottom, with the status lines above them; the rest scrolls:
     - **Two columns** when the window's inner width is at least twice a minimum column width
       (a constant, about 380 points, in `shell.rs` beside the size function), otherwise one.
       The sections go to the columns by T291's
       split, from each section's height measured in the previous frame (the first frame: its
       number of rows, T291's `Heading` rows included). Each section is drawn inside
       `ui.push_id(section)`, so its widgets keep their ids when it moves to the other column
       while the user types. Each section starts with a heading bar (its `SectionHeading` text,
       not collapsible).
     - **A field row:** the check box (`egui::Checkbox::without_text`, T291's hover text on it),
       the field's name as a label of its own (the field's own tooltip on it where it has one),
       then the + box where T291 gives one (`+`, its hover text), then the field: one line, or
       several for the long texts (`multiline`), a ▼ field as the metadata window's
       (`draw_values_field`, the same recent and own values; a save of a template adds nothing to
       the recent values), a choice list (`draw_choice_list`), or Copyrighted's own check box
       (`Checkbox::without_text` too). The window keeps its own open ▼ list and Escape flag (as
       the metadata window's `value_list` and `value_list_escape`); "Add to My Values" and ×
       change the own values and are saved as in the metadata window. Its errors from the last
       refused save under it, in the error colour.
     - **Creator's Contact Info** and **Location Created:** T291's `Heading` row with the name
       only, then its sub-fields on indented rows, each with its check box and name; a contact
       field is drawn as in the metadata window (with its ▼ where it has one), a Location Created
       sub-field as a one-line field in its one entry (`list_text_mut(…, 0, …)`); its error under
       it.
     - **Any other structure list:** a row with its check box and name, then the list as the
       metadata window draws it, below, without drawing its name a second time
       (`draw_structure_list` gets a flag, or its body is split out, with the metadata window
       unchanged); a sub-field's error under that sub-field, where `list_error` is shown today.
     - After the form is drawn, `notice_typing()`.
  6. **Clear Form:** T291's Clear Form; the file stays as the window's file.
  7. **Load…:** when `changed()`, first T291's question `Discard the changes to the template?`
     (`rfd::MessageDialog` with `MessageButtons::OkCancelCustom` of T291's `Discard` / `Cancel`
     and T291's window title as its title; Cancel does nothing). Then the Windows open
     dialog (`rfd::FileDialog`, no title), starting in `templates_folder(%LOCALAPPDATA%)`, made
     first when missing (U13-Q2; without `LOCALAPPDATA`, or when it cannot be made, no start
     folder), with T291's file type name and the extension `xmp`. A file chosen is sent
     to `TemplateIo::load`; while it waits, the whole form and Clear Form, Load…, Save… and Close
     are greyed, and Escape and × do nothing (a text typed while a save waits would otherwise be
     taken as saved, and one typed while a load waits would be lost). The
     answer: `Ok` → `load`, the window's file is that file, and the status lines are
     `Loaded {name}.` (normal), then the not-loaded line (`TemplateRead::skipped`, `dropped` and
     `load`'s own list) when there is something, and one line per `unreadable` value, both as
     errors; `Err` → one error line, the form unchanged. The errors of a refused save are
     cleared at the next Save…, Load or Clear Form.
  8. **Save…:** `template()` first. Refused → its errors under their items and one status error
     line (T291's text for the reason), no dialog. Otherwise the Windows save dialog
     (`rfd::FileDialog::save_file`, no title), starting in the templates folder (made when
     missing, as for Load), with
     the window's file's name as the suggested name when it has one, T291's file type name and
     the extension `xmp`. The Windows dialog asks before replacing the name as typed (U13-Q2), and
     already adds `.xmp` to a name typed without an extension. The path chosen goes through
     `with_xmp_extension`, which is then needed only for a name such as `a.txt`; when that added
     `.xmp` and a file of the new name exists, T291's question `{name} already exists. Replace
     it?` (`Replace` / `Cancel`, built as the Discard question) comes first. Then
     `TemplateIo::save`; while it waits the form and the buttons are greyed as for Load. The
     answer: `Ok` → `mark_saved()`, the window's file is that path, the title names it, the status
     line `Saved {name}.`; `Err` → one status error line by T291's texts (`PercentInFolder`, the
     other `TemplateFileError`s, `NoConfig`, the stopped reader), the form unchanged.
  9. **Close**, the window's ×, and Escape (point 4): when `changed()`,
     first the Discard question; then the window is closed and its form dropped. An answer that
     arrives after the window was closed is ignored. While a request waits, Close, × and Escape
     do nothing (point 7).
  10. All user-visible texts come from `template_form.rs` (T291), `editor.rs` (section headings,
      field names) and `shell_text.rs` (the menu item); a text that is missing there is not
      written in `main.rs`: stop and ask (rule 13). The same holds for an item T291 or T292 do not
      export: stop and ask, do not change their lines of `lib.rs`, `template_form.rs` or
      `template_io.rs`.
- **Tests to add:** in `shell.rs`, the File menu holds `Metadata Template…` right after Import…
  with no key; in `shell_text.rs`, the command's name; in `shell.rs`, the window's size function
  (default wide enough for two columns, clamped to a small program window); in `main.rs`, on the
  pieces of point 3 with hand-made `TemplateEvent`s: `open_template(…, None)` twice gives one
  window; an answer with another id than the waiting one changes nothing; a `Saved` answer `Ok`
  makes the window's file that path and `changed()` false; a `Loaded` answer `Err` leaves the
  form as it was and gives one error line; a `Loaded` answer `Ok` with a skipped field gives
  `Loaded {name}.` normal and the not-loaded line as an error. The drawing is checked by the
  owner's hand check (T295).
- **Not in this task:** the import dialog's row and Edit… (T295); Load Template… and Save as
  Template… in the metadata window and the right-click submenu (U13d); a list of templates in
  the window (U13-Q3: files are renamed or deleted in File Explorer).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At Done the reviewer adds the CHANGELOG line (Added: File ▸ Metadata Template…, a window to
  fill in a metadata template with a check box per field and a + box on lists, saved and loaded
  as an XMP file).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, third of the chain T291 → T292 → T293 → T294 → T295 → T296). Waits for T291 and T292, the owner's answer to U13-Q15, its cold read and the chain review |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
| 2026-11-23 | Blocked | Cold read: point 4 "Modality and keys" added (modal like the metadata window, its place in the key loop, counts in `other_dialog_open()`, Escape closes a ▼ list first); the size from a function of its own in `shell.rs` (the Settings window's cap gives one column); the tests on a `TemplateWindow` and an `open_template` function, not a `Loupe`; `TemplateIo::start(None, …)`, a failed start answers as stopped; `draw_structure_list` without its name a second time; the ▼ fields' own list and Escape flag, own values saved, contact fields with ▼; a fixed `.id()` per opening; `push_id` per section; `Checkbox::without_text`; the File-menu sentence dropped (no test lists the menu); dialogs: no title on file dialogs, `OkCancelCustom` with T291's title, no start folder without `LOCALAPPDATA`; Windows adds `.xmp` itself; status line colours, errors cleared, list sub-field errors placed; Close greyed while a request waits; missing exports: stop and ask (T291 and T292 list them) |
| 2026-11-23 | Open | T291 and T292 done (b3c3231 and the commit of T292's Done); its cold read and the chain review were done before (see T291's log). Set Open |
| 2026-11-23 | Ready for review | Done by the agent. `Command::MetadataTemplate` after Import… (no key); `template_window_size` with `TEMPLATE_COLUMN_MIN_WIDTH` (380), `TEMPLATE_WINDOW_MIN`, a default of 840 × 760 and a 24-point margin in `shell.rs`; `TemplateWindow`, `Answered`, `open_template` and the window in `main.rs` (backdrop in `Order::Foreground` with the window kept above it by `set_sublayer`; `TemplateIo::start(None, %LOCALAPPDATA%\RawStride)` at start). `draw_structure_list` split into its name and `draw_structure_list_body`, which draws a refused save's sub-field error where it differs from `list_error` (metadata window unchanged); the sub-field text boxes got an id salt so they keep the focus when a section moves column. Tests: 3 in `shell.rs`, 1 in `shell_text.rs`, 8 in `main.rs`; the expected lists of menu names and menu keys in `shell_text.rs` got the new item. Noted: rfd is built without `common-controls-v6`, so the Discard and Replace questions show OK / Cancel, not Discard / Replace (both answers are handled). check.ps1 all OK, 2521 passed, 78 ignored, no clippy warnings |
| 2026-11-23 | Done | Reviewed: points 1–10 and the tests as written; the key loop's branch after the delete and Open-in questions and before the metadata window's and the import dialog's; `other_dialog_open()` and `open_window_shown()` count the window; the backdrop and the window in `Order::Foreground` (`set_sublayer`); greyed while a request waits, × and Escape ignored then; an answer after Close dropped; the metadata window's lists unchanged but for the text boxes' id salt. Reviewer's notes, no change: `a_refused_save_shows_its_errors_until_clear_form` uses an empty form, so it shows the file kept by Clear Form more than errors cleared (the clearing is three lines, seen in the code). Leftover: rfd is built without `common-controls-v6`, so the Discard and Replace questions show OK and Cancel; both answers work. Home: `backlog.md`, "Waiting for a decision" (turning the feature on touches `Cargo.toml` and the test binaries, T47). `.\tools\check.ps1` run again by the reviewer: all OK, 2521 passed, 78 ignored, no clippy warnings; inventory row 1370 -> 1382 (library 1200 passed and 1 ignored, binary 181). CHANGELOG line added (Added). The drawing is hand-checked with U13c (T295). Moved to `done/`; T294 Open |
