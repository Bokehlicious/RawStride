# T239. A separate Tasks line "Writing rotations: {n}" (pw-loupe) — Done

- **Why:** since T237 the Tasks area's line "Writing ratings: {n}" also counts the rotation writes,
  because `MetadataCache::pending_writes` (`crates/pw-loupe/src/metadata.rs`, about line 694)
  adds `self.ratings.pending()` and `self.orientations.pending()`. The owner decided after the hand
  check of ADR 0024 (2026-11-20): rotation writes get their own line, "Writing rotations: {n}",
  beside the ratings' line (backlog, "Leftovers from the review of T237"). The reviewer records
  the decision in ADR 0017 (U5-Q4, U5-Q7) before this task is Done.
- **Crates:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/metadata.rs` (point 1 and its tests)
  - `crates/pw-loupe/src/tasks.rs` (point 2 and its test)
  - `crates/pw-loupe/src/tasks_text.rs` (point 3 and its test)
  - `crates/pw-loupe/src/main.rs` (point 4 only)
  - `docs/glossary.md` (point 5)
  - `docs/test-inventory.md`
  - this task's own file (status and log)
- **What:**
  1. **The counts** (`metadata.rs`): beside `pending_writes`, two methods
     `pending_rating_writes(&self) -> usize` (`self.ratings.pending()`) and
     `pending_rotation_writes(&self) -> usize` (`self.orientations.pending()`), each with a doc
     comment (queued or sent, not yet confirmed; a turn that waits for its embedded value has no
     write yet and is not counted, as `pending_writes` says today). `pending_writes` stays as it
     is, the sum of the two: `finish_pending_writes` and the exit wait use it and do not change.
  2. **The lines** (`tasks.rs`): `PendingWrite` gets a variant `Rotations(usize)` (doc comment:
     rotation writes queued or sent but not yet confirmed,
     `metadata.pending_rotation_writes()`); the doc comment of `Ratings` names
     `metadata.pending_rating_writes()`. `pending_writes` gets a parameter `rotations: usize` after
     `ratings` and gives the lines in this order: ratings, rotations, "Apply to All Marked", the
     window's write. A count of 0 gives no line, as for ratings. Update its doc comment.
  3. **The text** (`tasks_text.rs`): `pending_write_text` answers `PendingWrite::Rotations(count)`
     with `format!("Writing rotations: {count}")`, the same form as the ratings' line: a count
     after a colon, so it needs no singular or plural form (rule 13). The module comment needs no
     change.
  4. **The wiring** (`main.rs`, `draw_columns`, about line 5995): the call to `pending_writes`
     passes `self.metadata.pending_rating_writes()` and
     `self.metadata.pending_rotation_writes()` instead of `self.metadata.pending_writes()`.
     Nothing else in `main.rs` changes; the drawing already shows each `PendingWrite` through
     `pending_write_text` (`draw_tasks`, about line 8261), so no `Rotations` arm is added
     anywhere in `main.rs`. If it matches on the variants somewhere, stop and report.
     `ratings.rs` has a private struct of the same name (`ratings::PendingWrite`), another type:
     leave it alone.
  5. **The glossary** (`docs/glossary.md`, about line 145): the row
     "Writing ratings: {n}; Saving metadata: {i} of {n}; Saving metadata" gets
     "Writing rotations: {n}" after "Writing ratings: {n}", and its description names rotation
     writes after rating writes ("…; changed by T239"). Name the new term in the report
     (rule 13).
- **Tests to add:**
  - `metadata.rs`, a **new** test `rating_and_rotation_writes_are_counted_apart` beside T237's
    tests (the helpers `turn_file`, `cache_with_orientation`, `FakeSubmit::new`, `turn` and
    `set_rating`; `a_failed_rotation_turns_back_and_its_text_is_the_status_text`, about line 2129,
    already queues a rating and a turn on one path): with a rating write and a turn queued,
    `pending_rating_writes` is 1, `pending_rotation_writes` is 1 and `pending_writes` is 2; a turn
    of a `turn_file` path with no known value (it waits for its embedded value, as about line
    2205) counts in neither;
  - `tasks.rs`, `the_pending_write_lines_appear_and_go` (extended, not a new test): the new
    parameter in all five calls, plus a
    rotation line alone, and all four lines in order (ratings, rotations, batch; and with no batch
    the window's write after rotations); a count of 0 gives no line;
  - `tasks_text.rs`, `the_pending_write_texts` (extended, not a new test):
    `PendingWrite::Rotations(3)` is `"Writing rotations: 3"`.

  So the number of tests grows by one (the `metadata.rs` test); take the counts from
  `test-output.txt`, not from the inventory's own description of which targets hold
  `metadata.rs`.
- **Not in this task:** counting the waiting turns (they have no write yet; a task only if the
  owner wants them shown); ADR 0017 (the reviewer's, see Why); the exit wait's own list of
  unsaved writes (T237, unchanged).
- **Rules:** no change to any `[dependencies]`. No other file changes; if one must, stop and
  explain (rule 2). Every text the user sees as `docs/tasks/README.md` rule 13 says.
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the files above;
  `docs/test-inventory.md` has the new counts (line 4, the run total and "after T87–T239";
  line 20, the `pw-loupe` count) and line 20's text on the Tasks area ("the pending-write
  lines (rating writes, …)") names rotation writes. Then, after the review, the owner's hand
  check (not the agent's; the agent does not claim it): with a few images
  marked, a rating key and a turn at once show two lines, "Writing ratings: {n}" and
  "Writing rotations: {n}", each gone when its writes are done (they may only flash, since
  writes are answered quickly).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Blocked | Added from the owner's decision after the hand check of ADR 0024 (backlog item "'Writing ratings' counts rotations"). Waits for the cold read before Open |
| 2026-11-20 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README and this file only): doable with the named files; every line reference matches; `main.rs` only shows each line through `pending_write_text`, no match on the variants. Changed after it: the `metadata.rs` test is a new, named test and the other two are extensions, so the count grows by one, taken from `test-output.txt`; the helpers for a queued rating, a turn and a waiting turn named; the colon form needs no singular or plural; the new glossary term named in the report; the inventory's lines 4 and 20 (counts and the Tasks-area text) named; `ratings::PendingWrite` left alone; the hand check is the owner's, after the review |
| 2026-11-20 | Open | Cold read done |
| 2026-11-21 | Ready for review | Points 1–5 done. `metadata.rs`: `pending_rating_writes` and `pending_rotation_writes`, `pending_writes` unchanged; new test `rating_and_rotation_writes_are_counted_apart`. `tasks.rs`: `PendingWrite::Rotations`, `pending_writes(ratings, rotations, batch, editor_saving)`, test extended (rotation line alone, all four in order, with and without a batch). `tasks_text.rs`: "Writing rotations: {count}", test extended. `main.rs`: the `draw_columns` call only; no match on the variants. Glossary row gets "Writing rotations: {n}" (new term). Test inventory: 2053 passed, 76 ignored; `pw-loupe` 1027; line 20 names rotation writes and the new test. `.\tools\check.ps1` all OK, no clippy warnings. The owner's hand check follows the review |
| 2026-11-21 | Done | Reviewed: the diff holds only the named files (the other changes in the worktree are another agent's T240–T242) and matches points 1–5 and the tests; `pending_writes` unchanged, the sum of the two new counts; `main.rs` changes only the `draw_columns` call, no match on the variants; the owner's decision is already in ADR 0017 U5-Q4 and U5-Q7 (49508ae). The reviewer's own run: check.ps1 all OK, 2053 passed, 76 ignored, no clippy warnings. No CHANGELOG line: rotation is unreleased and its Added line covers it. The owner's hand check follows |
