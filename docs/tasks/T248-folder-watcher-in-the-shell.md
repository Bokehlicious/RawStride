# T248. ADR 0026 F2b: the folder watcher follows the tabs and ends before an eject (pw-loupe) — Blocked: waits for T247

- **Why:** ADR 0026 step F2 (accepted by the owner 2026-11-18): the watcher's watches start and
  end with the tabs (§3.2, "a tab's watch ends when it is closed or opened on another folder"),
  and a watch must not keep a memory card busy, so it is ended before an eject (§3.2, "Network
  folders and removable drives"). T247 made `FolderWatcher`; this task gives it the tabs' folders,
  ends the watches on the drive of an import's source before that import is started with
  "Eject", and
  notes each event on the tabs it concerns. Acting on the notes (listing a folder again, the
  status texts, the fallback's listing on activation) is F3. Last task of the chain T247 → T248
  (plan, ADR 0026 F2 row); the owner's hand check of F2 follows it.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/tabs.rs` (points 1 and 2, and their tests)
  - `crates/pw-loupe/src/main.rs` (points 3–6, and their tests)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **Two notes on a tab** (`tabs.rs`, `Tab`): `pub changed_on_disk: bool` (a watch said the
     folder changed since the tab's files were last listed; ADR 0026 §7, "`Tab` gains 'changed
     on disk'") and `pub unwatched: bool` (its folder's watch could not start or has ended, F-Q3).
     Both false by default. `Tab::open` (a folder opened in the tab, also again) sets both to
     false, beside the fields it resets; `Tab::clear` already resets everything. Doc comments
     say who sets them (this task) and that F3 acts on them and clears `changed_on_disk` when it
     lists the folder.
  2. **`Tabs`** gains:
     - `pub fn folders_to_watch(&self, except_drive_of: Option<&Path>) -> Vec<PathBuf>`: every
       tab's folder, in tab order (duplicates kept; `FolderWatcher::set_folders` collapses
       them), leaving out tabs without a folder, tabs whose `unwatched` is set (so a tab opened
       again, which clears its note, brings its folder back into the set and the watch is tried
       again; see "Not in this task"), and, when `except_drive_of` is given, every folder on the
       same drive as that path. The eject is of the whole drive (`pw_ingest::eject` ejects
       `drive_of(source)`), so a tab on `E:\` or `E:\PRIVATE` blocks it as much as one inside
       the source `E:\DCIM\100MSDCF`. The drives are compared as `pw_ingest::drive_of` of each
       path's resolved form (`resolved_folder` in `tabs.rs`, the form `same_folder` uses), so a
       relative path, a `\\?\` prefix or a `subst` drive is compared by its real drive; a path
       with no drive letter (a network path) is never left out. It is in `tabs.rs` because
       `main.rs` is the binary and cannot reach `tabs.rs`'s private helpers;
     - `pub fn note_changed_on_disk(&mut self, folder: &Path)` and
       `pub fn note_unwatched(&mut self, folder: &Path)`: set the note on every tab whose folder
       is `same_folder` as `folder` (as `tabs_of_folder_mut` finds them).
  3. **The watcher in the app** (`main.rs`): a field `folder_watcher: FolderWatcher`, started
     where the other workers are made in `run_native`'s creation closure, with
     `wake` = a clone of `cc.egui_ctx` calling `request_repaint()`. A field
     `watched_folders: Vec<PathBuf>` holds the last set given.
  4. **Following the tabs:** a method `follow_tab_folders(&mut self)`, called once per frame in
     `ui` right after `poll_import_worker` and `poll_folder_watcher` (point 6, so an `Ended`
     noted this frame leaves the set at once) and again at the end of `ui` after the commands
     have run (so a tab opened, closed or cleared this frame is watched or ended the same frame). It
     computes the wanted set, `self.tabs.folders_to_watch(self.ejecting_source.as_deref())`
     (point 5); when it differs from `watched_folders` it calls `set_folders` and stores it. It
     does not wait.
  5. **Ended before an eject** (`start_import`): when `form.eject` is set, after
     `self.tabs.clear_inside(..)` and before `self.import_worker.run(..)` (which takes `form` by
     value, so the path is made from `form.source.trim()` before that call; the worker ejects
     the same trimmed text, `validate_source`), the import's source is stored in a field
     `ejecting_source: Option<PathBuf>`, the wanted set is given at once (point 4's method), and
     `wait_until_applied` is called with a named constant of 2 s. A `false` is logged with
     `eprintln!` and the import starts anyway (the eject reports its own failure, as today). The
     wait blocks the window for at most 2 s, only when a watch on the card does not end at once
     (accepted: it happens once, when the import is started). On the `Err` arm, where
     `self.tabs = previous_tabs` is restored, `ejecting_source` is cleared again.
     `ejecting_source` is cleared in `poll_import_worker` when the import ends: in the
     `Ok(event)` arm for `ImportEvent::Done` (sent on every end: a validation or scan failure, a
     cancel, failed files, and after the eject), before the `self.open(&folder, true)` calls of
     the same arm, so the result folders are watched at once; and on
     `TryRecvError::Disconnected`. While it is set, a tab opened on the card's drive (from the
     library, during a hidden import) gets no watch (point 4), so the card is not kept busy at
     the eject; such a tab is not marked `unwatched` (when the import ends its watch starts
     through point 4).
  6. **The events:** a method `poll_folder_watcher(&mut self)`, called right after
     `poll_import_worker` and before the first `follow_tab_folders`, drains `try_recv`: `Changed(folder)` →
     `self.tabs.note_changed_on_disk(&folder)`; `Ended { folder, reason }` →
     `self.tabs.note_unwatched(&folder)` and `eprintln!("pw-loupe: the folder {} is not watched:
     {reason}", …)`. Nothing else happens yet: no listing, no status text, no repaint beyond the
     wake (F3).
- **Tests to add:**
  - `tabs.rs`: `folders_to_watch(None)` gives the tabs' folders in order and leaves out a tab
    without a folder and a tab marked `unwatched`; with `Some(E:\DCIM\100MSDCF)` it leaves out
    `E:\`, `E:\DCIM` and `E:\PRIVATE` (lower-case `e:` too) and keeps `D:\Photos` and a network
    path `\\server\share\photos` (written paths; no file access needed for drives that do not
    exist, since `resolved_folder` falls back to the path as given); the two `note_` functions
    set the note on every tab of that folder, also written another way (a trailing separator,
    other case; under a temporary folder, as `tabs_of_folder_mut`'s test, since `same_folder`
    resolves on disk), and on no other tab; `Tab::open` on a tab with both notes clears them,
    and the folder is then in `folders_to_watch` again; `clear_inside` leaves a tab with no
    notes.
  - `main.rs`: none of its own; the methods only pass values between `Tabs` and
    `FolderWatcher`, which are tested in their modules. The hand check covers the wiring.
- **Hand check (the owner, F2 as a whole):** with a release build; only what is seen on the
  screen and in File Explorer is checked (the log lines are not shown in a release build):
  1. Open the memory card's top folder (for example `E:\`) in one tab and a folder on the PC in
     another. In the import window, choose a **subfolder** of the card as the source (for
     example `E:\DCIM\100MSDCF`), with "Eject" on, and import: the import runs and the card is
     ejected as before (the tab on another folder of the card did not keep it busy). The PC tab
     still works.
  2. Start an import of many files (a few hundred) from a card with "Eject" on and hide the
     import window while it copies; then open a folder on the card from the library: at the end
     the card is ejected.
  3. Close a tab on a PC folder, then rename that folder in File Explorer: it works. (What
     happens to a rename of a watched folder's **parent** while its tab is open is told by
     T247's log; the reviewer puts it here before this task is set to Open.)
  4. RawStride starts, opens folders, rates images and exits as before; nothing new is shown.
- **Not in this task:** listing a folder after `Changed`, the active tab at once and the others
  on activation, the status texts, the fallback's listing for an `unwatched` tab on activation
  and F5, a folder gone (F-Q4, F-Q5) — all F3; the F1 leftovers in `plans/adr-0018-0025-steps.md`
  ("Taken up when a step is written") — F3. A folder whose watch ended is not watched again until
  one of its tabs is opened again (point 2 then brings it back into the set); a second tab of the
  same folder keeps its `unwatched` note until it is opened again itself, which only costs F3's
  fallback a listing on its activation. F3 decides whether F5 tries the watch again.
- **Rules:** `docs/tasks/README.md` rules 1–12; no user-visible text (the log lines only);
  `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added (ADR 0026 F2, second of T247 → T248); waits for T247 |
| 2026-11-21 | Blocked | Cold read: the eject is of the whole drive, so `folders_to_watch` leaves out every folder on the source's drive (compared on the resolved form), not only the source's subtree; tabs marked `unwatched` left out so a tab opened again retries its watch; where the source path is taken (before `run` takes `form`), `ejecting_source` cleared before the result folders open, the order of `poll_folder_watcher` and `follow_tab_folders`, the 2 s wait accepted; `note_` tests under a temporary folder; hand check: a subfolder as the source with a tab on another folder of the card, a long hidden import, only visible results |
