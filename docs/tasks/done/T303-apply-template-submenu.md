# T303. The right-click menu's Apply Template submenu — Done

- **Why:** ADR 0017 U13-Q8, U13-Q26 and U13-Q27 (owner, 2026-11-24; part U13d); ADR 0019 §2.9
  (E-Q17, "later, with ADR 0017 U13: a submenu to apply a metadata template"). The right-click
  menu on images gets a submenu **Apply Template** after Metadata…, listing the templates
  folder's files and **Choose File…**; the chosen template is written to the target images
  after one question, in T302's batch. This is U13d's last task; the owner's hand check of U13d
  follows it.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/shell.rs`,
  `crates/pw-loupe/src/shell_text.rs`, `crates/pw-loupe/src/template_form.rs`,
  `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/lib.rs` (exports only),
  `crates/pw-loupe/src/metadata.rs` (only T302's `allow(dead_code)` attributes removed),
  `docs/glossary.md`, `docs/test-inventory.md`.
- **Background (read before starting):** ADR 0017 §6.5 U13-Q2, U13-Q8, U13-Q9, U13-Q19,
  U13-Q26, U13-Q27; ADR 0019 §2.9 (which images: E-Q16; how it looks: E-Q18). `shell.rs`:
  `Command`, `MenuEntry` (`ExternalPrograms` is the model of an entry drawn from data),
  `IMAGE_CONTEXT_MENU`, `menu_items` and the tests on `IMAGE_CONTEXT_MENU`
  (`the_image_context_menu_holds_the_image_commands_in_order`,
  `every_item_of_the_image_context_menu_is_the_menu_bars_item`). `shell_text.rs`:
  `command_text`, `ShellText`, `submenu_text`. `main.rs`: `draw_menu_bar`, `menu_entry`,
  `image_context_menu` (and its four call sites), `external_programs_menu`, `command_enabled`,
  `run_command`, `right_click`, `key_targets`, `marked_subjects` (how "Apply to {n} Marked"
  resolves paths with `resolve_path`), `ImportTemplateWait` (the model of a wait for the
  template thread), `poll_template_io`, `template_file_dialog`, `template_question`,
  `self.message` and the two places that build the status line's parts (the contact sheet's and
  the loupe's). `template_form.rs`: `templates_folder`, `TemplateText`, `template_text`,
  `load_failed_text`, `not_loaded_lines`. `editor.rs`: `variable_error_text`,
  `EditorText::MoreUnsaved`. `metadata.rs`: T302's `start_template_batch`,
  `template_batch_progress`, `take_template_batch_done`, `BatchSummary` (`status_lines`).
  `pw_metadata::supports_rating` (which file kinds hold metadata).
- **What:**
  1. **The entry** (`shell.rs`): a new `MenuEntry` kind (for example `Templates`), placed in
     `IMAGE_CONTEXT_MENU` right after Metadata… (before its separator); the menu bar does not get
     it (U13-Q8 names only the right-click menu). Its commands carry what was chosen without a
     path in `Command` (which is `Copy`): `Command::ApplyTemplate(u16)`, an index into the list
     kept for the open menu (point 2), and `Command::ApplyTemplateFile` for Choose File…;
     neither has a key. Every exhaustive match is extended: `menu_items`, `draw_menu_bar` and
     `menu_entry` for the entry (`draw_menu_bar` draws nothing for it), `run_command` and
     `command_text` for the commands (`command_text` gives `Apply Template` and `Choose File…`).
     The test that every context-menu item is the menu bar's item skips this entry, as only the
     right-click menu has it; the order test lists it.
  2. **The list** (`template_form.rs`, a plain function exported from `lib.rs`, for example
     `template_files(folder: &Path) -> Vec<PathBuf>`; U13-Q26): the files of the folder itself
     whose extension is `xmp` in any case, not its subfolders (`.saving` included), sorted by
     name without regard to case; a folder that is missing or cannot be read gives an empty
     list. **Read once per opening of the menu:** egui gives no "opened" event and
     `menu_entry` takes `&self` inside the menu's closure, so the list is kept in a field that
     can be set through `&self` (a `RefCell<Option<Vec<PathBuf>>>`) or in egui's memory: filled
     when the submenu is drawn and it is empty, and emptied only after the frame's commands have
     run and no popup is open (`egui::Popup::is_any_open`), so `run_command` can still turn
     `ApplyTemplate(i)` into its path in the frame of the click. The submenu's items are the
     names without `.xmp`, then a separator and Choose File…; with an empty list only Choose
     File…. The submenu's text `Apply Template` is a new `ShellText` variant.
  3. **Greyed** (U13-Q27): the submenu's items while a batch of either kind runs
     (`batch_running`), while a template chosen here is being read (point 4), or without a
     current image. The metadata window is modal (D16), so the menu cannot be opened while it is
     open.
  4. **Choosing** (U13-Q27): Choose File… opens `template_file_dialog`'s open dialog; Cancel there
     does nothing. A listed file, or the file chosen there, is sent to the template thread
     (`TemplateIo::load`). The targets are taken **when the item is chosen** (for Choose File…
     before its open dialog opens): `key_targets`
     (the marked images when the right-click was on a marked image, else the clicked one, which
     `right_click` made current; E-Q16), each resolved with `resolve_path` as `marked_subjects`
     does, less the files `pw_metadata::supports_rating` refuses (counted as left out); this
     follows E-Q16 as every right-click command does, which is U13-Q8's "every marked image
     (the clicked image when none is marked)" for a click on a marked image. A wait
     (a field of `Loupe` like `ImportTemplateWait`: the id, the path, the targets and the number
     left out) holds them until the answer, which `poll_template_io` takes by its id before the
     template window's answers. While the metadata window is open the answer is held and acted on
     when it closes (no question over the window). While the wait exists a repaint is asked
     for, as for the import's wait. Then, each message box titled `Metadata Template – {name}`
     (`TemplateText::Title` with the name) with OK only (a new helper beside
     `template_question`):
     - not read: `load_failed_text`; the thread stopped (also when the load cannot be sent):
       `LoadStopped`'s text; no question;
     - a read with no field: `The template {name} holds no field.`; a variable that cannot be
       read (`TemplateFile::check_variables`): `The template {name} cannot be applied: {reason}`
       with `variable_error_text`;
     - no target left: `None of the images can hold metadata.`;
     - otherwise the question (`template_question` as it is, titled `Metadata Template`,
       buttons **Apply** and Cancel), one `TemplateText` variant holding the name, both counts
       and the not-loaded lines: `Apply the template {name} to {n} images?` (`1 image` in the
       singular), then, when some targets were left out, `{m} images without metadata support
       are left out.` (singular likewise), then one line each of `not_loaded_lines` of the read
       (what the file held that was not loaded);
     - on Apply, `start_template_batch` with the targets and the read `TemplateFile`; when it is
       refused because a batch or the metadata window's own save started meanwhile, the message
       box `Metadata is being written. Try again when it is done.`
     `{name}` is the file name without `.xmp`.
  5. **While it runs and after** (U13-Q27): both status-line builders add, after
     `self.message`, `Applying the template {i} of {n} …` from `template_batch_progress`. At its
     end (`take_template_batch_done`, taken every frame in the main window, also while the
     metadata window is open), with T302's `allow(dead_code)` attributes removed:
     - nothing failed and no notice: `self.message` gets `Template {name} applied to {n}
       images.` (`{n}` the written count, singular as above);
     - otherwise the message box titled as in point 4 lists `status_lines` of the summary (the
       written count first, then the failed files, then the notices), at most the first 20 lines
       and then `EditorText::MoreUnsaved`'s `… and {more} more`; `self.message` gets the
       written-count line.
     The template's name is kept for these texts in a `Loupe` field set when the batch starts
     (T302's batch holds no name). While a template batch runs a repaint is asked for
     (`request_repaint_after`), so the progress moves without a mouse move; the Tasks area's
     "Saving metadata: i of n" (`batch_progress`) shows it too, which is accepted. The rating and inspector views
     follow each write as after "Apply to {n} Marked" (through `EditsWritten`).
  6. **Texts** (rule 13): the question (one variant with both counts), Apply, the progress, the
     done line, the no-field, cannot-be-applied, no-target and batch-running texts as
     `TemplateText` variants in `template_form.rs` (whole sentences, singular and plural inside
     the function); the submenu's text in `shell_text.rs`. `docs/glossary.md`: one row for
     Apply Template, Choose File…, Apply and the question.
- **Tests to add:** `template_form.rs`: `template_files` on a temporary folder with `b.xmp`,
  `A.XMP`, `c.txt`, a subfolder `.saving` holding `d.xmp` and a subfolder named `e.xmp`,
  giving `A.XMP`, `b.xmp`; on a missing folder, empty; every new text (exact), singular and
  plural, the question with and without the left-out sentence. `shell.rs`: the context menu's
  order with the new entry after Metadata…, and that no menu of the menu bar has it;
  `command_text` of the two commands. `main.rs`, without a `Loupe`: the targets less the
  unsupported files (a plain function), and the choice between the done line and the message
  box's lines (capped at 20 and the "more" line) from a `BatchSummary` (a plain function). The
  test inventory's `pw-loupe` row and totals are updated.
- **Not in this task:** a key or a menu-bar item for Apply Template; a list of templates anywhere
  else; the filmstrip's right-click (ADR 0019 §2.9 has none); undo of a template write.
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Order:** after T301 and T302 are committed (it changes `main.rs` and `poll_template_io` after
  T301, and uses T302's batch).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At Done the reviewer adds to the CHANGELOG (Added) that a template can be applied to the marked
  images from the right-click menu, removes the item "the template submenu" from
  `docs/tasks/known-limits.md`'s right-click section, and at the hand check's commit removes the
  template item from `README.md`'s Planned list. **The owner's hand check of U13d** (T300–T303):
  in the metadata window Load Template… with a template holding Headline, Keywords with + and a
  Location Created City puts them in the form as unsaved edits (Keywords added after the image's
  own), a field hidden in the layout is named as not loaded, OK writes them; Save as Template…
  of an image with values saves a file that the template window shows with those fields
  checked; an image whose Headline holds `{x}` saved as a template and applied to another image
  writes `{x}` as text; the right-click submenu lists the templates folder's files by name and
  Choose File…; on three marked images Apply Template asks `Apply the template … to 3 images?`
  and writes all three, with the progress and the done line in the status bar; a template with
  `{job}` (hand-written) gives its message and no question; a batch with a failed image (a file
  made read-only) shows the message box; a JPEG with an open keyword conflict and a template
  with Keywords + is named as a conflict kept (U13-Q28), and an import with that template onto
  such a JPEG names it in the import's summary; a Load Template… with Location Created City
  keeps the form's other Location Created sub-fields; Save as Template… on a form with a field
  error, and on an image with no value, refuses with its text; Choose File… applies the file
  chosen; marked images including a file kind without metadata support name the left-out
  count in the question; Metadata… during a template batch opens a locked window showing the
  progress; an image deleted during a batch is listed as failed. At the same Done the reviewer
  adds to `docs/tasks/known-limits.md`: a move or delete of an image while a template batch
  still has it pending (moves wait only for rating writes; the write then fails and is listed);
  a template thread or metadata worker that stops with a request taken leaves the submenu
  greyed until the program restarts; a template write has no undo; a template batch on a
  backup or `subst` tab of a running import may write a file the import is about to write.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0017 U13-Q8, U13-Q26, U13-Q27, owner, 2026-11-24; part U13d). Waits for T301, T302, its cold read and the chain review of T299–T303 |
| 2026-11-24 | Blocked | Cold read (fresh agent); rewritten in place by the reviewer before any work. Fixed: `lib.rs` added (exports); every exhaustive match named (`draw_menu_bar`, `menu_entry`, `menu_items`, `run_command`, `command_text`); the list kept through `&self` (a `RefCell` or egui memory) and emptied only after the frame's commands, as egui has no "opened" event; a listed file read as Choose File…'s is; the wait (`Loupe` field) holding the targets; the targets resolved with `resolve_path` and filtered by `supports_rating`; greyed while a read waits; texts worded for applying (not `refused_text`'s "Not saved"), the Apply button, the left-out sentence in the question's variant, the "more" line; an OK-only box titled with the template's name; the progress after `self.message` in both status-line builders; T302's `template_batch_progress` named; the path of `known-limits.md`. ADR 0017 U13-Q27's wording settled with it (Apply, the greying) |
| 2026-11-24 | Blocked | Chain review (T299–T303; recorded in T299's log): `metadata.rs` added for removing T302's attributes; the name kept in a `Loupe` field; repaints while the read waits and while the batch runs; the answer held while the metadata window is open; the refusal text covers the window's own save; the question through `template_question` with its fixed title and the not-loaded lines in its variant; the targets per E-Q16 (stated), for Choose File… taken before its dialog; the hand check widened (import with U13-Q28, Location Created, Save refusals, Choose File…, the left-out count, the locked window, a deleted image); four known limits for the reviewer at Done |
| 2026-11-24 | Open | T302 Done and committed (999a910), T301 Done and committed (d6e0054); cold read and chain review done. Checked against them: T302's `start_template_batch`, `template_batch_progress`, `take_template_batch_done` and `allow(dead_code)` attributes are as named here; T301's window answer in `poll_template_io` (taken by its id after the import's, before the template window's) leaves room for this task's wait, the ids being distinct. Set Open (owner, 2026-11-24) |
| 2026-11-24 | Ready for review | Done by the task agent. `shell.rs`: `Command::ApplyTemplate(u16)`, `Command::ApplyTemplateFile`, `MenuEntry::Templates` after Metadata… in `IMAGE_CONTEXT_MENU`, `menu_items` skips it; tests: the order, no menu-bar menu has it. `shell_text.rs`: `ShellText::ApplyTemplate`, `command_text` of both commands. `template_form.rs`: `template_files`, `template_name` (file name without `.xmp`), `apply_variables_text` (wraps the crate-private `variable_error_text`, so `lib.rs` need not export it), `TemplateText` variants `ApplyQuestion`, `Apply`, `Applying`, `Applied`, `ApplyNoField`, `ApplyVariables`, `ApplyNoTarget`, `ApplyBatchRunning`. `lib.rs`: exports. `metadata.rs`: T302's three `allow(dead_code)` removed. `main.rs`: `ApplyTemplateWait` (answer held while the metadata window is open, acted on in `poll_template_io` once it is closed), `template_targets`, `TemplateBatchReport` / `template_batch_report`, `template_message` (OK only, titled with the name), fields `apply_template_wait`, `template_batch_name`, `template_list` (`RefCell`, emptied in `ui` after the commands when `Popup::is_any_open` is false), `templates_menu`, `command_enabled`, `run_command`, `apply_template_file`, `send_apply_template`, `apply_template_answer`, `finish_template_batch` and `template_batch_status` (both status lines, after `self.message`), repaints while the wait or a template batch exists. `load_failed_text` and `LoadStopped` are given the file name with `.xmp` (as elsewhere); the title and the new texts the name without it. Glossary: one row. Tests: 11 (shell 1, shell_text 1, template_form 5, main 4); test inventory pw-loupe 1425 -> 1436, totals 2569 passed, 78 ignored. check.ps1 all OK (after a `cargo fmt --all`), no clippy warnings |
| 2026-11-24 | Done | Reviewed against every point and test: the entry only in the right-click menu, the list read once per opening and emptied after the frame's commands (every context menu is in the main window), the greying, the targets taken at the choice and filtered, the answer held while the metadata window is open, the texts, the status line, the report capped at 20 lines; T302's attributes removed. No finding to fix. Noted, accepted: the held answer is acted on when the metadata window is closed even if another dialog has opened (the box is modal); the message box always uses the warning level. No independent review needed (one crate, the task's eight files, not the safe write). check.ps1 all OK (run by the task agent after its last change; 2569 passed, 78 ignored, no clippy warnings). CHANGELOG: one Added line. Leftovers: the four known limits added to `known-limits.md` (Templates in the metadata window), and the right-click section's note on the template submenu removed. Approved by the owner; task moved to done/, task index and plan updated. The owner's hand check of U13d (T300–T303) follows; at its commit the template item is removed from `README.md`'s Planned list |
