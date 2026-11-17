# T173. ADR 0017 U9b, fifth part: the move requests and answers in the shell's metadata part — Open

- **Chain:** T169 → T170 → T171 → T172 → T173 → T174. The owner's answers U9-Q5–U9-Q10 are in
  ADR 0017 §6.1.
- **Why:** the shell's metadata part (`crates/pw-loupe/src/metadata.rs`, `MetadataCache`) owns the
  rating store and receives every event of the metadata worker in `poll` (its private `apply`);
  `main.rs` only takes queues out of it (`take_written`, `take_editor_events`). T170's two move
  events must reach `main.rs` the same way, and `main.rs` needs the rating store's new functions
  (T172) through the cache. This task is that connection, testable with the file's `FakeSubmit`.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/metadata.rs`,
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **Submit:** `move_image(&mut self, image: PathBuf, target: PathBuf, image_extensions:
     Vec<String>, suffixes: Vec<String>) -> Result<RequestId, SubmitError>` and `move_back(&mut
     self, moved: Vec<Moved>) -> Result<RequestId, SubmitError>` in the `MetadataSubmit` trait, implemented for `MetadataWorker` by calling T170's methods,
     and for the tests' `FakeSubmit` (recording what was sent, as its other methods do).
  2. **Answers:** `pub(crate) enum MoveAnswer { Moved { id, image, result }, MovedBack { id,
     moved, result } }` with the fields of T170's events. `apply` puts `WorkerEvent::Moved` and
     `WorkerEvent::MovedBack` into a queue in place of T170's ignoring arm, and
     `pub(crate) fn take_moves(&mut self) -> Vec<MoveAnswer>` takes them, as `take_written` does.
  3. **The rating store** through the cache: `pub(crate) fn is_queued(&self, path: &Path) ->
     bool`, `pub(crate) fn current_rating(&self, path: &Path) -> Rating` and `pub(crate) fn
     restore_rating(&mut self, path: &Path, rating: Rating) -> Result<(), RatingRefused>`, each
     calling T172's function of the `RatingStore` field.
  4. **Images back after an undo:** `pub(crate) fn has_image(&self, path: &Path) -> bool`: the
     path (absolute) is in the cache's list of images. A moved image stays in that list until the
     tab is loaded again, so after an undo in the same tab its rating is still known; T174 loads
     the tab's ratings again only when a restored image is not in the list (decided after the
     cold read, 2026-11-17, in place of a second read slot beside the folder load's).
- **Tests to add** (in `metadata.rs`, with `FakeSubmit`): a `Moved` and a `MovedBack` event
  reach `take_moves` in order and are taken once; `is_queued` and `current_rating` follow the
  store (an X queued shows `is_queued` until `poll` sends it); `restore_rating` queues the rating;
  `has_image` is true for a loaded image and false for another. `FakeSubmit` records `move_image`
  and `move_back`.
- **Not in this task:** sending the moves and acting on the answers (T174).
- **Rules:** AGENTS.md §3; no new texts.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added for ADR 0017 U9b after the cold read of the first T171–T172 (events and the rating store live in `metadata.rs`); waits for T172 |
| 2026-11-17 | Blocked | Cold read: `add_images` would need a second read slot beside the folder load's (one `load_request`, `ratings_ready` derived from it) and could wipe a shown rating; replaced by `has_image` (point 4); the types of `move_image` and `move_back` given |
| 2026-11-17 | Open | T172 is done |
