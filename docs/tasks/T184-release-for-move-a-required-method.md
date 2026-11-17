# T184. `ImageCheck::release_for_move` a required method (pw-metadata, pw-imagecheck) — Open

- **Why:** from the review of T177 and the first backlog sort (owner, 2026-11-17). The default
  `ImageCheck::release_for_move` calls `release`, so a new `ImageCheck`, or a wrapper of
  `RawImageCheck` that forwards only `release` (as `Timed` in
  `crates/pw-imagecheck/tests/real_files.rs` does), would note the histogram's write stamp before
  a move again (what T177 removed) without a warning. With no default, each implementation must
  decide, and the compiler says where.
- **Crates:** `pw-metadata`, `pw-imagecheck` (one test file). **Files:**
  `crates/pw-metadata/src/safe_write.rs`, `crates/pw-metadata/src/worker.rs` (its `mod tests`
  only), `crates/pw-metadata/tests/safe_write.rs`, `crates/pw-imagecheck/tests/real_files.rs`,
  and this task's own file (status and log).
- **What:**
  1. **The trait** (`safe_write.rs`): `fn release_for_move(&self, image: &Path);` without a
     body. Its doc comment keeps the first two sentences and replaces "The default calls
     [`ImageCheck::release`]." with exactly: "Every implementation gives it (T184): one that
     notes nothing in [`ImageCheck::release`] may call it or do nothing; a wrapper forwards it to
     the check it wraps." (A doc comment, not a text the user sees.)
  2. **`NoImageCheck`** (`safe_write.rs`): `fn release_for_move(&self, _image: &Path) {}`.
  3. **The test checks**, each given an empty `fn release_for_move(&self, _image: &Path) {}`
     (their `release` is empty too): `FailingCheck` in `worker.rs`'s `mod tests` (about line
     1373), `FailingCheck` and `LockingCheck` in `tests/safe_write.rs`. `RecordingCheck` in
     `worker.rs` has its own already and does not change.
  4. **`Timed`** (`pw-imagecheck/tests/real_files.rs`): `fn release_for_move(&self, image: &Path)
     { self.check.release_for_move(image); }`, forwarding as its `release` does.
  5. `RawImageCheck` (`pw-imagecheck/src/lib.rs`) has its own already and does not change. If
     any other `impl ImageCheck for` in the workspace fails to build (the reviewer found only the
     seven above), stop and say where instead of changing it.
- **Tests to add:** none; the behaviour does not change (every implementation does what the
  default did, or what it did already; `Timed` wraps `RawImageCheck::without_release()`, which
  has no move closure, so forwarding to `release_for_move` gives what `release` gave), and the
  compiler checks that each implementation has the method. `docs/test-inventory.md` does not change (no test is added or removed).
- **Not in this task** (`backlog.md`, "Leftovers from the review of T177", kept at the sort of
  2026-11-17): a write-only hook (`ImageCheck::before_write`) in place of `with_move_release`.
- **Rules:** `docs/tasks/README.md` rules 1–12; no new texts. This task changes the crate of the
  safe write and two crates, so an independent review of the diff follows the reviewer's own.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added from the first backlog sort (owner, 2026-11-17); waits for T180 (queue order) and for its cold read |
| 2026-11-17 | Blocked | Cold read: doable with the named files; the seven implementations are all there are (no blanket impls, nothing outside `crates/`, `pw-loupe` only builds `RawImageCheck`); only `safe_write.rs` says "the default calls". Changed: the doc comment's sentence given word for word; why `Timed` forwarding changes nothing written under **Tests to add**. Still waits for T180 (queue order) |
| 2026-11-17 | Open | T180 is Done: set Open (T180 changed nothing in `pw-metadata` or `pw-imagecheck`; T179's `edit.rs` changes do not touch `ImageCheck`) |
