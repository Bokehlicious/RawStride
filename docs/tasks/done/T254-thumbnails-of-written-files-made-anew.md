# T254. ADR 0026 F3e: the thumbnail of a file still being written is made anew (pw-loupe) — Done

- **Why:** the owner's hand check of T252 (2026-11-21, point 1): an image copied into a folder on
  a USB stick appeared in the contact sheet with the thumbnail "Unreadable" until F5. Writing to
  the stick is slow, so the folder was listed (0.5–2 s after the file's name appeared) while the
  copy was still going on, and the thumbnail was made from the part written so far. When File
  Explorer has already given the file its full size, such a thumbnail may also come back whole
  but partly empty. The watcher asks Windows only for name changes
  (`FILE_NOTIFY_CHANGE_FILE_NAME`, `watcher.rs` `Watch::start_read`), so the end of the copy
  gives no event, and a made or failed cell is never asked for again (`thumbs.rs`
  `Entry::wants`) until the tab's thumbnails are all made anew (F5, or the next file that comes
  or goes, through `active_changed`). The owner decided in the backlog sort after F3 (2026-11-21,
  ADR 0026 §3.2, the addition to F-Q7): a change of size or write time of a listed image file also
  counts as a change; the active tab keeps the files another program added since the tab was
  read, with the size and write time each had when its thumbnail was asked for, and at each
  listing, and when such a thumbnail comes back, a file that differs has its thumbnail made anew.
  Nothing else is read again (a changed file's rating and metadata still wait for F5). Last task
  of the chain T253 → T254; T253 gave `ThumbWorker::forget_path`.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/watcher.rs` (point 1, and its test)
  - `crates/pw-loupe/src/thumbs.rs` (point 2, and its tests)
  - `crates/pw-loupe/src/lib.rs` (only the new type added to `pub use thumbs::{…}`)
  - `crates/pw-loupe/src/main.rs` (points 3–6)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **The watcher:** `Watch::start_read` asks `ReadDirectoryChangesW` for
     `FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_SIZE | FILE_NOTIFY_CHANGE_LAST_WRITE`
     (both new names are in `windows-sys`' `Win32_Storage_FileSystem`, which `pw-loupe` already
     enables; add them to the `use` list). `counts_as_change` is unchanged: a record counts only
     when its name `is_listed`, so a write to a sidecar (`.xmp`), a temporary file (`.pwtmp-`,
     `.pwbak-`, `.pwpart`) or the write time of a subfolder still gives nothing. The quiet moment
     is unchanged, so a long copy gives at most one `Changed` every `MOST` (2 s) while it goes on
     and one after its last write. The doc comments of `WatchEvent::Changed` ("Files came or
     went"), of `start_read`, and the module's own doc comment if it names what is watched, say
     that a change of size or write time of a listed file also counts (ADR 0026 §3.2, the
     addition to F-Q7).
  2. **The bookkeeping (`thumbs.rs`, no file access):** a new `pub struct AddedFiles` (plain
     data, `Default`), with doc comments: the files of the open tabs that another program added
     while RawStride had them listed, each with the stamp (`crate::histogram::FileStamp`, size and
     write time, taken just before its thumbnail was first asked for since it was added or last
     made anew; or no stamp yet) and a mark "check when answered". Methods:
     - `add(&mut self, paths: &[PathBuf])`: each path not yet kept is kept without a stamp and
       without the mark; one already kept is left as it is;
     - `contains(&self, path: &Path) -> bool`;
     - `note_requested(&mut self, path: &Path, stamp: Option<FileStamp>)`: for a kept path
       without a stamp, records `stamp` (also `None`: "asked for while unreadable"); a path that
       already has a stamp keeps it, since the drawing code asks again every frame while a cell
       is requested and the stamp must be the one taken before the first of those requests; a
       path not kept: nothing;
     - `is_stale(&self, path: &Path, now: Option<FileStamp>) -> bool`: true when the path is
       kept, a stamp was recorded, and `now` differs from it;
     - `mark_check(&mut self, path: &Path)` and `take_check(&mut self, path: &Path) -> bool`: set
       the mark of a kept path; return whether it was set and clear it;
     - `made_anew(&mut self, path: &Path)`: the path is kept, its stamp and mark dropped (the next
       request records a new stamp);
     - `forget_requests(&mut self)`: every kept path's stamp and mark dropped (after a new
       thumbnail generation, every cell is asked for again);
     - `retain(&mut self, keep: impl Fn(&Path) -> bool)`: drops the paths `keep` refuses;
     - `paths(&self) -> Vec<PathBuf>`: the kept paths, in any order.
  3. **Kept and given new requests (`main.rs`):** `Loupe` gains `added_files: AddedFiles`, one
     for all tabs (a file that came while its tab was active stays kept when the user switches
     tab and back, or presses F5, while its copy goes on). `load_tab` (a folder opened, another
     tab made active, F5's reload) calls `forget_requests()`, since it makes a new thumbnail
     generation; it does not empty it. In `follow_disk_changes`, when the listing changed
     something, after `active_changed(false)`: `forget_requests()`, then `add(&relisted.new)`
     (the paths as the tab holds them). In `load_activated_tab`, after `load_tab` succeeded and
     the listing was `Some(Ok(relisted))`: `add(&relisted.new)`. At each listing of
     `follow_disk_changes` (point 5), `retain` keeps only the paths that some open tab's `files`
     holds (`self.tabs.tabs()`), so files gone or in closed tabs are dropped.
  4. **The request (`main.rs`):** where the drawing code asks the worker for a thumbnail (the
     contact sheet, `to_request_indexes` over `visible_files`, and the filmstrip in the preview,
     `to_request_indexes` over `keep_files`), for a path `added_files.contains`, it first calls
     `added_files.note_requested(path, file_stamp(path))` (`pw_loupe::file_stamp`). Other paths
     are not read.
  5. **At each listing (`main.rs`):** in `follow_disk_changes`, when the listing changed nothing
     (`!relisted.changed()`, which returns early today), before the return: the `retain` of
     point 3; then for each kept path that is in the active tab's `files`, with its index there,
     when `added_files.is_stale(path, file_stamp(path))`:
     - the cell's state (`thumb_grid.state(index)`) is not `CellState::Requested`:
       `thumb_worker.forget_path(path)`, `thumb_grid.forget(index)`,
       `added_files.made_anew(path)`;
     - it is `Requested` (its read may have begun before the last write): `mark_check(path)`,
       left to point 6.

     Kept paths of other tabs are not read. When any cell was made anew, `ctx.request_repaint()`;
     the drawing code's usual request of the visible missing cells then asks for it. Nothing else
     on that path changes: no status text, no anchor, no metadata read. The texture of a made
     cell is kept until the new thumbnail replaces it, as after a turn (`thumb_grid.forget` in the
     orientation code), so the cell does not go empty.
  6. **When a thumbnail comes back (`main.rs`, `poll_thumbnails`):** after `thumb_grid.apply`
     accepted the event and its texture was handled as today, only when
     `added_files.contains(&event.path)` and `added_files.take_check(&event.path)` (no disk read
     otherwise): when `added_files.is_stale(&event.path, file_stamp(&event.path))`,
     `thumb_worker.forget_path`, `thumb_grid.forget(event.index)`,
     `added_files.made_anew(&event.path)` and `ctx.request_repaint()`. So a thumbnail whose read
     was under way at a listing that found its file changed is asked for again once, after it is
     answered, even when no later listing comes; a file still being written is read again at most
     once per listing (at most every 2 s, `MOST`), not after every answer. The doc comments of
     `follow_disk_changes` and `poll_thumbnails` say so (ADR 0026 §3.2, the addition to F-Q7,
     T254).
- **Tests to add:**
  - `watcher.rs` (Windows only, as the other watch tests, with `TempDir`, `wait_until_watching`
    and the event helpers): a sidecar `x.xmp` made before `wait_until_watching` and an image
    already in the folder (for example the `probe-0.jpg` that `wait_until_watching` made); the
    sidecar written to again (bytes appended, the file closed) gives no `Changed` within 1.5 s
    (`events_for`); the image written to the same way then gives `[changed(folder)]`
    (`events_until(5 s, |_| true)`; a later, delayed write-time note is not asserted against);
  - `thumbs.rs`, `AddedFiles`: `add` keeps a path without a stamp and leaves one already kept
    with its stamp; `note_requested` records the first stamp and keeps it on later calls, and
    does nothing for a path not kept; `is_stale` is false without a recorded stamp, false for the
    same stamp, true for a different size, a different write time, and between `None` and
    `Some`; `mark_check`/`take_check` set and clear the mark, only for a kept path; `made_anew`
    and `forget_requests` drop the stamp and the mark, so the next `note_requested` records anew;
    `retain` drops refused paths.
- **Not in this task:** reading a changed file's rating or metadata again (F-Q7 stands; F5); the
  large image in the preview of a file still being written is not opened again (F5 does it); the
  order of a Modified or Size sort does not follow a write until F5; a folder opened (or a tab
  made active for the first time) while a copy into it goes on does not keep the file, since it
  was not added by a listing (F5 after the copy); the stamp is taken on the UI thread just before
  the request, so a request merged by the worker into one already in flight (a zoom while a read
  is under way, `worker.rs` dedup) may keep a thumbnail read before the last write, and its
  result may be cached after `forget_path` (rare; F5); a USB stick's file system (FAT, exFAT)
  keeps write times in 2 s steps, so a change inside one step can be missed, though File
  Explorer setting the file's time at the end of the copy normally differs; every rating or turn
  RawStride writes into a JPEG (the safe write sets its write time) now gives a `Changed` and a
  folder listing on the UI thread, which before came only through the rename records of the
  same write, and a kept file gets its thumbnail made once more; a cell made anew while off
  screen keeps its texture until the next generation, as after a turn; a tab not shown makes all
  its thumbnails anew when it becomes active, as today; the listings a long copy gives every 2 s
  run on the UI thread, as every listing (`known-limits.md`, "The listing runs on the UI
  thread"). Each of these to `known-limits.md`, "Tabs following the disk", at Done if the
  reviewer finds it worth noting.
- **Rules:** `docs/tasks/README.md` rules 1–13 (no new texts); `AGENTS.md` §3.
- **Hand check (the owner):** with a release build:
  1. Open a folder on a USB stick and copy several large RAW files into it at once in File
     Explorer: the new images appear, and once the copy has ended their whole thumbnails are
     shown without F5 (they may show "Unreadable" or a partly grey thumbnail while the copy goes
     on). The copy is not visibly slower than with the folder not open in RawStride, and
     RawStride stays responsive.
  2. During such a copy, switch to another tab and back, and press F5 once: the thumbnails are
     still whole without another F5 once the copy has ended.
  3. Rate and turn images in a folder on the PC, a RAW (sidecar) and a JPEG: nothing changes
     that did not before (no text "by another program", the contact sheet does not jump).
  4. Copy a JPEG into a folder on the PC, then rate and turn it: its thumbnail follows the turn
     and does not flicker more than before.
  5. Copy an image into a folder on the PC: it appears with its thumbnail and the status line
     says "1 new image", as before.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added in the backlog sort after F3 (the owner's choice: ADR 0026 §3.2, addition to F-Q7; a first draft that only asked again for failed cells was replaced after its cold read: a thumbnail of a file given its full size at the start of a copy may come back made but partly empty, and a cell still being made at the last change was missed); waits for T253, its cold read and the chain review |
| 2026-11-21 | Blocked | Cold read and chain review (T253 → T254): point 6 only for a cell marked at a listing (`mark_check`/`take_check`), so a file still being written is read again at most once per listing, not after every answer, and only `contains` paths are read; `added_files` is one for all tabs and `load_tab` only drops the stamps (a file kept across F5 or a tab switch during its copy), `retain` to the open tabs' files, `clear` removed; the stamp named as taken before the request, the worker's merged in-flight request, FAT's 2 s write times, a listing at each JPEG write of RawStride's and a folder opened during a copy put under Not in; the watcher test no longer asserts nothing more within 2 s; hand check gains several files at once, the copy's speed, F5 and a tab switch during a copy, and a JPEG copied in then rated and turned. Waits for T253 |
| 2026-11-21 | Open | T253 done (`ThumbWorker::forget_path`) |
| 2026-11-21 | Ready for review | Points 1–6 done. Watcher: name, size and write-time notifications, `counts_as_change` unchanged, doc comments of the module, `WatchEvent::Changed` and `start_read`. `thumbs.rs`: `AddedFiles` (exported from `lib.rs`). `main.rs`: `added_files` field; `load_tab` calls `forget_requests`; `follow_disk_changes` adds `relisted.new` after `active_changed(false)` and `forget_requests`; when nothing changed, `retain_added_files` then `made_anew_if_stale` (new private helpers); `load_activated_tab` adds `relisted.new`; `note_requested` at both request sites; the marked check in `poll_thumbnails`. One addition beyond the text: `retain` also runs on a listing that changed something (point 3 says "at each listing"; point 5 names only the unchanged branch), so a gone file is dropped at once. 7 tests (6 `AddedFiles`, 1 watcher). check.ps1 all OK (2202 passed, 76 ignored), no clippy warnings. Not hand-checked |
| 2026-11-21 | Done | Reviewed: points 1–6 as written (the filmstrip's indexes are the tab's file indexes; the check in `poll_thumbnails` after `apply` accepted); the agent's added `retain` on a changed listing accepted (point 3 "at each listing"). `file_stamp` is called each frame while a kept cell is requested, as point 4 says (a few files during a copy). Reviewer's own check.ps1 all OK (2202 passed, 76 ignored), no clippy warnings. CHANGELOG: the F3 Added line gains the thumbnail of an image still being copied; known-limits "Tabs following the disk": the first item amended, four T254 items. Awaiting the owner's hand check (5 points) |
| 2026-11-21 | Done | Hand-checked by the owner (2026-11-21): points 1, 2, 3 and 5 passed (point 2: the thumbnails whole without another F5); point 4 failed: a copied-in JPEG's thumbnail jumps once more after a turn (RawStride's own write changes the file and the next listing finds it differs from its stamp) -> T255 (the owner's choice over a known limit) |
