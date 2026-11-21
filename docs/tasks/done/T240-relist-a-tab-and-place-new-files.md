# T240. ADR 0026 F1a: list a tab again, place new files in the sort, keep the contact sheet still (pw-loupe, logic only) — Done

- **Why:** ADR 0026 (accepted by the owner 2026-11-18) step F1 is "Reload Folder" (F5): the
  active tab's folder is listed again and §2's rules are applied, so files added or removed by
  another program are shown without opening the folder again. §2 holds for every change of a
  tab's files. Today a tab's list changes only through `Tab::replace_files` (`browse.rs`), after
  RawStride's own moves, undo and delete, and `relist_tabs` (`main.rs`) then applies the whole
  sort order again (`Tabs::forget_sort`), which can move images already shown (U14-Q5 says a new
  read never moves an image). This task makes the plain-Rust part: RawStride's own temporary
  files never listed (§2 point 6), a tab given a new listing with the new files placed where the
  order puts them (§2 point 4, F-Q6), the marks kept while the ratings are read (§2 point 2, ADR
  0014 amendment 3, the owner 2026-11-21), and the arithmetic that keeps the image at the top of
  the contact sheet where it is (§2 point 5). Point 4 is the one change users see before T242:
  under an active filter the marks now survive a switch to another tab and back (and a folder
  read that fails, which leaves them hidden until a read succeeds); nothing acts on them while
  they are hidden, and the status line's "{n} marked" is shown only with a current image shown
  (`main.rs`, the status line). It is the first task of the chain T240 → T241 →
  T242 (plan, ADR 0026 F1 row); T242 wires it into the shell.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/lib.rs` (point 1 and its test)
  - `crates/pw-loupe/src/sort.rs` (point 2 and its tests)
  - `crates/pw-loupe/src/tabs.rs` (point 3 and its tests)
  - `crates/pw-loupe/src/browse.rs` (points 4–6 and their tests; one existing test changed, point 4)
  - `crates/pw-loupe/src/grid.rs` (point 7 and its tests)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **`is_listed`** (`lib.rs`): also false when the file name contains `.pwtmp-` or `.pwbak-`
     (compared ignoring case): the names of the safe write's copy and backup,
     `<name>.pwtmp-<hex>.<ext>` and `<name>.pwbak-<hex>.<ext>` (`pw-metadata` `fileops.rs`,
     `temp_name`), which end in an image extension and were listed when a folder was read during
     a write (ADR 0026 §2 point 6). `.pwpart` is already left out by its extension. Its doc
     comment says so. `list_folder` uses `is_listed` and needs no change.
  2. **`sort.rs`**, two public functions beside `plan`:
     - `insert_position(files: &[PathBuf], new: &Path, order: SortOrder, capture: &CaptureTimes,
       rating_of: &dyn Fn(&Path) -> Option<Rating>, modified_of: &dyn Fn(&Path) ->
       Option<SystemTime>) -> usize`: the index in `files` before the first file that `order`
       puts after `new`; `files.len()` when there is none. The comparison is exactly the one
       `sorted_order` uses: the key's value in the order's direction, no value last in both
       directions, then the file name ignoring case, then the path as it is (the old-position
       tie-break of `sorted_order` does not apply). `files` need not be in the order (a rating or
       a modification time may have changed since it was applied): it is scanned from the start,
       not searched by halves. Make the value of a key and the comparison one private function
       each, used by both `plan`/`sorted_order` and `insert_position`, so they cannot drift
       apart; `plan`'s behaviour does not change (its tests stay as they are). With `Modified`
       a key's value is a disk read (`modified_of`), so `relist` and `place_waiting` (points 5
       and 6) take the values of the placed files once per call, not once per new file: give
       `sort.rs` a private or `pub(crate)` way to compute the values once and place several new
       files with them (for example a function that takes the values), with `insert_position`
       built on it.
     - `key_known(path: &Path, order: SortOrder, capture: &CaptureTimes, ratings_ready: bool) ->
       bool`: whether `insert_position` can place `path` now: `Name`, `Type` and `Modified`
       always; `Capture` when `capture.is_read(path)`; `Rating` when `ratings_ready`.
  3. **`Tab`** (`tabs.rs`): a new public field `unplaced: Vec<PathBuf>`: files listed since the
     order was applied that wait for their key's value (a capture time or a rating) and stand at
     the end of `files`, in the order they came. Its doc comment says that they are always the
     last files of `files` (points 5 and 6 keep it so). `replace_files` and `reorder` (called by
     the shell after moves, undo and delete) do not know `unplaced`, so the invariant is checked,
     not trusted: a private `Tab` helper, called first by `relist` and `place_waiting`, drops the
     files of `unplaced` no longer in `files` and checks that the rest are exactly the last
     files of `files`, in `unplaced`'s order; when they are not, `unplaced` is emptied and
     `sorted` set to `None`, so the next `follow_sort` applies the whole order (as after an
     open). `Tab::open` empties it. `Tab::follow_sort`:
     - when `self.sorted == Some(order)`: calls `place_waiting` (point 6) with its arguments and
       returns a new variant `SortFollow::Placed { moved: bool }` when it placed a file (`moved`
       when a file changed place), otherwise `SortFollow::Sorted` as today. `Placed` is its own
       variant so the shell can tell a placement (the contact sheet keeps its top image, T242)
       from a new order (it scrolls to the current image); `main.rs`'s `sort_tabs` matches only
       `Reordered { moved: true }` and so ignores `Placed` until T242, which is right since
       nothing is waiting before T242 calls `relist`;
     - when the order is applied (`SortPlan::Order`): `unplaced` is emptied after `reorder`
       (the whole order now holds for every file).
     `Tabs::forget_sort` also empties every tab's `unplaced` (the whole order is applied
     next). The other branches and `Tabs::follow_sort` and `record_capture` are unchanged.
  4. **`Tab::refilter`** (`browse.rs`): while a filter is active and the ratings are not read
     (`!ratings_ready`, the branch that makes `visible` empty), `selection.retain_visible` is not
     called: the images are hidden only because their values are unknown, so the marks stay (ADR
     0014 §2.8 as changed by amendment 3, §16, the owner 2026-11-21). In every other case it
     is called as today. This holds for every tab, so inactive tabs (refiltered with
     `ratings_ready` false) also keep their marks; that is intended. The doc comments of
     `refilter` and `rated` (whose "marked images that left are unmarked" is no longer true
     while the ratings are unread) say so. The existing test
     `an_active_filter_shows_nothing_until_the_ratings_are_read` sets `t.current = 4` and calls
     `t.selection.ctrl_click(4, 5)`, which marks both 4 and 5 (`selection.rs`, `ctrl_click`
     marks the current image first), and asserts
     `assert_eq!(t.selection.count(), 0, "hidden images are unmarked")`; that one assertion is
     changed, with the owner's approval (ADR 0014 §16), to `assert_eq!(t.selection.count(), 2,
     "marks are kept while the ratings are read")`, and after the following
     `t.refilter(true, …)` (visible `[2, 4]`) three assertions are added: `is_marked(4)`,
     `!is_marked(5)` and `count() == 1`; nothing else in that test changes. Before you change it,
     list every caller of `Selection::count`, `Selection::is_marked` and `Selection::marked_in`
     in `pw-loupe` in your report, with whether it counts only marks in `visible`: a mark the
     filter hides while the ratings are read must not become a target of a rating key, a move, a
     delete or "Open in". If one would act on such a mark, stop and ask. (The reviewer's cold
     read found none: every action needs `tab.position()` or `visible`.)
  5. **`Tab::relist`** (`browse.rs`, beside `replace_files`):
     `relist(&mut self, listed: Vec<PathBuf>, current: Option<&Path>, pending: &dyn Fn(&Path)
     -> bool, order: SortOrder, ratings_ready: bool, rating_of: &dyn Fn(&Path) ->
     Option<Rating>, modified_of: &dyn Fn(&Path) -> Option<SystemTime>) -> Relisted`.
     `pending` says whether RawStride has a write queued or in flight for a file of the tab: such
     a file counts as listed even when `listed` lacks it, since on a drive without
     `ReplaceFileW` the safe write renames the original away for a moment (`pw-metadata`
     `fileops.rs`, `replace_by_rename`), and a listing in that moment must not take its mark or
     call it removed (chain review). `current` is passed on to `replace_files`
     (the image an undo brings back, T242; `None` for a reload). `listed` is the folder's new listing as `list_folder` gives it (file-name
     order), in the tab's own form (the caller converts; a rename that changes only the case
     counts as one file gone and one new); paths are compared as given, as `replace_files` does.
     A path that comes twice in `listed` is taken once (the first).
     - First the `unplaced` check of point 3.
     - The files of `self.files` that are in `listed` (or `pending`) keep their order; the
       others are gone.
       The files of `listed` that are not in `self.files` are new, taken in `listed`'s order.
     - Nothing new and nothing gone: nothing in the tab changes, and an empty `Relisted` is
       returned.
     - When `self.sorted == Some(order)`: each new file whose key is known (`key_known`; for
       `SortKey::Rating` a new file's key is **never** known here, since its rating has not been
       read yet) is inserted at `insert_position` among the files before the `unplaced` tail
       (that part of the list, not the tail, is passed to `insert_position`; with `get`, not
       slicing that can panic); each other new file is put at the end and added to `unplaced`.
     - Otherwise (no order applied yet, or another one chosen) the new files are put at the end
       in `listed`'s order, `unplaced` is emptied and `sorted` becomes `None`, so the next
       `follow_sort` applies the whole order, as after an open (also when the user then chooses
       the order the tab was in before).
     - `unplaced` keeps only the files still there.
     - The new list then goes through `self.replace_files(new_list, current, ratings_ready,
       rating_of)`, so the marks of the files still there, the anchor, the filter and the
       current image (or the file that followed it) follow `replace_files`' rules.
     - `Relisted` (a new public struct in `browse.rs`, `Debug`, `Default`, `PartialEq`) holds
       `added: usize`, `removed: usize` and `current_removed: Option<PathBuf>` (the current file
       when it was among the gone ones, for ADR 0026 F-Q5's text), and a method `changed()`
       (added or removed is not 0).
  6. **`Tab::place_waiting`** (`browse.rs`): `place_waiting(&mut self, order: SortOrder,
     ratings_ready: bool, rating_of: …, modified_of: …) -> bool`: when `self.sorted !=
     Some(order)` it changes nothing and returns false. Otherwise, after the `unplaced` check of
     point 3, each file of `unplaced` whose
     key is now known (`key_known`), in `unplaced`'s order, is taken out of the tail and inserted
     at `insert_position` among the placed files; the files still waiting stay at the end. The
     new order goes through `self.reorder(…)` (the current image, the marks, the anchor and the
     filter kept as `reorder` keeps them), and the placed files leave `unplaced`. Returns whether
     a file changed place (`reorder`'s answer; a file whose place is the end changes nothing but
     still leaves `unplaced`). With nothing known, nothing changes and it returns false.
  7. **`grid.rs`**, two public functions for §2 point 5:
     - `anchored_offset(offset: f32, stride: f32, columns: usize, old_top: usize, new_top: usize)
       -> f32`: the scroll offset that keeps the image that was at position `old_top` in the
       visible list, now at `new_top`, at the same height on the screen: `offset` plus the
       difference of their rows (`row`) times `stride`, computed in `f64`, never below 0. A
       non-finite `offset` gives 0; a non-finite or non-positive `stride` gives `offset`
       unchanged.
     - `top_position(old_from_top: &[PathBuf], new_visible: &[PathBuf]) -> Option<usize>`: the
       position in `new_visible` of the first path of `old_from_top` that is in it (the image at
       the top, or the first one after it that is still shown); `None` when none is. One lookup
       per path (a `HashMap` or `HashSet` of `new_visible`), not a search of the list for each.
- **Tests to add** (beside the existing tests of each module, with their helpers):
  - `lib.rs`: `is_listed` is false for `bild.pwtmp-1a2b.jpg`, `bild.PWBAK-0123.ARW` and
    `bild.pwpart`, true for `bild.jpg` and for `pwtmp.jpg` (no dot before it).
  - `sort.rs`: `insert_position` by `Name` (between two names, first, last), `Name` descending,
    `Capture` with a file that has no time (last in both directions), `Modified` with a list not
    in the order (scanned from the start: the first file after `new` decides), `Rating`, and an
    empty list (0); a new file whose value ties a placed file's (placed after it by name, then
    path); the shared comparison: for each key, a small list put in `plan`'s order, and for
    every `i`, `insert_position` of file `i` into the list without it gives `i`; `key_known` for
    each key.
  - `tabs.rs`: `follow_sort` with `sorted == Some(order)` places a waiting file once its capture
    time is recorded and answers `Placed { moved: true }`, and answers `Sorted` while it still
    waits; applying a new order empties `unplaced`; `open` empties it.
  - `browse.rs`:
    - `relist` with a file added by `Name`: placed between its neighbours, the current image and
      the marks unchanged (by file), `Relisted { added: 1, removed: 0, current_removed: None }`;
    - `relist` with the current file and a marked file gone: the following file current, the
      other marks kept, `current_removed` names the gone current file;
    - `relist` under `Capture` with the time not read: the new file at the end and in
      `unplaced`; after `record_capture` and `place_waiting` it is in its place and leaves
      `unplaced`; a second new file that comes while the first waits is placed before the tail
      when its key is known;
    - `relist` under `Rating`: the new file waits even with `ratings_ready` true, and is placed
      by `place_waiting` with its rating;
    - `relist` with no order applied (`sorted == None`): new files at the end in name order;
    - `relist` with the same listing: nothing changes and `changed()` is false;
    - `relist` with `current` naming a new file (an undo): that file becomes current;
    - `relist` where a file missing from `listed` is `pending`: it stays, with its mark, and is
      not counted as removed;
    - `relist` in the "otherwise" branch: `sorted` becomes `None` and `unplaced` is empty;
    - `forget_sort` empties `unplaced`;
    - `relist` with every file gone: no files, nothing shown, `current_removed` set; and
      `relist` of an empty tab (no files) with files listed;
    - a waiting file that is gone in a second `relist` leaves `unplaced`;
    - after `replace_files` with a list that breaks the tail (a waiting file moved before a
      placed one), `place_waiting` empties `unplaced` and `sorted` becomes `None`;
    - `place_waiting` with `sorted != Some(order)` changes nothing and returns false;
    - `relist` under an active label filter with `ratings_ready` false: nothing shown, the
      marks kept; after `refilter(true, …)` the marked files that match stay marked and a
      marked file that does not match is unmarked;
    - the changed assertion of `an_active_filter_shows_nothing_until_the_ratings_are_read`
      (point 4).
  - `grid.rs`: `anchored_offset` with the top image moved down one row and up two rows, with a
    partial row kept (the part of `offset` below a row boundary unchanged), never below 0, and
    for a non-finite offset, a zero stride and `columns == 0` (taken as 1, as `row` does);
    `top_position` when the top image is still there, when it is gone and the next one is, when
    none is, and with an empty `old_from_top`.
- **Not in this task:** the command, the key and the texts (T241); wiring into the shell, the new
  reads after a reload, the capture-time requests for `unplaced` files, the scroll and the use
  of `relist` in `relist_tabs` (T242); following the disk by itself (ADR 0026 F2, F3); the
  import (F4). `replace_files`, `reorder`, `plan` and `list_folder` keep their behaviour; only
  `refilter`'s marks change (point 4).
- **Rules:** `AGENTS.md` §3 (no `unwrap`, `expect`, `panic!` or unchecked indexing outside
  tests), `docs/tasks/README.md` rules 1–12. No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added. The owner decided 2026-11-21: marks kept while the ratings are read (ADR 0014 amendment 3, §16; ADR 0026 §2 point 2), and `relist_tabs` uses the placement (ADR 0026 §2 point 4, T242) |
| 2026-11-21 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README and this file): doable with the named files; no caller of the marks acts on a mark hidden while the ratings are read (every action needs `position()` or `visible`). Changed after it: the test's marks are 4 and 5 (`ctrl_click` marks the current image first), so the changed assertion is 2, then 4 marked and 5 not; the `unplaced` tail checked, not trusted (`replace_files`/`reorder` do not know it): emptied and `sorted = None` when broken; `unplaced` emptied when the order is another; `place_waiting` does nothing unless `sorted == Some(order)`; `Modified` values taken once per call; duplicates in `listed` taken once; `rated`'s doc comment; inactive tabs keep marks too; the shared-comparison test spelled out; tests for every file gone, an empty tab, a waiting file gone, a broken tail, ties, `columns == 0`, an empty anchor. The jump of `active_reordered` after a placement is T242 point 4. Ctrl+A with nothing visible clears the kept marks: `backlog.md`, "Leftovers of writing ADR 0026 F1" |
| 2026-11-21 | Blocked | Chain review of T240–T242 (fresh agent, `AGENTS.md`, the README and the three task files): changed here: `SortFollow::Placed { moved }` so the shell can tell a placement from a new order; `relist` takes `current` (an undo's image) and `pending` (a file RawStride is writing counts as listed: on a drive without `ReplaceFileW` the original is renamed away for a moment); the "otherwise" branch also sets `sorted = None`; `forget_sort` empties `unplaced`; the users' visible change of point 4 said in "Why". `list_folder` skipping entries that give an error and a read of only the new files' metadata for F3: `backlog.md`, "Leftovers of writing ADR 0026 F1" |
| 2026-11-21 | Open | Cold read and chain review done |
| 2026-11-21 | Ready for review | Points 1–7 done. lib.rs: `is_listed` leaves out `.pwtmp-`/`.pwbak-` (ignoring case); re-exports `Relisted`, `insert_position`, `key_known`, `anchored_offset`, `top_position`. sort.rs: private `Keyed` (the values taken once), `compare` and `place`, used by `plan`/`sorted_order` and `insert_position`; `key_known`. tabs.rs: `unplaced`; `SortFollow::Placed { moved }`; `follow_sort`, `open`, `forget_sort`. browse.rs: `refilter` keeps the marks while an active filter waits for the ratings; `Relisted`, `relist`, `place_waiting` (and `pub(crate) place_known`, which tells `follow_sort` whether a file was placed), private `check_unplaced`. grid.rs: `anchored_offset`, `top_position`. Callers of `count`/`is_marked`/`marked_in` (point 4): `main.rs` `metadata_subject` and `marked_subjects` (`marked_in(visible)`), the cell drawing and the filmstrip (`is_marked` of visible cells only), the status line (`count()` only with a current image shown), `key_targets` (`targets`, only with `position()`); `browse.rs` `reorder`/`replace_files` (`marked_in(all)` to carry marks over) and `preview_list` (`marked_in(visible)`); none acts on a hidden mark. Notes: when the `unplaced` check breaks the tail inside `follow_sort`, it goes on to apply the whole order in the same call instead of answering `Sorted`; `relist` of an empty tab makes the last file current (`replace_files`' rule). 27 tests added, the one assertion of point 4 changed as written. check.ps1 all OK, 2080 passed, 76 ignored, no clippy warnings |
| 2026-11-21 | Done | Reviewed (reviewer, all the diff against points 1–7): as written. Accepted deviation: a broken `unplaced` tail found inside `follow_sort` applies the whole order in the same call. `relist`'s early return (nothing new, nothing gone) ignores `current`: noted in T242 point 6. The reviewer's clean run of check.ps1: all OK, 2080 passed, 76 ignored, no clippy warnings. Moved to `done/`; no hand check of its own (F1's is in T242). No CHANGELOG line (T242's at Done) |
