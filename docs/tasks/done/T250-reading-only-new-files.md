# T250. ADR 0026 F3b: the ratings of only the new files, files waiting for their rating, Reload Folder reads the embedded turns again (pw-loupe) — Done

- **Why:** ADR 0026 step F3 (accepted by the owner 2026-11-18). When a tab's folder changes on
  disk, the active tab is listed again at once (§3.2) and its new files are shown as they come
  (§3.3). Three leftovers in `plans/adr-0018-0025-steps.md` ("Taken up when a step is written")
  must be solved first, in the metadata cache and the tab model, before the shell wires it (T251):
  - *A read of only the new files:* today every load and reload reads all of a tab's ratings
    again (`MetadataCache::begin_load`/`begin_reload`), which makes every image "reading" and,
    under a rating filter, empties the contact sheet until the read is back. Right for F5, but a
    listing the watcher starts may come every 2 s during a long copy: the sheet would blink each
    time. The new files need a read of their own that leaves the others ready.
  - *A new file waits for its rating:* until then it is not shown under a rating or label filter
    (§2 point 3) and not placed under a rating order (§2 point 4). Today both depend on one
    `ratings_ready: bool` for the whole tab, so new files would be filtered and placed as if they
    had no rating.
  - *Orientations are not read again:* after "Reload Folder" (F5), `OrientationStore` keeps the
    embedded values it read, so the next turn of a file turned by another program starts from
    the old value. ADR 0026 §3.1 says F5 shows a file changed by another program as it is now.
    A value RawStride wrote into a file (`confirmed_by_write`, T236) is **kept** (cold read: the
    thumbnail cache's key has no file time and the view core keeps the flip it read, so letting
    it go would show a stale thumbnail and flip; a task of its own, `known-limits.md`). A change
    seen by the watcher forgets nothing: only files that come and go are followed (F-Q7), and a
    new file has no value kept.
  Second task of the chain T249 → T250 → T251 (plan, ADR 0026 F3 row).
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/metadata.rs` (points 1, 2 and 6, and their tests)
  - `crates/pw-loupe/src/orientations.rs` (point 6 and its tests)
  - `crates/pw-loupe/src/tabs.rs` (points 3 and 5, point 4's whole order, and their tests)
  - `crates/pw-loupe/src/browse.rs` (point 4 and its tests)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **`MetadataCache::add_files(&mut self, files: Vec<PathBuf>, worker: &mut impl
     MetadataSubmit) -> Result<Vec<PathBuf>, String>`** (`metadata.rs`): the paths made absolute
     as `begin` does (same error); each path not in `images` yet is added, `Loading` when it
     `supports_rating`, else `Unsupported`; paths already there are left as they are and not
     sent. The added supported paths are sent in **one** `read_ratings` request of their own. Its
     id and its paths are kept apart from `load_request` (for example a map from id to paths), so
     **`ratings_ready()` does not change** and the files already read stay `Ready` with their
     ratings. A full queue keeps the paths and sends them from `poll`, as `retry_pending_read`
     does for the folder's read (kept apart from `pending_read`).
     - Its answer: the paths of that request still `Loading` get their ratings and XMP
       orientations as the folder read does (`ratings.read`, `orientations.read` with that id and
       those paths) and become `Ready`.
     - Its failure (an error answer, a submit error other than a full queue, or the worker
       stopped): those paths become `Error` with the message; `read_error` is not set (the
       folder's read did not fail) and `last_failure` is not touched; one failure goes to
       `poll`'s failures, made with the same text function as the folder read's notice
       (`editor_text(&EditorText::MetadataError { error })`). An `Error` file counts as read
       (point 2): it is shown and placed as unrated (accepted; the folder read's failure hides
       every image because the whole read failed, ADR 0014 Q7).
     - **The folder read leaves them alone:** the `RatingsRead` arm of `load_request`, and
       `fail_loading` (from `fail_read`), change only the paths that are not in an `add_files`
       request still sent or waiting; when the worker has stopped (`drain`'s `Disconnected`),
       those requests fail as above.
     - `begin` (a load or a reload) forgets every `add_files` request still sent or waiting, so a
       late answer changes nothing (it already replaces `images`). `prune` and `forget_deleted`
       drop removed paths from the waiting `add_files` paths as they do for `pending_read`.
     - Returns the absolute paths added.
  2. **`MetadataCache::rating_known(&self, path: &Path) -> bool`** (`path` absolute): true when
     the path's state is `Ready`, `Unsupported` or `Error`; false when it is `Loading` or the path
     is not in `images`. Doc comment: whether a filter and a rating order can use the file's
     rating (T251 asks it for a tab's unread files).
  3. **`Tab` gains `pub unread: Vec<PathBuf>`** (`tabs.rs`): files of `files`, in the tab's own
     form, whose rating is being read on its own (`add_files`) after a listing (ADR 0026 §2
     points 3 and 4). Empty by default; `Tab::open` empties it beside the fields it resets
     (`clear` already does). **Only the shell fills it** (T251, after a listing the watcher
     started, through `mark_unread`, point 4); `relist` does not, since Reload Folder and an
     activation read the whole tab anyway (`ratings_ready` false until that read is back), and
     T251 makes the relist after an undo read the whole tab when one of its files is not in the
     cache. The doc comment says who fills and empties it and what it does.
  4. **The tab model** (`browse.rs`, and `tabs.rs` for the whole order):
     - `Relisted` gains `pub new: Vec<PathBuf>`: the new files (those counted in `added`) in the
       tab's form, in `listed`'s order, so the shell knows which files to read; and
       `pub gone: Vec<PathBuf>`: the files removed (those counted in `removed`), in the tab's
       form and its order, so the shell can forget them in the cache (T251). Existing tests that
       build `Relisted` literally gain `new` and `gone` (only that; rule 5).
     - `pub fn mark_unread(&mut self, files: &[PathBuf])`: adds each of `files` that is in
       `files` and not yet in `unread`; then the shell refilters (doc comment says so).
     - `pub fn clear_read(&mut self, known: &dyn Fn(&Path) -> bool) -> bool`: removes from
       `unread` every file `known` says is read (the shell passes `MetadataCache::rating_known` on
       the resolved path) and returns whether one was removed. It does not refilter or place; the
       shell does that next (`refilter`, `follow_sort`), as the doc comment says.
     - Every function that changes `files` keeps `unread` a subset of `files` in its order (a file
       gone leaves it; `reorder` and a placement keep the same files).
     - `refilter`: under an active filter, a file in `unread` is not visible (as all files are not
       while the whole tab's ratings are read, ADR 0014 Q7), and its mark, if any, is kept as
       those are (amendment 3): `retain_visible` is given the visible indexes and those of the
       unread files. Without a filter it is shown at once, as today. A `keep` that names an unread
       file still keeps it visible (it is the current image).
     - The rating order: a file in `unread` is not placed: `place_known` treats its key as not
       known also when `ratings_ready` is true (it waits at the end in `unplaced`, where `relist`
       already puts every new file under a rating order); and `Tab::follow_sort`'s whole order
       (`tabs.rs`, the `plan` branch) under a rating key waits, as it does while `ratings_ready`
       is false, while `unread` is not empty. The other keys are unchanged (a capture time is read
       on its own, `capture_requests`).
  5. **`pub fn inside_lexically(path: &Path, root: &Path) -> bool`** (`tabs.rs`, beside
     `path_starts_with`): whether `path` is `root` or inside it, compared as `path_starts_with`
     does, without file access. T251 uses it to hold back the listing of a tab inside an
     import's destination.
  6. **Reload Folder reads the embedded turns again** (`orientations.rs`, `metadata.rs`):
     `OrientationStore` gains `pub fn forget_embedded(&mut self, paths: &[PathBuf])`: for each
     path's entry with no pending or waiting write (`is_pending`, `is_waiting`), the embedded
     value is forgotten; the confirmed value, `confirmed_by_write` and the shown value are not
     changed. `MetadataCache::begin_reload` calls it with the reloaded files before it sends the
     read; `begin_load` does not. The next turn of such a file asks for its embedded value again.
- **Tests to add:**
  - `metadata.rs` (with the existing fake worker): `add_files` sends one read of only the new
    supported paths, keeps the others `Ready` with their ratings and leaves `ratings_ready()`
    true; its answer makes them `Ready` with their ratings; an error answer makes them `Error` and
    gives one failure; a full queue sends them from `poll`; the folder read's answer, arriving
    while an `add_files` read is sent, leaves those paths `Loading`; `begin_load` after
    `add_files` makes a late answer change nothing; a path already in `images` is not sent again;
    `rating_known` for each state and for an unknown path; `begin_reload` forgets an embedded
    value and keeps a pending turn's value and a value written by RawStride (a JPEG or TIFF path,
    since only those take a written value), while `begin_load` forgets nothing.
  - `orientations.rs`: `forget_embedded` forgets the embedded value of a path without a write,
    keeps a path with a queued, sent or waiting turn as it was, changes no confirmed or shown
    value, and touches no other path; the next turn then asks for the embedded value again.
  - `tabs.rs` / `browse.rs`: `relist` gives the new files in `Relisted::new` and the removed
    ones in `Relisted::gone` (a pending file is not in `gone`), and leaves `unread` empty; `mark_unread` adds only files of the tab; `Tab::open` empties `unread`; a file gone
    leaves `unread`; under an active filter an unread file is hidden while the files already read
    stay shown (and keep their marks), and a marked unread file keeps its mark; without a filter
    it is shown; under a rating order an unread file waits in `unplaced` with `ratings_ready` true
    and is placed once `clear_read` removed it; `follow_sort` with a rating order not yet applied
    waits while `unread` is not empty and applies it after; `clear_read` returns whether it
    removed one; `inside_lexically` for the folder itself, a subfolder, a sibling with a longer
    name and a trailing separator, and (`#[cfg(windows)]`, as the existing tests) other case.
- **Not in this task:** calling any of this from the shell, the watcher's events, the status
  texts (T251); the thumbnails made anew when the active tab's list changes (to be watched in
  T251's hand check); reading again a file that only changed (F-Q7, F5 does that); a value
  RawStride wrote into a file and then changed by another program (kept, see Why).
- **Rules:** `docs/tasks/README.md` rules 1–12 (the failure text comes from the existing
  `editor_text`; no new user-visible text); `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added (ADR 0026 F3, second of T249 → T250 → T251); waits for T249 |
| 2026-11-21 | Blocked | Chain review (T249 → T251): `Relisted` gains `gone` (T251 forgets those files in the cache, so a file brought back is read anew); the Why no longer says the relist after an undo reads the whole tab (T251 makes it) |
| 2026-11-21 | Blocked | Cold read: `relist` no longer fills `unread` (only the shell after a watcher listing, `mark_unread`), so moves, undo, F5 and activation are unchanged until T251; the folder read's answer and `fail_loading` leave `add_files` paths alone, a stopped worker fails them; the failure text through `EditorText::MetadataError`, `last_failure` untouched, an `Error` file counts as read (unrated); the whole order under a rating key waits while `unread` is not empty (`Tab::follow_sort`); marks of unread files kept through `retain_visible`; `prune`/`forget_deleted` cover the waiting paths; point 6 narrowed to the embedded values (`forget_embedded`): a value RawStride wrote stays, since the thumbnail cache and the view core would show it stale; `Relisted` literals in existing tests gain `new`; `inside_lexically`'s case test Windows only |
| 2026-11-21 | Open | T249 Done |
| 2026-11-21 | Ready for review | Done by the agent: `metadata.rs` `add_files` (own reads in `added_reads`, waiting paths in `pending_added`, a failure on sending kept for the next `poll`; the folder read's answer and `fail_loading` skip their paths; `begin` forgets them; `prune`, and so `forget_deleted`, drops waiting paths; a stopped worker fails each sent read and the waiting paths, one failure each when a file became `Error`), `rating_known`, `begin_reload` calls `OrientationStore::forget_embedded`; `begin`'s path resolution moved into `absolute_files` (shared with `add_files`); `orientations.rs` `forget_embedded`; `tabs.rs` `Tab::unread` (emptied by `open`), `follow_sort`'s whole order waits under a rating key while `unread` is not empty, `inside_lexically`; `browse.rs` `Relisted::new`/`gone`, `mark_unread`, `clear_read`, `replace_files` keeps `unread` a subset of `files`, `refilter` hides unread files under an active filter and keeps their marks, `place_known` leaves them waiting under a rating key. `add_files`, `rating_known` and `inside_lexically` carry `#[cfg_attr(not(test), allow(dead_code))]` until T251 calls them; `inside_lexically` is not yet exported from `lib.rs` (not in this task's files; T251 does that). 20 tests (metadata 9, orientations 1, browse 6, tabs 4); the three `Relisted` literals in existing tests gained `new` and `gone` only. check.ps1 all OK (2181 passed, 76 ignored), no clippy warnings |
| 2026-11-21 | Done | Reviewed: points 1–6 and every test asked for are there; only the named files changed; the folder read's answer, `fail_loading` and a stopped worker leave or fail the `add_files` paths as asked; `unread` kept a subset of `files` through `replace_files` (also `relist`'s path). check.ps1 rerun by the reviewer: all OK (2181 passed, 76 ignored), no clippy warnings. Leftovers: `inside_lexically` not exported and the three `allow(dead_code)` lines added to T251's Files (lib.rs export; the allow lines removed in metadata.rs and tabs.rs); a turn value RawStride wrote kept at Reload Folder in `known-limits.md`, "Reload Folder (ADR 0026 F1)". No hand check (T251's covers F3). T251 set to Open |
