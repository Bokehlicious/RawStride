# T172. ADR 0017 U9b, fourth part: the move rules, the undo list, the tab's list and the texts — Blocked: waits for T171

- **Chain:** T169 → T170 → T171 → T172 → T173 → T174. The owner's answers U9-Q5–U9-Q10 are in
  ADR 0017 §6.1.
- **Why:** ADR 0017 §2.6, step U9b. The shell's plain-Rust part, tested without a window (GR11):
  where an image goes, when its move may be sent, a batch's answers gathered into one undo step,
  the undo list, the tabs' lists after a move or an undo, and the texts. T173 connects the
  metadata worker, T174 the keys and menu.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/moves.rs` (new),
  `crates/pw-loupe/src/moves_text.rs` (new), `crates/pw-loupe/src/browse.rs`,
  `crates/pw-loupe/src/tabs.rs`, `crates/pw-loupe/src/ratings.rs`, `crates/pw-loupe/src/lib.rs`
  (the modules and exports), `docs/glossary.md`, `docs/test-inventory.md`, and this task's own
  file (status and log).
- **What:**
  1. **Where** (`moves.rs`): `pub enum MoveKind { Reject, Pick }`; `pub const REJECTED_FOLDER:
     &str = "_rejected"`, `pub const PICKED_FOLDER: &str = "_picked"`;
     `pub fn target_folder(image: &Path, kind: MoveKind) -> Option<PathBuf>`: the image's folder
     joined with the kind's name; `None` when the image has no folder or an empty one (the shell
     passes absolute paths), or when its folder's own name is `_rejected` or `_picked`, compared
     without regard to case (U9-Q9: there the keys only set the rating).
  2. **One batch** (`moves.rs`, U9-Q7): `pub struct MovedImage { pub image: PathBuf, pub moved:
     Vec<pw_metadata::Moved>, pub restore: Option<Rating> }` (`restore` is the rating the image
     had before an X, from `RatingStore::current_rating` below; `None` for P, which changes no
     rating). `Batch::new(images: Vec<(PathBuf, PathBuf, Option<Rating>)>)` (image, target,
     restore), with:
     - `to_send(&self, is_queued: impl Fn(&Path) -> bool) -> Vec<(PathBuf, PathBuf)>`: the images
       not yet sent whose path has no rating write still waiting in the shell's queue
       (`is_queued`), with their targets. It marks nothing; `sent(&mut self, image: &Path)` marks
       one once the worker took it (a full queue leaves it for the next call). An image with a
       queued write waits, so its move reaches the worker after that write (T170 point 4);
     - `answered(&mut self, image: &Path, result: Result<Vec<Moved>, String>)`;
     - `done(&self) -> bool` once every image has its answer, and `finish(self) -> BatchResult {
       moved: Vec<MovedImage>, failed: Vec<(PathBuf, String)> }`. An image that another image of
       the batch moved as its partner (its path is a `from` in another answer's moves; for
       example the JPG of a marked RAW+JPEG pair) is not a failure: its own answer is dropped.
  3. **Undo list** (`moves.rs`, U9-Q8): `pub struct UndoMoves` with at most `UNDO_STEPS = 20`
     steps, for the running session only: `push(step: Vec<MovedImage>)` (an empty step is not
     pushed; a 21st drops the oldest), `pop() -> Option<Vec<MovedImage>>`, `is_empty()`. One list
     for the whole program, not per tab.
  4. **A tab's list** (`browse.rs`): `Tab::replace_files(&mut self, files: Vec<PathBuf>,
     current: Option<&Path>, ratings_ready: bool, rating_of: &dyn Fn(&Path) -> Option<Rating>)
     -> bool`: the tab gets `files` (paths in the tab's own form; the caller converts); the
     marking and the anchor are kept for the files still there (compared as given, as `reorder`
     rebuilds the `Selection`); the current image is `current` when it is in `files`, otherwise
     the old current image when it is still there, otherwise the file that followed it (the last
     one if none followed). Then `refilter`: keeping `current` when it was given and found;
     otherwise keeping the current image only when it was shown before, as `reorder` ends.
     Returns whether the list changed.
  5. **Every tab of a folder** (`tabs.rs`): `same_folder` made `pub`, and `pub fn
     tabs_of_folder_mut(&mut self, folder: &Path) -> impl Iterator<Item = &mut Tab>` (the tabs
     whose folder is `folder`, by `same_folder`).
  6. **Ratings** (`ratings.rs`; the store's keys are absolute paths, so callers pass absolute
     ones): `pub fn is_queued(&self, path: &Path) -> bool` (a write for
     `path` is in `queued`, not yet sent); `pub fn current_rating(&self, path: &Path) -> Rating`
     (what `current` gives: the shown value, otherwise the confirmed one, otherwise the default);
     `pub fn restore(&mut self, path: &Path, rating: Rating) -> Result<(), RatingRefused>`: shows
     and queues exactly `rating`, as `set` does after its key is applied (the same checks of the
     path); used by the undo of an X.
  7. **Texts** (`moves_text.rs`, rule 13; whole sentences, the singular and plural in the same
     function):
     - `move_failed_text(name: &str, reason: &str)`: "{name} could not be moved: {reason}";
     - `moves_failed_text(count: usize, name: &str, reason: &str)`: "{count} images could not be
       moved. The last: {name}: {reason}" (for `count` ≥ 2);
     - `undo_failed_text(name: &str, reason: &str)`: "{name} could not be moved back: {reason}";
     - `undone_text(count: usize)`: "1 image moved back." / "{count} images moved back.";
     - `nothing_to_undo_text()`: "There is no move to undo.".

     Also export `worker_stopped_text` (`rating_text.rs`) from `lib.rs`, for T174.
  8. **Glossary** (`docs/glossary.md`, the 3-column form, "–" in the Swedish column as the newer
     rows): `_rejected`, `_picked` (folder names, not translated), "move back", and T174's menu
     items "Move Rejected and Picked Images to Subfolders", "Move to Picked" (P) and "Undo Move"
     (Ctrl+Z), named here so the texts and the glossary agree; each note names ADR 0017
     U9-Q5–U9-Q9 and T172.
- **Tests to add:** `moves.rs`: `target_folder` for a normal folder, inside `_rejected` and
  `_Picked` (`None`), without a folder; `to_send` leaves out an image with a queued write and gives
  it once the write left the queue, never after `sent`; `finish` sorts answers into moved and
  failed and drops the failed answer of a partner moved by another image; the undo list keeps 20
  and drops the oldest, ignores an empty step. `browse.rs`: `replace_files` with files removed
  (current removed → the following one; marks kept by path), with files added back (`current`
  given), with nothing visible under a filter. `tabs.rs`: `tabs_of_folder_mut` finds two tabs of
  one folder and not a third. `ratings.rs`: `is_queued` before and after `pump`; `current_rating`
  for an unread image is the default; `restore` shows and queues the exact rating.
  `moves_text.rs`: each text, with 1 and with more.
- **Not in this task:** the worker's events in the shell (T173); the keys, menu and wiring (T174).
  A failed move's answer is text only (T170): when `move_group` could not move every file back,
  the text names where those files are now (`MoveError`'s `Display`), but they are not in the
  batch's moves, so the undo list cannot move them back (accepted; `backlog.md`, "Leftovers from
  the review of T170").
- **Rules:** AGENTS.md §3; `docs/tasks/README.md` rule 13.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added for ADR 0017 U9b; rewritten after the cold read of the first T171–T172 (the batch does not mark images sent by itself, partners moved by another image are not failures, `restore` takes the rating from `current_rating`, `tabs_of_folder_mut`); waits for T171 |
| 2026-11-17 | Blocked | Cold read: `replace_files`'s `rating_of` type and its refilter rule made exact, paths in the tab's own form for it and absolute for the store; `worker_stopped_text` exported here for T174 |
| 2026-11-17 | Blocked | Review of T170: a failed move's files that could not be moved back are only named in the text, not undone ("Not in this task", `backlog.md`) |
