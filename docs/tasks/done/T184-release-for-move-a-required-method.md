# T184. `ImageCheck::release_for_move` a required method (pw-metadata, pw-imagecheck) — Done

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
     [`ImageCheck::release`]." with exactly: "Every implementation gives it (T184): one whose
     [`ImageCheck::release`] notes nothing may call it; one whose `release` does nothing may do
     nothing; a wrapper forwards it to the check it wraps." (A doc comment, not a text the user
     sees; changed at the review, 2026-11-17.)
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
| 2026-11-17 | Ready for review | Done as written: `release_for_move` has no body; `NoImageCheck`, the two `FailingCheck`s and `LockingCheck` give an empty one, `Timed` forwards it. No other `impl ImageCheck for` failed to build. `.\tools\check.ps1` all OK, no clippy warnings |
| 2026-11-17 | Open | Sent back after review (owner, 2026-11-17). Reviewer: points 2–5 as written, every one of the seven implementations gives the method, no other document says "the default calls"; `.\tools\check.ps1` all OK, 1818 passed, 69 ignored, no clippy warnings. Independent review (`/code-review`, 3 findings): the doc comment's "may call it or do nothing" lets an implementation whose `release` closes sessions without noting anything leave `release_for_move` empty, so the files stay open during the move; the fault was in this task's word-for-word text. **Left to do:** the doc comment's last sentence as point 1 now gives it ("one whose [`ImageCheck::release`] notes nothing may call it; one whose `release` does nothing may do nothing; a wrapper forwards it to the check it wraps."), nothing else; `.\tools\check.ps1` again. Accepted: no test of `Timed` forwarding (no tests asked for, no move runs through `Timed`); a cut-short `test-output.txt` was the reviewer's own run still going |
| 2026-11-17 | Ready for review | Left to do done: the doc comment's last sentence is now point 1's word for word; nothing else changed. `.\tools\check.ps1` all OK, no clippy warnings |
| 2026-11-17 | Done | Reviewed again and committed once the tests passed (owner's rule, 2026-11-17): the doc comment's last sentence is point 1's word for word, nothing else changed. `.\tools\check.ps1` all OK, 1818 passed, 69 ignored, 0 failed; no clippy warnings (grep of `test-output.txt`). No leftovers: the independent review's other findings were accepted (see the row above). No hand check: nothing the user sees changes. T186 no longer waits for T184 |
