# T293. The template window — Blocked

- **Why:** ADR 0017 U13, part U13c (U13-Q2, U13-Q3, U13-Q4, U13-Q12, U13-Q13,
  U13-Q15), third of the chain T291 → T292 → T293 → T294 → T295 → T296. The user
  fills in a metadata template in a window of its own, in two columns, with a check box per field
  and a + box on the lists of plain text values, and saves it as an XMP file with **Save…** or
  opens one with **Load…**. Opened from **File ▸ Metadata Template…** (and, from T295, from the
  import dialog). T291 gives the form, its rules and texts; T292 reads and saves the files.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/shell_text.rs`,
  `docs/test-inventory.md`.
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
     needs no change (a command without a key gives `None` there). The File menu's tests that
     list its entries get the new entry (an intended change, named in the report).
  2. **The thread:** `main.rs` starts one `TemplateIo` (T292) with the program the metadata
     worker uses and `%LOCALAPPDATA%\RawStride` (`None` without `LOCALAPPDATA`), at the
     program's start, and reads its answers each frame (with a repaint while a request is
     waiting).
  3. **One template window** (`TemplateWindow` in `main.rs`, or a name of the agent's choice):
     the `TemplateForm`, the file it was loaded from or saved to (`None` at first), the status
     lines of the last load or save (each with an error flag, drawn in the theme's error colour
     when set, as T290 does), the errors of the last refused save by item, and the id of a
     request waiting for its answer. `open_template_window(path: Option<PathBuf>)` opens it, or
     brings the open one to the front; with a path it then loads that file (Load's steps, point
     6, without the open dialog). T295 calls it from the import dialog.
  4. **Drawing** (U13-Q13): an `egui::Window` with T291's title, resizable, opened centred over
     the program window and larger than the metadata window (as the Settings window, U8-Q14).
     Inside, the buttons **Clear Form**, **Load…**, **Save…** and **Close** stay in one row at the
     bottom, with the status lines above them; the rest scrolls:
     - **Two columns** when the window's inner width is at least twice a minimum column width
       (a constant, about 380 points), otherwise one. The sections go to the columns by T291's
       split, from each section's height measured in the previous frame (the first frame: its
       number of rows). Each section starts with a heading bar (its `SectionHeading` text, not
       collapsible).
     - **A field row:** the check box with the field's name (T291's hover text on the check
       box, the field's own tooltip on the name where it has one), then the + box where T291
       gives one (`+`, its hover text), then the field: one line, or several for the long texts
       (`multiline`), a ▼ field as the metadata window's (`draw_values_field`, the same recent
       and own values; a save of a template adds nothing to them), a choice list
       (`draw_choice_list`), or Copyrighted's own check box. Its errors from the last refused
       save under it, in the error colour.
     - **Creator's Contact Info** and **Location Created:** a row with the name only, then its
       sub-fields on indented rows, each with its check box and name and a one-line field
       (Location Created's in its one entry, `list_text_mut(…, 0, …)`), its error under it.
     - **Any other structure list:** a row with its check box and name, then the list as the
       metadata window draws it (`draw_structure_list`), below.
     - After the form is drawn, `notice_typing()`.
  5. **Clear Form:** T291's Clear Form; the file stays as the window's file.
  6. **Load…:** when `changed()`, first T291's question `Discard the changes to the template?`
     (`rfd::MessageDialog`, `Discard` / `Cancel`; Cancel does nothing). Then the Windows open
     dialog (`rfd::FileDialog`), starting in `templates_folder(%LOCALAPPDATA%)`, made first when
     missing (U13-Q2), with T291's file type name and the extension `xmp`. A file chosen is sent
     to `TemplateIo::load`; while it waits, Clear Form, Load… and Save… are greyed. The answer:
     `Ok` → `load`, the window's file is that file, and the status lines are `Loaded {name}.`,
     the not-loaded line (`TemplateRead::skipped`, `dropped` and `load`'s own list) when there is
     something, and one line per `unreadable` value, the last two as errors; `Err` → one error
     line, the form unchanged.
  7. **Save…:** `template()` first. Refused → its errors under their items and one status error
     line (T291's text for the reason), no dialog. Otherwise the Windows save dialog
     (`rfd::FileDialog::save_file`), starting in the templates folder (made when missing), with
     the window's file's name as the suggested name when it has one, T291's file type name and
     the extension `xmp`. The Windows dialog asks before replacing the name as typed (U13-Q2). The
     path chosen goes through `with_xmp_extension`; when that added `.xmp` and a file of the new
     name exists, T291's question `{name} already exists. Replace it?` (`Replace` / `Cancel`)
     comes first. Then `TemplateIo::save`; while it waits the buttons are greyed as for Load. The
     answer: `Ok` → `mark_saved()`, the window's file is that path, the title names it, the status
     line `Saved {name}.`; `Err` → one status error line by T291's texts (`PercentInFolder`, the
     other `TemplateFileError`s, `NoConfig`), the form unchanged.
  8. **Close**, the window's ×, and Escape while the window has the focus: when `changed()`,
     first the Discard question; then the window is closed and its form dropped. An answer that
     arrives after the window was closed is ignored.
  9. All user-visible texts come from `template_form.rs` (T291), `editor.rs` (section headings,
     field names) and `shell_text.rs` (the menu item); a text that is missing there is not
     written in `main.rs`: stop and ask (rule 13).
- **Tests to add:** in `shell.rs`, the File menu holds `Metadata Template…` right after Import…
  with no key; in `shell_text.rs`, the command's name; in `main.rs`, as far as it can be done
  without drawing: `open_template_window(None)` twice gives one window; an answer of `TemplateIo`
  with another id than the waiting one changes nothing; a `Saved` answer `Ok` makes the window's
  file that path and `changed()` false; a `Loaded` answer `Err` leaves the form as it was and
  gives one error line. The drawing is checked by the owner's hand check (T295).
- **Not in this task:** the import dialog's row and Edit… (T295); Load Template… and Save as
  Template… in the metadata window and the right-click submenu (U13d); a list of templates in
  the window (U13-Q3: files are renamed or deleted in File Explorer).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, third of the chain T291 → T292 → T293 → T294 → T295 → T296). Waits for T291 and T292, the owner's answer to U13-Q15, its cold read and the chain review |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
