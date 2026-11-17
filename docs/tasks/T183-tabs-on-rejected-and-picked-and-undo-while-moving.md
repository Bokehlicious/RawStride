# T183. ADR 0017 U9b: a tab open on `_rejected` or `_picked` follows a move and an undo; Ctrl+Z while a move runs says so (pw-loupe) — Blocked

- **Why:** from the review of T174 and the first backlog sort (owner, 2026-11-17; ADR 0017 §6.1,
  U9-Q11):
  1. After a batch of moves only the tabs of the images' own folders get a new list of files
     (`batch_done`), and after an undo only the tabs of the folders the images go back to
     (`undo_done`). A tab open on `_rejected` or `_picked` itself keeps listing files that have
     left, or misses the ones that came, until the folder is opened again.
  2. Ctrl+Z while a batch or an undo runs or waits does nothing and shows nothing (`undo_move`
     returns at once; the menu item is greyed then, so only the key reaches it).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/moves.rs`,
  `crates/pw-loupe/src/moves_text.rs`, `crates/pw-loupe/src/lib.rs` (only the two `pub use`
  additions of point 6), `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`, and this task's
  own file (status and log).
- **What:**
  1. **Where a step's files went** (`moves.rs`): `pub fn moved_to_folders(images: &[MovedImage])
     -> Vec<PathBuf>`: the folders (`Path::parent`, an empty parent left out) of every
     `Moved::to` of every image (the image and its companion files, as moved), each once,
     compared as given, in the order first met.
  2. **Listing a folder's tabs again** (`main.rs`): a private method `relist_tabs(&mut self, ctx:
     &egui::Context, folders: &[PathBuf]) -> Vec<String>` (the errors):
     - when no tab is open on any of `folders` (`self.tabs.tabs()` with `same_folder`), it does
       nothing and gives no errors;
     - otherwise every tab of each folder is listed again through `replace_tab_files` with the
       closure `undo_done` uses today (`list_folder` of the tab's folder, `resolve_paths`; a tab
       without a folder keeps its files), with no file made current (`None`);
     - then, in this order: `self.tabs.forget_sort()`; when the active tab was one of them,
       `load_active_tab()` (its new files' ratings are not in the metadata cache); then
       `self.follow_sort(ctx)` (it may call `active_reordered` again; that is allowed).
     Listing again, not dropping single files, also covers a folder spelt differently from the
     moved paths (case, a `\\?\` prefix) and files a failed move back left behind.
  3. **After a batch** (`main.rs`, `batch_done`, which gets `ctx` from `follow_moves`): after the
     tabs of the images' own folders (as today), `relist_tabs(ctx, &moved_to_folders(&result.moved))`;
     its errors join the batch's other errors (the Tasks list and the status line, as today). The
     status line is set after it, as today. When the batch gives no message (no failure, no
     error) and the status line shows `move_running_text()`, the status line is cleared
     (`self.message = None`), so point 5's text does not outlive the move.
  4. **After an undo** (`main.rs`, `undo_done`): besides the folders the images went back to (as
     today), `relist_tabs(ctx, &moved_to_folders(&restored))`, its errors joining `errors`. The
     existing listing of the images' own folders keeps its closure, but its sort and reload go
     the same order as point 2 (`forget_sort`, then `load_active_tab` or `active_reordered` as
     today, then `follow_sort`), since `follow_sort` before `load_active_tab` sorts the new files
     by ratings not read yet and marks the tab sorted. One `forget_sort` and one `follow_sort` for
     both listings is fine. The status line is still set last, as today.
  5. **Ctrl+Z while a move runs** (`moves_text.rs`, `main.rs`): `pub fn move_running_text() ->
     &'static str`: `"A move is still running. Try Undo Move again when it is done."`; in
     `undo_move`, when `self.moves.busy()`, the status line shows it (`self.message`) and nothing
     else happens: the key is not queued, the undo list is not touched (as intended, U9-Q11). The
     same text while an undo runs. The menu item stays greyed while busy (`Command::UndoMove`,
     unchanged).
  6. **The exports** (`lib.rs`): `moved_to_folders` and `move_running_text` added to the `pub use`
     lists of `moves` and `moves_text` that `main.rs` imports from.
- **Tests to add:**
  - `moves.rs`: `moved_to_folders_gives_each_folder_once`: two images of one folder, each with a
    companion file, moved to `…/_rejected`: one folder; an image of a second folder moved to its
    own `_rejected` adds that folder after the first; an empty list gives nothing.
  - `moves_text.rs`: `the_text_while_a_move_runs` asserts the exact text of point 5 (a new test;
    `the_move_texts_with_one_and_with_more` is not changed).
  - No test of `Loupe` (it cannot be built in a test); the owner's hand check covers points 2–5:
    a second tab open on `_rejected` while X moves images from the first, then Ctrl+Z with each
    tab active, and Ctrl+Z pressed while a batch of many marked images runs.
- **Not in this task** (`backlog.md`, "Leftovers from the review of T174", kept at the sort of
  2026-11-17): the undo's state in `moves.rs` (`UndoRun`), `forget_sort` clearing every tab's
  sort, `send_moves` building the extension list each frame, the `tab.folder == None` branch,
  `replace_tab_files` giving every tab of the active folder `ratings_ready`, and *Left by T178*.
  Also left (`backlog.md`, the same item, from this task's cold read): a tab on `_rejected`
  made active while a batch runs is loaded again when the batch is done (its position, filter
  and inspector reset); a tab open on a `_rejected` folder that was deleted gives a listing error
  after every batch; an undo that partly fails is not pushed back, so its failed images can no
  longer be undone. A tab open on a folder below `_rejected` or `_picked` is not followed.
- **Rules:** `docs/tasks/README.md` rules 1–13 (rule 13: the text is made in `moves_text.rs`;
  "Undo Move" is already in `docs/glossary.md`); `AGENTS.md` §3. Moves run while the user keeps
  working, so an independent review of the diff follows the reviewer's own.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added from the first backlog sort (owner, 2026-11-17: the two items of the review of T174 as one task, U9-Q11 in ADR 0017 §6.1); waits for T180 (`main.rs`) and for its cold read |
| 2026-11-17 | Blocked | Cold read: `lib.rs` must export the two new names (added to the files); no collision with T180. Changed: one `relist_tabs` for a batch and an undo, listing the `_rejected` or `_picked` tabs again rather than dropping single files (a folder spelt differently, files a failed move back left); "when a tab exists" checked with `same_folder`; the order `forget_sort`, `load_active_tab`, `follow_sort`, also in `undo_done` (it sorted the new files by ratings not read yet); the running-move text cleared after a batch without a message; Ctrl+Z not queued, as intended. Three timing gaps noted as left (`backlog.md`). Still waits for T180 |
