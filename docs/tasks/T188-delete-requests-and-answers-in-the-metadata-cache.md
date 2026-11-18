# T188. ADR 0018 B3, second part: the delete requests and answers in the shell's metadata part (pw-loupe) — Open

- **Chain:** T185 (B1) → T186 (B2) → T187, T188 (B3) → T189 (B4) → T190 (B5). The owner's
  answers are B-Q1–B-Q15 in ADR 0018 §3; how the steps fit is ADR 0018 §2.10.
- **Why:** the shell's metadata part (`crates/pw-loupe/src/metadata.rs`, `MetadataCache`) owns
  the rating store and receives every event of the metadata worker in `poll`; `main.rs` only
  takes queues out of it. T186's two delete events must reach `main.rs` the same way as the
  moves' did (T173), and after a delete the cache forgets the deleted paths (ADR 0018 §2.7).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/metadata.rs`,
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **Submit:** `prepare_delete(&mut self, images: Vec<PathBuf>, image_extensions: Vec<String>,
     suffixes: Vec<String>) -> Result<RequestId, SubmitError>` and `delete_image(&mut self, image:
     PathBuf, image_extensions: Vec<String>, suffixes: Vec<String>) -> Result<RequestId,
     SubmitError>` in the `MetadataSubmit` trait, implemented for `MetadataWorker` by calling
     T186's methods, and for the tests' `FakeSubmit` (recording what was sent, as its other
     methods do: a `Vec` of what was sent per method, its own id counter, and an entry in
     `operations`, `"prepare_delete"` / `"delete_image"`, as `move_image` and `move_back` have;
     `FakeSubmit::new` sets them up).
  2. **Answers:** `#[derive(Clone, Debug, PartialEq, Eq)] pub(crate) enum DeleteAnswer { Prepared { id, images: Vec<PreparedDelete> },
     Deleted { id, image, result: Result<Vec<PathBuf>, DeleteError> } }` with the fields of
     T186's events (`DeleteError` and `DeleteRefusal` from the crate root, where T185 re-exports
     them; `PreparedDelete` and `DeleteGroup` from where T186 re-exports them). `apply` puts `WorkerEvent::DeletePrepared` and `WorkerEvent::Deleted` into a
     queue in place of T186's ignoring arm, and `pub(crate) fn take_deletes(&mut self) ->
     Vec<DeleteAnswer>` takes them, as `take_moves` does.
  3. **Forgetting deleted files:** `pub(crate) fn forget_deleted(&mut self, files: &[PathBuf])`
     (absolute paths, as the cache keys them): each is removed from the cache's list of images
     (`images`), the inspector's image is cleared when it is one of them (as `prune` does), and
     the rating store forgets them through `RatingStore::prune` with a `keep` that is false for
     them, and they are removed from `pending_read` when a folder read waits to be sent again
     (so the retry does not ask for a deleted file). The whole cache's `prune` may be reused
     after removing them from `images`. A write still queued or sent is kept by `prune`, as
     today; its answer for a missing file fails and is shown as any failed rating write is
     (expected; the delete waits for such writes, T187, so only a key pressed during the delete
     reaches this). Nothing else is forgotten: the metadata window and the editor are modal and
     handled by T189.
  4. The new items the program does not use yet (the two trait methods' declarations, the
     functions, and the enum or queue if clippy asks) carry `#[cfg_attr(not(test),
     allow(dead_code))]`, with the comment line "T189 (ADR 0018 B4) uses these; until then only
     the tests do." above each group, as `metadata.rs` does at about lines 510 and 526; T189
     removes them.
- **Tests to add** (in `metadata.rs`, with `FakeSubmit`): a `DeletePrepared` and a `Deleted`
  event reach `take_deletes` in order and are taken once; `FakeSubmit` records `prepare_delete`
  and `delete_image`; `forget_deleted` removes a loaded image (`has_image` false after); clears
  the inspector's image when it is one of them (`cache.inspector.image.is_none()`, checked in the
  same file's tests) and keeps it otherwise; drops the store entry of an image without a pending
  write (`cache.ratings.contains(path)` false); keeps a rating write that is still queued
  (`is_pending` true after); and takes a deleted path out of a waiting `pending_read`.
- **Not in this task:** sending the requests and acting on the answers (T189).
- **Rules:** `AGENTS.md` §3; no new texts.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows no change from this
  task outside the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked: waits for T187 | Added for ADR 0018 B3, written with B2–B5 for the chain review |
| 2026-11-17 | Blocked: waits for T187 | Cold read: doable in `metadata.rs` alone (the only two `MetadataSubmit` implementors are there; `main.rs` calls the move methods by name). Changed: the derives of `DeleteAnswer`, where the types come from, how `FakeSubmit` records, `pending_read` cleared too, what a write answer for a forgotten path does (no entry made again; the failure is shown, expected), the `dead_code` attribute on the trait declarations with its exact comment, and how each test checks (private fields from the same file's tests). Waits for T187 rather than T186 only so the B3 tasks are reviewed one at a time in `pw-loupe` |
| 2026-11-18 | Open | T187 is Done and committed (`9d2156d`): set Open. Checked: the `#[cfg_attr(not(test), allow(dead_code))]` examples are still at `metadata.rs` lines 510 and 526; T186's placeholder arm `WorkerEvent::DeletePrepared { .. } \| WorkerEvent::Deleted { .. } => {}` is at about line 1085 in `apply`; `take_moves`, `is_pending`, `has_image`, `prune` and `pending_read` are where the brief expects them |
