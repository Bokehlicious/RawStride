# T252. ADR 0026 F3d: Reload Folder starts the folder's watch anew (pw-loupe) — Done

- **Why:** the owner's hand check of ADR 0026 F3 (T251, point 10, 2026-11-21) failed: a folder
  on a USB stick was open in a tab, the stick was pulled out and put back, F5 was pressed in that
  tab, and an image then copied into the folder in File Explorer did not appear until a second
  F5. ADR 0026 §3.3 (F-Q3) says a tab whose watch has ended is watched again after F5. T251's F5
  (`reload_active_tab`) clears the tab's `unwatched` note, and `follow_tab_folders` gives the
  watcher a new set only when the set differs from the last one. The likely cause (not verified):
  Windows did not complete the old watch's read with an error when the stick was pulled, so no
  `WatchEvent::Ended` came, the tab was never marked `unwatched`, F5 changed nothing in the set,
  and the old watch stayed, on a handle that no longer gives any notification. The same happens
  when the watch did end but another tab of the same folder, opened after the `Ended`, keeps the
  folder in the set (`note_unwatched` marks only the tabs open at that moment): the thread keeps
  the entry as `State::Ended` and F5 changes nothing. The shell cannot restart a watch by giving
  a set without the folder and then one with it: `drain_control` keeps only the newest
  `Control::Set` of those waiting, so the two can collapse into one. This task gives the watcher a
  way to start one folder's watch anew, and F5 uses it, whichever way the old watch was lost.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/watcher.rs` (points 1–3, and their tests)
  - `crates/pw-loupe/src/main.rs` (point 4)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **The message:** `Control` gains `Again(PathBuf)`: start the watch of this folder anew.
     `FolderWatcher` gains `pub fn watch_again(&mut self, folder: &Path)`, with a doc comment:
     with a running thread it sends `Control::Again` and wakes the thread (a new
     `sys::Running::send_again(&self, folder: PathBuf) -> bool`, made as `send_set` is; when it
     returns false, the thread is treated as stopped exactly as `set_folders` does: logged,
     `running` set to `None`, `reason` set). Without a thread it does nothing: a folder in the set
     has already been answered with `Ended`, and the shell lists such a tab itself (F-Q3). It does
     not block. The non-Windows `sys` gets a `send_again` written as its `send_set` is
     (`match self.0 {}`).
  2. **On the thread:** `drain_control` keeps every `Again` folder received, in order. When a
     `Stop` was received they are dropped with the rest. Otherwise they are handled **after** the
     newest `Set` of the same drain has been applied and **before** the `opened` loop, so an
     `Again` acts on the newest set, and an open it replaces that came back in the same drain is
     closed through `opens_to_close` instead of going live and giving a `Started` first. For each
     folder, the entry `same_folder` finds in `entries`:
     - none (the folder is not in the set, for example a drive left out before an eject):
       nothing;
     - `State::Live(watch)`: the watch is ended with `begin_end` (its completion is then
       taken by `completed` from `ending` and gives no `Ended`), `quiet.forget` for the folder,
       and a new open is started for the same entry;
     - `State::Opening(key)`: `key` is pushed to `opens_to_close` and a new open is started;
     - `State::Ended`: a new open is started.

     "A new open" is the code `apply` uses for a folder in `change.open` (a new key from
     `next_key`, `spawn_open`, `State::Opening(key)`; a key that cannot be had, or a failed
     `spawn_open`, gives `State::Ended` and `WatchEvent::Ended` as there), moved into one
     private function that both `apply` and the `Again` handling call. The entry stays where
     it is in `entries` and keeps the folder's spelling. An `Again` does not change
     `to_publish`; a cancelled read in `ending` or a key in `opens_to_close` holds back
     publishing as it does for a set (`may_publish`).
  3. **The doc comments** that the new code would contradict are brought in line, and only
     these: `WatchEvent` (a watch started anew gives `Started` or `Ended` again, as a first watch
     does) and `WatchEvent::Ended` ("not watched again until it has left the set and been given
     again": or until `watch_again`); `FolderWatcher` (names `watch_again`); `State::Ended` (kept
     so it is not opened again while it stays in the set, unless `watch_again` asks);
     `ending` and `opens_to_close` (also watches and opens replaced by `watch_again`, for a
     folder still in the set); `wait_until_applied` ("Opens of folders still in the set do not
     count": except an open replaced by `watch_again`, which is waited for as one of a folder
     that left the set).
  4. **F5:** in `reload_active_tab`, when the active tab has a folder, the shell calls
     `self.folder_watcher.watch_again(&folder)` every time, before it lists the folder, whatever
     the tab's `unwatched` note was. When the folder is not in the watcher's set at that moment
     (its `unwatched` note left it out), the `Again` finds no entry and does nothing, and the
     cleared note puts the folder back into the next set (`follow_tab_folders`), which opens a new
     watch; when that set comes in the same drain as the `Again`, the open the set began is
     replaced once, which costs one open and still gives one `Started`. The doc comment of
     `reload_active_tab` says so. The new watch's `Started` notes the tab as changed, so the
     folder is listed once more when the watch runs (as at any watch's start, T249); that second
     listing is expected.
- **Tests to add** (in `watcher.rs`, with the temporary folder and the helpers the existing tests
  use; Windows only, as theirs, except the last). Each test that counts `Started` first waits for
  the folder's first watch (`wait_until_watching`) and takes the events so far, so a late first
  `Started` is not counted as a second one:
  - a watched folder given to `watch_again` gives exactly one more `Started`, and a file created
    in it afterwards gives one `Changed` naming the folder as given;
  - `watch_again` on a folder that is not in the set gives no event for it, and a folder that is
    in the set still gives `Changed` for a file created in it;
  - a folder given while it does not exist gives `Ended`; the folder is then created and given to
    `watch_again`: it gives `Started`, and a file created in it gives `Changed` (the path F5
    relies on when the entry is `State::Ended`);
  - `set_folders` without the folder followed at once by `watch_again` of that folder gives no
    `Started` for it, and `wait_until_applied` returns true within `STOP_WAIT` plus a margin (a
    guard that a folder outside the set is never opened; it does not tell the order of the two
    messages apart, and need not);
  - without a thread (built as `without_a_thread_only_ended_is_sent` builds it, so on every
    system): after `set_folders` and its `Ended` have been taken, `watch_again` of that folder
    sends nothing.
- **Not in this task:** noticing a stick pulled out or put back without F5 (a device
  notification, `WM_DEVICECHANGE`; ADR 0026 F-Q3 says no timer and no such watch); a watch that
  goes dead without an error while no one presses F5 stays dead until F5 (to `known-limits.md`,
  "The folder watcher", at Done); the old handle's cancelled read that never completes on a
  removed volume is dropped after `STOP_WAIT` and logged, as today; an `Again` that replaces an
  open still hanging on an unreachable network folder keeps `wait_until_applied` from returning
  true until that open returns, so an import that ejects right after such an F5 waits its 2 s
  (`EJECT_WATCH_WAIT`) and starts anyway (to `known-limits.md`, "The folder watcher", with "A hung
  open holds back applied", at Done).
- **Rules:** `docs/tasks/README.md` rules 1–13 (no new texts); `AGENTS.md` §3.
- **Hand check (the owner):** with a release build:
  1. T251's point 10 again: open a folder on a USB stick, pull the stick out, put it back, press
     F5 in that tab, then copy an image into the folder in File Explorer: it appears without a
     second F5.
  2. Optional, with a debug build (`cargo run -p pw-loupe`, which keeps the console window): pull
     the stick out and say whether the console shows "pw-loupe: the folder … is not watched".
     This tells whether Windows ended the old watch (the cause above) or not.
  3. A folder on the PC: press F5, then copy an image into the folder in File Explorer: it
     appears, and the status line says "1 new image".
  4. Open a tab on a folder on the card that is **not** the import's source. Start an import
     from the card with "Eject the card when the import is done", make that tab active and press
     F5 during the import: the card is still ejected when the import ends.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added from T251's hand check point 10 |
| 2026-11-21 | Open | Cold read: point 4 calls `watch_again` on every F5 (a tab opened after an `Ended` keeps the folder in the set as `State::Ended`, which the `unwatched` condition missed; the Why names it); `Again` handled before the `opened` loop and dropped on `Stop`; the non-Windows `send_again` as `send_set` (`match self.0 {}`, not "returns false"); point 3 names every doc comment the change contradicts (`WatchEvent::Ended`, `State::Ended`, `ending`, `opens_to_close`, `wait_until_applied`); tests wait for the first watch before counting, a test for an `Ended` entry started anew added, the left-out-folder test named a guard; a hung network open replaced by an `Again` noted for known-limits; hand check point 4 uses a tab on the card's drive outside the source (F5 cannot come during `start_import`'s wait) |
| 2026-11-21 | Ready for review | Points 1–4 done: `Control::Again`, `FolderWatcher::watch_again`, `sys::Running::send_again` (both systems); `drain_control` keeps the `Again`s and handles them after the newest set and before the `opened` loop (`WatchThread::again`), dropped on `Stop`; `apply`'s open moved into `begin_open`; the doc comments of point 3; `reload_active_tab` calls `watch_again` before the listing. 5 tests (4 Windows, 1 on every system); `docs/test-inventory.md` counts and description. check.ps1 all OK (2191 passed, 76 ignored), no clippy warnings |
| 2026-11-21 | Done | Review (reviewer): points 1–4 as written; `again` takes the entry's state with `mem::replace`, ends a live watch with `begin_end` (its completion is matched in `ending` by key and `OVERLAPPED`, the new open has a new key), pushes a running open to `opens_to_close`, and starts the new open through `begin_open`, which `apply` now shares; `Again`s handled after the newest set and before the `opened` loop, dropped on `Stop`; F5 calls `watch_again` every time; the doc comments of point 3; the five tests as asked (the first three wait for the first watch). No independent review (one crate, four files). check.ps1 rerun by the reviewer: all OK, no clippy warnings. No CHANGELOG line: it fixes F3's unreleased Added line. Leftovers: a watch that goes dead without an error, and an `Again` replacing a hung network open, to `known-limits.md`, "The folder watcher". Awaiting the owner's hand check (points 1–4 above) |
