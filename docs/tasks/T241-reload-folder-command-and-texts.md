# T241. ADR 0026 F1b: the "Reload Folder" command, its key and texts, and reads forgotten on a reload (pw-loupe) — Open

- **Why:** ADR 0026 §3.1 (F-Q1, accepted by the owner 2026-11-18): a command **"Reload Folder"**
  in the menu that holds "Open Folder…", key **F5**; F5 over the library still reads the drives
  again (U4-Q1). A reload reads **every image's rating and metadata again** and drops what was
  read before, so a file changed by another program (a rating from another program, an edit of
  the pixels) is shown as it is now. Today a tab load of the same folder keeps the inspector's
  metadata, its Exif summary and its histograms (`folder_loaded` and `begin_load` keep them when
  the same folder is loaded again, T110, T144), which is right for a switch between the contact
  sheet and the preview, but not for a reload. A folder that is gone leaves the tab as it was and
  says so (F-Q4); a current image removed by another program gives its own text (F-Q5). This task
  makes the command, the key test, the texts and the "forget on reload" of the three readers;
  T242 wires them in. Second task of the chain T240 → T241 → T242 (plan, ADR 0026 F1 row).
  The thumbnails need nothing here: `load_active_tab` starts a new generation, and
  `ThumbWorker::set_generation` empties the thumbnail cache and refuses an older generation's
  results (`pw-thumbcache` `worker.rs`); the view core's `open_folder` empties every cache of the
  preview (`pw-viewcore` `state.rs`).
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/shell.rs` (point 1 and its tests)
  - `crates/pw-loupe/src/lib.rs` (point 2, the re-exports of point 3, and its test)
  - `crates/pw-loupe/src/shell_text.rs` (point 3 and its tests)
  - `crates/pw-loupe/src/main.rs` (point 7 only: two match arms)
  - `crates/pw-loupe/src/metadata.rs` (point 4 and its test)
  - `crates/pw-loupe/src/inspector.rs` (point 5 and its test)
  - `crates/pw-loupe/src/histogram.rs` (point 5 and its test)
  - `docs/glossary.md` (point 6)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **`Command::ReloadFolder`** (`shell.rs`): a new variant, in `MENUS`' File menu right after
     "Open Folder…", shown with the key F5 (`plain(Key::F5)`). `acts_in` gives `Where::Both`
     (the default arm). It is not in `IMAGE_CONTEXT_MENU` (it acts on the folder, not the images
     clicked); add it to that constant's doc comment's "Left out on purpose" list.
  2. **`reload_key_action(key: egui::Key, ctrl: bool, shift: bool, alt: bool) -> bool`**
     (`lib.rs`, beside `delete_key_action`): F5 with no modifier. Its doc comment says that F5
     with the pointer over the left column (the library and the Tasks area) is the library's
     "Refresh" (U4-Q1), which the shell asks first, and every other F5 is "Reload Folder" (ADR
     0026 §3.1).
  3. **Texts** (`shell_text.rs`), each made by one function from one template (rule 13):
     - `command_text(Command::ReloadFolder)` → `"Reload Folder"`;
     - `folder_gone_text(folder: &Path) -> String` → `"The folder is gone or cannot be read:
       {folder}"` (the folder's `display()`), the status line when a reload finds the folder
       missing or unreadable (F-Q4);
     - `removed_by_another_program_text(file: &Path) -> String` → `"{file} was removed by
       another program"` with the file name only (its `file_name()`, the whole path when it has
       none), the status line when the current image is gone after a reload (F-Q5, §2 point 1).
     The two new functions are added to `lib.rs`'s `pub use shell_text::{…}` (the module is
     private), as the other status texts are. The existing tests
     `every_menu_item_text_in_menu_order` (insert `"Reload Folder"` after `"Open Folder…"`) and
     `every_menu_key_text_in_menu_order` (insert `Some("F5")` after `Some("Ctrl+O")`) get the
     new item, as every menu item is listed there; each of the two status texts gets a test of
     its own with a folder or a file name.
  4. **`MetadataCache::begin_reload`** (`metadata.rs`): the same as `begin_load` (same arguments
     and answer), and the inspector's metadata is always emptied (`InspectorRead::clear`), also
     when the same folder is loaded again; `begin_load` is unchanged. Its doc comment says it is
     for "Reload Folder" (ADR 0026 §3.1) and that a read in flight stays in flight as `clear`
     says. Make `begin_load` and `begin_reload` share their body (a private function with a
     flag, or `begin_reload` calling `begin_load` and then clearing), not two copies.
     `metadata.rs` is a module of the binary, so until T242 calls it, `begin_reload` gets
     `#[cfg_attr(not(test), allow(dead_code))]` as the module already does elsewhere; T242
     removes it.
  5. **`folder_reloaded(&mut self)`** (no arguments) on the Exif summary (`inspector.rs`, beside
     `folder_loaded`) and on the histograms (`histogram.rs`, beside `folder_loaded`): empties
     what is kept as `folder_loaded` does for another folder; a read in flight stays in flight
     as there. Doc comments name ADR 0026 §3.1. The Exif summary's type is `pub(crate)` in the
     binary, so its `folder_reloaded` gets the same `cfg_attr` as point 4 until T242.
  6. **`docs/glossary.md`**: a row "– | Reload Folder | the File menu item and F5 elsewhere than
     over the left column: the active tab's folder listed and read again (ADR 0026 F1); added
     by T241", right after the row for "Refresh"; in the "Refresh" row the words "(also F5 over
     the Library)" become "(also F5 with the pointer over the left column; F5 elsewhere is
     Reload Folder)".
  7. **`main.rs`**: `run_command` matches every `Command` without a catch-all, so it gets one arm
     `Command::ReloadFolder => {}` with the comment "wired in T242 (ADR 0026 F1)"; and
     `command_enabled` (which ends in `_ => true`) gets an arm `Command::ReloadFolder => false`
     with the same comment, so the menu item is greyed until it does something. Nothing else in
     `main.rs` changes.
- **Tests to add:**
  - `shell.rs`: the File menu holds `ReloadFolder` right after `OpenFolder`, with F5;
    `ReloadFolder` is not in `IMAGE_CONTEXT_MENU`; `acts_in` is `Both`.
  - `lib.rs`: `reload_key_action` is true for F5, false for Ctrl+F5, Shift+F5, Alt+F5 and F4.
  - `shell_text.rs`: point 3's tests.
  - `metadata.rs`, beside `a_folder_change_empties_the_inspector_and_the_same_folder_keeps_it`
    and with its test worker `FakeSubmit` (give it `Ok` ids for the reads; check the folder
    read with `read_attempts`): after a load and an inspector result for an image of the
    folder, `begin_load` of the same files keeps the inspector's result (as today) and
    `begin_reload` empties it; both send the folder read again. And a reload while writes are
    on their way: with one rating write queued and one sent (and one turn queued), after
    `begin_reload` the shown rating and orientation of those files are still the written ones
    (`RatingStore::read`'s and `OrientationStore`'s shown-versus-confirmed rule), and the answer
    of the folder read sent before the reload is ignored.
  - `inspector.rs` and `histogram.rs`, beside their `folder_loaded` tests: `folder_loaded` of
    the same files keeps the kept results (as today), `folder_reloaded` empties them, and a read
    in flight is still in flight afterwards.
- **Not in this task:** wiring the command and the key into `main.rs`, the new listing, the
  status line and the scroll (T242); the orientations read from a file's own Exif
  (`OrientationStore`'s embedded values) are not forgotten by a reload (`backlog.md`,
  "Leftovers of writing ADR 0026 F1"); the capture times read for the sort are kept (a changed
  capture time moves nothing, U14-Q5).
- **Rules:** `AGENTS.md` §3 and §7, `docs/tasks/README.md` rules 1–13 (rule 13 for the texts).
  No new dependency.
- **Wait for:** T240 Done (both change `lib.rs`). (T239, which also changed `metadata.rs`,
  `main.rs` and `docs/glossary.md`, is Done, 717a83a.)
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added; waits for T240 (`lib.rs`) and T239 (`metadata.rs`, `main.rs`, glossary) |
| 2026-11-21 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README and this file): doable with small fixes; the claims on the caches true; F5 has no other meaning in any key map. Changed after it: `main.rs` added for one empty arm in `run_command` (no catch-all, else it does not compile); `cfg_attr(not(test), allow(dead_code))` on the binary's two new methods until T242; `folder_reloaded` takes no argument (it would be unused); the two status texts re-exported from `lib.rs`; the two menu-list tests and `FakeSubmit` named; "the library" is the whole left column; the glossary's words named; T239's files in "Wait for" |
| 2026-11-21 | Blocked | Chain review of T240–T242: `command_enabled` gets `ReloadFolder => false` until T242 (else an enabled item that does nothing); a test that a reload keeps the shown ratings and turns of writes on their way and ignores the earlier read's answer |
| 2026-11-21 | Blocked | T239 Done (717a83a); waits only for T240 |
| 2026-11-21 | Open | T240 Done |
