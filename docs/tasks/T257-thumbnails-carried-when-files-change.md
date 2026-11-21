# T257. ADR 0026 F4a: the contact sheet keeps its thumbnails when the active tab's files change (pw-loupe) — Open

- **Why:** ADR 0026 §3.5 (the owner, 2026-11-21): `Loupe::active_changed` (`main.rs`), used after
  a listing the folder watcher started (`follow_disk_changes`), after a placement of waiting files
  and after a new order (`follow_sort`, `active_reordered`), starts a new thumbnail generation with
  `set_generation` (which empties the worker's cache), makes a new `ThumbGrid` with every cell
  missing and clears `thumb_textures`. Every visible thumbnail is then made and drawn anew. During
  an import (F4, T260) the active tab gets new files several times a second, so the contact sheet
  would flicker all the time. This task keeps the thumbnails already made, matched by file: the
  worker changes generation without emptying its cache (T256), and the grid's cells and textures
  move with their files. Second task of the chain T256 → T257.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/thumbs.rs` (points 1 and 2, and their tests)
  - `crates/pw-loupe/src/main.rs` (points 3 and 4)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. `ThumbGrid` gains `pub fn carried(&self, generation: u64, old: &[PathBuf], new: &[PathBuf])
     -> (ThumbGrid, Vec<(usize, usize)>)`, with a doc comment: a grid of `generation` with one
     entry per file of `new`. Each file is matched by its first place in `old` and in `new`
     (compared exactly, as `PathBuf`s; a path that comes twice counts only at its first place in
     either list, a later copy in `new` is missing). A matched file whose entry in `self` is
     ready (`Entry::Ready`, with its cell size), has no preview (`Entry::NoPreview`) or failed
     (`Entry::Failed`) takes that entry, so these three are carried; a requested or missing entry
     becomes missing, so it is asked for again (a request made in the old generation is dropped,
     T256). A file not in `old` is missing. The second value lists `(old index, new index)` for
     **every** matched file, whatever its entry, in `new`'s order, so the caller moves the
     textures: a cell set back to missing by `forget` keeps its texture until the new one
     replaces it (T232, T254), and that texture moves with it too. When `self` does not have
     exactly `old.len()` entries, nothing is carried: every entry is missing and the list is
     empty.
  2. `thumbs.rs` gains `pub fn carried_map<T>(map: HashMap<usize, T>, moves: &[(usize, usize)])
     -> HashMap<usize, T>`, with a doc comment: a map holding, for each `(from, to)`, the value
     `map` had at `from`, now at `to`; values at indexes not named are dropped.
  3. `Loupe` gains `thumb_files: Vec<PathBuf>`, the active tab's files (in the tab's own form)
     the current `thumb_grid` was made for, with a doc comment. It is set to the tab's files (or
     emptied when there is none) wherever `thumb_grid` is given a new `ThumbGrid::new`
     (`load_tab`, both branches) and in `active_changed` (point 4), and is empty at start (the
     test constructor too).
  4. `active_changed` keeps the made thumbnails: the generation is raised as today, but given to
     the worker with `set_generation_keeping_cache` (T256); `thumb_grid` becomes
     `thumb_grid.carried(generation, &thumb_files, &tab.files)`; `thumb_textures` becomes
     `carried_map(thumb_textures, &moves)`; `thumb_files` becomes the tab's files. Each file of
     `thumb_files` that is not among the tab's new files (gone from the folder: deleted, moved
     away, removed by another program) is forgotten in the worker's cache with
     `thumb_worker.forget_path`, called with the path in the form `made_anew_if_stale` passes it,
     so another file that comes later under the same name is not shown with the old image (the
     chain review: before this task every generation change emptied the cache). The added
     files' stamps (T254) are **kept** as they are, carried or not: the stamp taken before a
     file's first request stays the right one to compare with, also when the cell is asked for
     again and answered from the kept cache. So the call `self.added_files.forget_requests()`
     after `active_changed` in `follow_disk_changes` is removed, and no stamp is dropped here;
     `load_tab` keeps its `forget_requests()` (the cache is emptied there). A "check when
     answered" mark on a dropped request is taken when the new request comes back, as today.
     The rest of `active_changed` (the preview and scroll handling) is unchanged. `load_tab` (a
     folder opened, another tab, Reload Folder) still uses `set_generation` and makes everything
     anew; do not change it except for point 3. Moves, undo and deletes reach `active_changed`
     through `active_reordered`, so they keep the thumbnails too; nothing else changes for them.
     Update the doc comments that describe the thumbnails being asked for again: those of
     `active_changed`, `active_reordered`, `follow_disk_changes`, the move, undo and delete code
     in `main.rs` where they say so, and `AddedFiles::forget_requests` in `thumbs.rs` (now used
     only after a new folder or tab).
- **Tests to add:**
  - in `thumbs.rs`, for `carried`: files reordered, the ready entries follow their files with
    their cell size and the moves list says where; a new file in the middle is missing and
    shifts the others; a file gone is not carried; a requested entry becomes missing but is in
    the moves list; a failed and a no-preview entry are carried; a path twice in `new` is
    carried only at its first place; a grid whose length differs from `old` carries nothing;
    the new grid answers only events of the new generation (`apply`); if the agent factors the
    "files gone" list of point 4 out as a pure function, its cases;
  - in `thumbs.rs`, for `carried_map`: values moved to their new indexes, values not named
    dropped, an empty move list gives an empty map;
  - `main.rs` (shell wiring of points 3 and 4) is checked by hand in T260 (the contact sheet
    does not flicker when files come, when a sort is chosen, after a move and an undo); a test
    of a pure helper may be added if the agent factors one out.
- **Not in this task:** `load_tab` and the import; the thumbnail worker (T256). Accepted (ADR
  0026 F-Q7): a file another program changes, not one it added, keeps its cached thumbnail
  across a carry until F5 or another tab (`load_tab` empties the cache); before this task any
  listing emptied it by the way. Files another program added are made anew by T254's
  `forget_path` as before. Known from T256's review (the owner, 2026-11-21, left as it is
  for now): the worker's in-flight check compares generation and index, not path, so when
  `active_changed` asks again for a file whose old-generation request is still being made, the
  file is made a second time in parallel (the first result is still cached); this costs double
  work for a slow RAW preview during an import. Do not change the worker for it; if T260's hand
  check shows it slowing the contact sheet, it becomes a task of its own.
- **Rules:** `docs/tasks/README.md` rules 1–12 (no texts); `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added when F4 was written (ADR 0026 §3.5); waits for T256 |
| 2026-11-21 | Blocked | Cold read: doable with the named files; point 4's `made_anew` for every file not carried would break T254 (an answer cached in the old generation, its event dropped, then a fresh stamp matching the half-written image), so the stamps are kept and only `forget_requests()` is removed; duplicates in `new` and the wording of carried entries made exact; the moves list now covers every matched file, so a forgotten cell's kept texture moves too; moves, undo and deletes reach `active_changed` (said, and in T260's hand check); the accepted F-Q7 case of a changed file's cached thumbnail named |
| 2026-11-21 | Blocked | Chain review: a file gone from the tab is forgotten in the kept cache (`forget_path` in `active_changed`, so a later file of the same name is not shown with the old image); the other findings (stamps, moves/undo/deletes, wording) were already fixed after the cold read |
| 2026-11-21 | Open | T256 done; its review's leftover (the in-flight double work) written into "Not in this task" |
