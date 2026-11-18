# T189. ADR 0018 B4: the Delete key, the menu item, the confirmation window and the deletes in the shell (pw-loupe) — Open

- **Chain:** T185 (B1) → T186 (B2) → T187, T188 (B3) → T189 (B4) → T190 (B5). The owner's
  answers are B-Q1–B-Q15 in ADR 0018 §3; how the steps fit is ADR 0018 §2.10.
- **Why:** ADR 0018 §2.1–§2.8, step B4. T185–T188 give the file part, the worker requests, the
  plan, the batch, the texts and the answers. This task wires them to the user: Delete and Image
  ▸ "Delete to Recycle Bin", the confirmation, the deletes sent behind queued rating writes, and
  the tabs, view cores, caches and undo list following.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/lib.rs` (the key),
  `crates/pw-loupe/src/shell.rs` (one command), `crates/pw-loupe/src/shell_text.rs` (its text),
  `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/metadata.rs` (only point 9),
  `docs/test-inventory.md`, and this task's own file (status and log). **Starts after T188 and
  T183 are committed** (T183 changes `batch_done`, `undo_done` and `undo_move` in `main.rs`).
- **What:**
  1. **Command:** `Command::DeleteToRecycleBin` (`shell.rs`) "Delete to Recycle Bin" (the key
     Delete, no modifier), the **last** item of the Image menu, after `Command::ToggleMode`, with
     a separator before it (`shell.rs` about line 698). Its text in `command_text`
     (`shell_text.rs`); `acts_in` gives `Where::Both` through its default arm; the arms of
     `command_enabled` and `run_command` are in `main.rs` (about lines 4428 and 4495):
     greyed while the active tab shows no current image (`tab.position().is_some()`, as the
     rating items) or has no folder (`tab.folder.is_none()`: such a tab's list is not followed
     after a delete, so it is not offered there). It is not greyed while a move or a delete
     runs: then it says so (point 3). Extend the menu tests (`shell.rs` about lines 1026–1077,
     `shell_text.rs` about lines 420 and 480; the key column shows "Delete").
  2. **Key** (`lib.rs`): `pub fn delete_key_action(key: egui::Key, ctrl: bool, shift: bool, alt:
     bool) -> bool`: `egui::Key::Delete` with no modifier. Read in `main.rs` right after the move
     keys, with the same guards (`!ctx.egui_wants_keyboard_input()`, nothing while
     `dialog_open()`), and it runs the command. Teach the test helper `command_of_key` (in `lib.rs`'s tests, about
     line 1003) the same order, in both views, so `every_menu_key_gives_its_items_command` holds.
     Delete is a key of no other key map.
  3. **Starting a delete** (`main.rs`, a new `delete_images(&mut self)` that the command runs):
     - while a move or an undo runs or waits (`self.moves.busy()`): the status line shows
       `delete_while_moving_text()`, nothing else;
     - while a delete is not `Idle`: `delete_running_text()`, nothing else;
     - the targets are `key_targets()` (in the contact sheet the marked images when several are
       marked and the current one is among them, otherwise the current one; in the preview only the current one, which
       `Selection::targets` already gives for `ViewMode::Loupe`, B-Q2), resolved with
       `resolve_paths` (an error goes to the status line); no targets, nothing;
     - otherwise `DeleteStep::Preparing { images, id: None }`, and the active tab's folder is
       kept in `Loupe` (`delete_folder: Option<PathBuf>`, cleared whenever the step is `Idle`
       again). Nothing is sent from here; the follow-up of point 5 sends it.
  4. **The confirmation window** (`main.rs`), while the step is `Confirming(plan)`:
     - drawn as an **`egui::Modal`** (egui 0.37), so the rest of the window takes no mouse input
       (tabs, library, thumbnails); its "should close" (Escape or a click outside) is Cancel.
       Content: a heading `delete_title_text()`; `delete_confirm_text(plan.images.len(),
       plan.files, plan.bytes)`; when `plan.refused` is not empty, under it
       `delete_refused_text(count, file name of the first, its reason)`; when `plan.partners` is
       not empty, `delete_partners_text` with their file names (B-Q18); buttons
       `delete_button_text()` and `delete_cancel_text()`, each with a fixed id (`id_salt`);
     - **Cancel is the default** (B-Q4): Cancel's button gets `request_focus()` on the first frame
       the window is shown only (a `bool` in `Loupe`, set when the step becomes `Confirming`), so
       Tab can still move the focus to Delete;
     - **keys:** `dialog_open()` does not stop every key (it guards only some), so a branch at the
       top of the key loop, as the metadata window's (about line 5178), takes every key while the
       step is `Confirming`: Escape cancels; Enter cancels unless the Delete button has the focus
       (`ctx.memory(|m| m.has_focus(id))`; then egui clicks it); every other key does nothing.
       `dialog_open()` also returns true while `Confirming`, so the menus are greyed;
     - Cancel → `Idle`, nothing sent. Delete → `Running(DeleteBatch::new(plan.images))`.
  5. **The follow-up** (every frame, right after `follow_moves`, a new `follow_deletes(&mut
     self, ctx)`):
     - **Preparing:** while `id` is `None`, `prepare_delete(images, SUPPORTED_EXTENSIONS as
       Vec<String>, the settings' companion_suffixes)`; `QueueFull` tries again next frame,
       `Stopped` shows `worker_stopped_text()` and goes `Idle`. When the `Prepared` answer with
       that id comes and the active tab is no longer of `delete_folder` (`same_folder`), the
       answer is dropped and the step is `Idle`, nothing shown (the window must not ask about a
       tab that is not shown; T190 uses the same rule). Otherwise it gives `plan_delete` (with `is_image` from `pw_raw::SUPPORTED_EXTENSIONS`); when `plan.images` is empty the status line shows
       `delete_refused_text` for `plan.refused` (or nothing when that is empty too) and the step
       is `Idle`; otherwise `Confirming(plan)`. The window of point 4 is shown only while
       no other dialog is open (the metadata window, the settings window or a shown import
       dialog, which can be opened while a delete prepares); until then the step stays
       `Confirming` and waits.
     - **Running:** `to_send(|path| metadata.is_pending(path))`, each sent with `delete_image`
       (the same two lists) and marked `sent`; `QueueFull` leaves the rest for the next frame;
       `Stopped` answers it at once as failed with `DeleteRefusal::Failed(worker_stopped_text().to_owned())`
       (`file` the image). The ids sent are kept in a `HashSet<pw_metadata::RequestId>` in
       `Loupe` beside the step (T187's `DeleteStep` has no field for them and is not changed); a
       `Deleted` answer whose id is not among them is ignored, and they are cleared when the
       step is `Idle` again. `Preparing`'s images are cloned for the request, so a `QueueFull`
       keeps them.
     - A frame is asked for (`request_repaint_after(MOVE_POLL_INTERVAL)`) while the step is
       `Preparing` or `Running`.
  6. **When the batch is done** (`finish`), in this order:
     - every tab of each folder of the deleted images (`image_folders`) drops the deleted files
       (`result.files`: the shell's own resolved paths, as T186 echoes them, including the files
       of a group Windows deleted only partly) through `replace_tab_files`, as
       `batch_done` drops moved files: the tab's paths after `resolve_paths`, compared with
       T187's `same_path`; the active tab's view cores and thumbnails follow
       (`active_reordered`). The compare needs no call: `follow_compare` already runs every
       frame and closes it when its held image left the list. `replace_tab_files`' errors go to
       the Tasks list and the status line as in `batch_done`. A tab switched or closed meanwhile
       is fine: the folder's tabs that are open are followed, others not;
     - `self.metadata.forget_deleted(&result.files)` (T188) and
       `self.moves.undo.forget_deleted(&result.files)` (T187);
     - each failure goes to the Tasks error list with `delete_failed_text` (or
       `delete_partly_failed_text`), `{name}` the file name of its `DeleteError::file`, as a
       move's do; the status line shows, set last:
       `deleted_text(n)` when nothing failed; for one failure `delete_failed_text` (or
       `delete_partly_failed_text` when its `left` is not empty, with each path of `left` as
       `display().to_string()`); for more `deletes_failed_text(count, last name, last reason)`;
     - the step is `Idle`.
  7. **Moves while a delete is busy** (ADR 0018 §2.10): `follow_moves` does not start a waiting
     batch while the delete step is not `Idle` (the batch waits; a key's move of an image that
     is then deleted fails with the file's "does not exist" reason, accepted); `undo_move` while
     the delete step is not `Idle` shows `delete_running_text()` and does nothing else. X still
     sets the rating at once, as always.
  8. Nothing is deleted on a card or a drive without a Recycle Bin: T185 refuses it in the
     prepare, so the status line names the reason and no window opens (ADR 0018 §2.6). A file
     open in another program passes the prepare (the view cores hold the current image then);
     at the delete, T186 releases the view cores' sessions first, so only another program's
     handle stops it, with T185's reason and nothing of that image deleted (§2.4).
  9. **`crates/pw-loupe/src/metadata.rs`** (only this): remove T188's
     `#[cfg_attr(not(test), allow(dead_code))]` and its comment lines from the functions this
     task uses. One the program still does not use keeps its attribute, and the report says
     which and why.
- **Tests to add:** `lib.rs`: Delete gives `delete_key_action` with no modifier and not with
  Ctrl, Shift or Alt, and is in no other key map. `shell.rs`, `shell_text.rs`: the order and
  text of the new item (extending the existing lists). The wiring in `main.rs` has no unit test
  (`Loupe` cannot be built in a test); the owner's hand check covers it.
- **Known limits** (to `backlog.md` when this task is Done): if the metadata worker stops
  after it took a request, its answer never comes and the delete stays busy, which also holds
  moves back (the moves have the same gap today); quitting while a delete runs waits only for
  rating writes, so the images not yet sent are not deleted (each image is all or nothing in
  Windows' call); a held Delete key repeats "A delete is still running"; nothing is shown while
  the prepare runs; between the first answer and the end of the batch, the deleted images
  are still in the list, and a rating key on one fails (as for the moves); a JPG partner left
  out of the plan is not waited for, so a rating write still queued for it fails after the
  delete (noise, not loss); a folder read already sent may put a deleted path back into the
  metadata cache (the tab's list is the source); a `_rejected` folder reached under another
  spelling is not matched by `same_path`, so a later Ctrl+Z on it fails safely; while a delete
  runs, the metadata window can be opened on an image being deleted, and its save then fails.
- **Not in this task:** "Delete Rejected Images" (T190). A setting to skip the confirmation, an
  undo inside RawStride, and deleting one file of a pair (ADR 0018 §5).
- **Rules:** `AGENTS.md` §3; `docs/tasks/README.md` rule 13 (the texts are T187's and this
  task's menu text in `shell_text.rs`, which matches T187's glossary row; no string literal in
  the drawing code). Deletes run while the user keeps working and this is the last wiring task
  of the chain in `main.rs`, so it gets an independent code review before the owner's approval.
- **Hand check by the owner** (ADR 0018 §4, B4), on a copy of R2's folder (`RAW\RAW-DxO`): Delete
  on an ARW with its `.ARW.dop` and `.XMP` asks with "1 image (3 files, …)", Enter cancels, a
  click on Delete moves the three to the Recycle Bin and the image leaves the contact sheet;
  "Restore" in the Recycle Bin puts them back in place, and the folder shows the image after it
  is opened again; several marked images go with one key; in the preview only the current image
  goes although several are marked; an ARW with a TIFF of the same name names the TIFF in the
  window; Tab moves the focus to Delete and Enter then deletes, while
  a click outside the window or Escape cancels; a RAW+JPEG pair goes together; a memory card (or a folder on
  it) and a network folder are refused with their reason and no window; a file open in another
  program stops its image, the message names it, and nothing of it is deleted; Delete while a
  move runs says so; after deleting images inside `_rejected`, Ctrl+Z does not try to move them
  back.
- **Done when:** `.\tools\check.ps1` is all OK with no clippy warnings and `git diff --stat`
  shows no change from this task outside the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked: waits for T188 and T183 | Added for ADR 0018 B4, written with B2–B5 for the chain review |
| 2026-11-17 | Blocked: waits for T188 and T183 | Cold read: doable in 7 files. Changed: `command_enabled` and `run_command` are in `main.rs`, `acts_in` needs no arm, the test places named; the item last in the Image menu, greyed in a tab without a folder; the window an `egui::Modal` with a key branch at the top of the key loop (since `dialog_open()` does not stop every key), Cancel focused only on the first frame, Enter deletes only when Delete has the focus; the sent ids and the focus flag kept in `Loupe`; paths compared with `same_path`, `follow_compare` needs no call, listing errors and the Tasks text placed; known limits listed (worker stopped after taking a request, quitting while running, key repeat) |
| 2026-11-17 | Blocked: waits for T188 and T183 | Chain review (fresh agent, T185–T190 read together): `delete_folder` kept and an answer for a tab no longer active dropped (the same rule as T190); the window waits while another dialog is open; `.to_owned()`; `result.files` are the shell's paths and include a partly deleted group's files; more known limits listed |
| 2026-11-17 | Blocked: waits for T188 and T183 | The window names the image files that go along (`plan.partners`, ADR 0018 B-Q18) |
| 2026-11-17 | Blocked: waits for T188 | T183 is Done and committed: waits for T188 only. T183 changed `batch_done`, `undo_done` and `undo_move` in `main.rs` and added `relist_tabs`: line numbers to be checked when this task is set Open |
| 2026-11-18 | Blocked: waits for T188 | From the independent review of T186: a delete that is refused at the worker (a file open in another program, or the drive or the size refused after the confirmation) has already released the view cores' sessions of the image, as a failed move does. The image reopens as after a failed move; the owner's hand check of this task looks at the image after such a refused delete. Answers are matched by `id` and the event's `image`, not by `DeleteError::file` (which for an error from `companions` is the image made absolute) |
| 2026-11-18 | Open | T188 is Done and committed (`ac84036`): set Open. Line numbers checked after T183–T188 and corrected: `command_enabled` and `run_command` about 4428 and 4495 (were 4376, 4443), the menu tests in `shell.rs` about 1026–1077 (were 1018–1070), `command_of_key` about 1003 (was 994), the metadata window's key branch about 5178 (was 5126); the Image menu's `ToggleMode` (about 697) and the `shell_text.rs` tests (about 419, 479) are where the brief says. T188's `#[cfg_attr(not(test), allow(dead_code))]` items for point 9 are in `metadata.rs` (`MetadataSubmit::prepare_delete`, `delete_image`, `DeleteAnswer`, `take_deletes`, `forget_deleted`) |
