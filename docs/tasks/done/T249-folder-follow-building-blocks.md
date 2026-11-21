# T249. ADR 0026 F3a: a listing that fails on an entry error, the watch's start, the status texts (pw-loupe) — Done

- **Why:** ADR 0026 step F3 (accepted by the owner 2026-11-18): a tab follows its folder on disk
  (§3.2, §3.3). Before the shell (T251) lists folders by itself, three building blocks are needed:
  - `list_folder` skips directory entries that give an error (`filter_map(e.ok())`), so a network
    hiccup looks like files removed and their marks are dropped. When listings come by themselves,
    that would happen without the user doing anything (leftover of T240–T242, in
    `plans/adr-0018-0025-steps.md`, "Taken up when a step is written").
  - A folder is listed when its tab opens, but its watch starts later (the end of the frame, an
    open on a helper thread; seconds on a network, and after an import's eject). A file that
    arrives in that gap is never shown. The watcher says when a watch has started, so the shell
    lists once more then (leftover of the chain review of T247–T248, same section).
  - The status texts of §3.3 (F-Q9).
  First task of the chain T249 → T250 → T251 (plan, ADR 0026 F3 row).
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/lib.rs` (point 1 and its tests)
  - `crates/pw-loupe/src/watcher.rs` (point 2 and its tests)
  - `crates/pw-loupe/src/shell_text.rs` (point 3 and its tests)
  - `crates/pw-loupe/src/main.rs` (point 2's one arm in `poll_folder_watcher`, nothing else)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **`list_folder` fails on an entry error** (`lib.rs`): an `Err` from the `read_dir` iterator
     makes `list_folder` return `Err` with the folder read (`dir`, also when a file was given) and
     the error, written as its other errors (`"{dir}: {error}"`), instead of skipping the entry. Nothing else changes: an entry that is
     not a file or not listed (`is_listed`) is still left out, and an entry whose `is_file` check
     fails (the file went away between the listing and the check) is still left out, not an
     error. The doc comment says why (a listing with entries missing would drop their files and
     marks from the tab). What the user sees: an entry error at Reload Folder now gives
     `folder_gone_text` ("The folder is gone or cannot be read", `reload_tab`), not a listing
     with files missing; this is meant.
  2. **`WatchEvent::Started(PathBuf)`** (`watcher.rs`): sent once per watch, on the thread, as soon
     as the folder's first `ReadDirectoryChangesW` has been issued (in `WatchThread::opened`, the
     `Ok(watch)` arm after `start_read`), so a change made from then on
     is seen. It names the folder as `Changed` and `Ended` do (the first spelling given, T247
     point 1), and `wake` is called after it as after the other events. A watch that cannot start
     sends `Ended` as today and no `Started`; without a thread nothing new is sent (every folder
     is answered with `Ended`). A folder that leaves the set and is given again gets a new
     `Started` when its new watch starts. The enum's and `Started`'s doc comments say what the
     shell does with it (lists the folder once more, ADR 0026 F3: the gap between a tab's listing
     and its watch). `pw-loupe` `main.rs` matches on `WatchEvent` without a wildcard
     (`poll_folder_watcher`), so add an arm there that does the same as `Changed` (`note_changed_on_disk`), and
     only that, and say so in the report (T251 relies on it).
  3. **The status texts** (`shell_text.rs`, rule 13, beside `removed_by_another_program_text`):
     `pub fn folder_changed_text(added: usize, removed: usize) -> Option<String>`, a whole sentence
     made from one template per case, singular and plural inside the function (ADR 0026 §3.3,
     F-Q9):
     - `added` only: "1 new image" / "3 new images";
     - `removed` only: "1 image removed by another program" / "2 images removed by another
       program";
     - both: "3 new images, 2 removed by another program" (each number singular or plural as
       above: "1 new image, 1 removed by another program");
     - neither: `None`.
     The doc comment says it is the status line after RawStride listed a tab's folder because it
     changed on disk (T251), and that the current image removed has its own text
     (`removed_by_another_program_text`, F-Q5).
- **Tests to add:**
  - `lib.rs`: the existing `list_folder` tests still pass. An entry error cannot be made on
    purpose in a temporary folder; test what can be tested (a `list_folder` case with a `.pwtmp-`
    and a `.pwbak-` name left out; a subfolder with a listed extension is already covered) and say in the report that the error branch is
    covered by reading.
  - `watcher.rs` (with a temporary folder, as T247's tests): a folder given gets `Started` before
    any `Changed`, and a file made after `Started` gives a `Changed`; a folder that does not exist
    gets `Ended` and no `Started`; a folder taken out of the set and given again gets a second
    `Started`; without a thread (a `FolderWatcher` built in `mod tests` with `running: None`,
    then `set_folders`) only `Ended` is sent. Existing tests and helpers that now also receive
    `Started` are adjusted only by ignoring `Started` where they collect events: at least the
    helper `wait_until_watching` (it stops at the first event and asserts none came), and
    `a_folder_removed_at_once_while_opening_can_be_deleted_once_applied` and
    `quick_sets_apply_only_the_newest` (they assert no events, and a folder that becomes live
    there now sends `Started`). Say which in the report; rule 5: their checks themselves stay.
  - `shell_text.rs`: each case of `folder_changed_text`, 1 and more for each number, both, and
    neither.
- **Not in this task:** acting on `Started` and on the notes, the reads of only the new files, the
  shell's wiring (T250, T251).
- **Rules:** `docs/tasks/README.md` rules 1–13; `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  and `main.rs` (point 2's one arm).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Open | Added (ADR 0026 F3, first of T249 → T250 → T251) |
| 2026-11-21 | Open | Cold read: where `Started` is sent named (`WatchThread::opened`); the `main.rs` arm is certain (no wildcard); the error names `dir`; the user-visible effect at Reload Folder stated; the tests and helper that must ignore `Started` named (`wait_until_watching`, two tests asserting no events); the no-thread test built by hand in `mod tests`; the `lib.rs` test narrowed to `.pwtmp-`/`.pwbak-` in `list_folder` |
| 2026-11-21 | Blocked | Waits for the chain review of T249 → T251 (`docs/tasks/README.md`, before the first task is set to Open) |
| 2026-11-21 | Open | Chain review (T249 → T251) done; its findings were for T250 and T251 (removed files forgotten in the cache, `add_files` before `mark_unread`, an undo reading what it does not know); nothing changed here |
| 2026-11-21 | Ready for review | Points 1–3 done. `list_folder` returns `"{dir}: {error}"` on an entry error (covered by reading; a new test checks `.pwtmp-`/`.pwbak-` left out of a listing). `WatchEvent::Started` sent in `WatchThread::opened` after `start_read`; `main.rs` `poll_folder_watcher` handles `Started` exactly as `Changed` (`note_changed_on_disk`), nothing else. `folder_changed_text` in `shell_text.rs`, re-exported from `lib.rs` (a private module's unused `pub fn` would be dead code). Existing tests that now ignore `Started`: helper `wait_until_watching`, `a_folder_removed_at_once_while_opening_can_be_deleted_once_applied`, `quick_sets_apply_only_the_newest`; their checks unchanged. 6 tests added; check.ps1 all OK (2161 passed, 76 ignored), no clippy warnings |
| 2026-11-21 | Done | Reviewed: the diff follows points 1–3 and the tests asked for; only the named files changed; the `main.rs` arm is the one or-pattern with `Changed`. check.ps1 rerun by the reviewer: all OK (2161 passed, 76 ignored), no clippy warnings. Leftovers: the entry-error branch covered by reading only is noted in `docs/test-inventory.md` (pw-loupe row); acting on `Started` is T251. No hand check (T251's covers F3). T250 set to Open |
