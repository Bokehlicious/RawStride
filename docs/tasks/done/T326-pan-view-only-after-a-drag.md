# T326. ADR 0027: the pan view only after a drag — Done

- **Why:** found in the check after T325 (owner and reviewer, 2026-11-25; ADR 0027 §4.3, "The
  check after T325"): `latency.ps1` at 51.2 % and 64 % showed 5–6 of 13 steps with no change
  within 8 s, each with about 5 s of CPU (the next image opened and rendered from the start). The
  pan view (T325) is asked for on every image after the dwell; at 51.2 % it is nearly the whole
  image, a render under way is not cancelled by a step, and the worker is still busy with it when
  +2 should be prefetched. **Decided by the owner on 2026-11-25** (alternative A): the pan view is
  asked for only once the current image has been **dragged** since it became current. Stepping
  without dragging pays nothing; the first drag on an image keeps the last drawn view still until
  it pauses, the drags after it run on the pan view.
- **What the reviewer checked (2026-11-25):** in `crates/pw-viewcore/src/state.rs`:
  - `State::pan` (about line 832) sets `pan_at` when the centre moved; it does nothing at fit or
    before the size is known.
  - `State::set_view` (about line 750) sets zoom and centre from outside (the compare's second
    core follows the first, ADR 0017 U6); it returns early when nothing changed.
  - `State::go` (about line 854) makes a position current and resets `since`; `open_folder`
    calls `go(0)` after setting `cur` to `None`; `set_files` sets `cur` itself and resets `since`
    only when the current path changed (about line 1057).
  - `State::wanted` adds `(ck, Want::Ring)` after the other display mode: the 100 % ring when
    `pan_view` is `None`, the pan view when it is `Some` and of use and (`full` or not
    `in_pan()`) (about line 1138, T325).
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/state.rs`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. A new field of `State`, `dragged: bool`, with a doc comment: whether the current image has
     been dragged since it became current (ADR 0027 §4.3, owner 2026-11-25). It starts `false`.
  2. It becomes `true` in `State::pan` when the centre moved (where `pan_at` is set), and in
     `State::set_view` when the centre changed and the zoom did not (the second core of the
     compare is dragged that way; a zoom does not count): compared after the clamping, before
     the values are stored (`zoom == self.zoom && centre != self.centre`).
  3. It becomes `false` in `State::go` when the new, clamped position differs from `self.cur`
     (`go` to the same position keeps it), in `open_folder`, and in `set_files` where the current path
     changed (where `since` is reset). A zoom step, Z, `set_zoom`, `set_mode` (R) and
     `set_sharpening` keep it.
  4. In `State::wanted`, the pan view (the `Some` arm of `pan_view`) is added only when
     `self.dragged` holds, in the full plan too (`full`), so a pan view that is no longer wanted
     is neither protected nor kept by `accept`. The `None` arm (the 100 % ring at 100 % and
     above and at 50 % and below) is unchanged: the ring does not depend on `dragged`.
  5. `State::req`, `pan_view`, `satisfied`, `serving_of` and the budgets are not changed.
  6. The comments of `Want::Ring` and of the `wanted` code that adds it say that the pan view is
     asked for only after the image has been dragged (ADR 0027 §4.3, owner 2026-11-25).
- **Tests to add / change** (in `state.rs`; rule 5):
  - At 0.512 and 0.75, past the dwell, with the current image's shown view resident and not
    dragged (a state built in the test, or `shown_resident` with `s.dragged = false` set back): neither `wanted(false)` nor `wanted(true)` holds `(k(10), Want::Ring)`, no scaled
    job other than the shown view's is planned for image 10, and the neighbours are planned as
    before.
  - After `pan` moved the centre and the pan's pause has passed (`pan_at` set back, as the
    gesture tests do): the pan view is planned. A `pan` at fit, or one that does not move the
    centre (at the image's edge), leaves `dragged` false.
  - `set_view` with only the centre changed sets `dragged`; with the zoom changed (the centre
    too) it does not.
  - A step to another image (`go`) clears it, `go` to the same position keeps it; `open_folder`
    clears it; `set_files` with the current path at a new position keeps it, with another path
    current clears it.
  - A zoom step (`zoom_by`) and `set_mode` keep it: after the zoom gesture's pause the pan view at
    the new scale is planned.
  - At 1.0, 2.0 and 0.4 the 100 % ring is wanted with `dragged` false, as before.
  - **The T325 tests** that expect a pan view without a drag:
    `between_50_and_100_percent_the_plan_asks_for_a_pan_view_not_a_ring`,
    `a_short_pan_inside_the_pan_view_plans_nothing_and_draws_the_pan_view`,
    `a_pan_past_the_cover_plans_a_new_pan_view_only_after_the_pause`,
    `during_a_drag_neither_the_shown_view_nor_the_pan_view_is_planned_between_50_and_100`,
    `no_pan_view_is_wanted_when_the_shown_view_is_the_whole_image` and
    `an_accepted_pan_view_satisfies_the_shown_requirement_while_it_contains_the_visible_region`
    (owner, 2026-11-25): in each, the image is marked as dragged (`s.dragged = true`) right after
    the state is made (in `shown_resident` for the tests that use it; by hand where a test
    builds its state with `state(1024)`, in `an_accepted_pan_view_…` both of its two states),
    and no assertion is changed. In `no_pan_view_is_wanted_when_the_shown_view_is_the_whole_image` the image is
    marked too, so the test still shows that a whole-image pan view is not wanted when dragged.
    Run them before and after; any of them that then fails, and any other existing test that
    fails, is stopped at and reported by name (rule 10); do not change it otherwise.
- **Not in this task:** cancelling a render under way when the user steps (alternative C, not
  chosen); the pan view's size (`PAN_VIEW_SIZE`, `PAN_VIEW_KEEP`); a drag followed at once by a
  step still lets the pan view's render delay the next image (accepted with A; the reviewer
  writes it into `known-limits.md` at Done, and edits the item "A drag between 50 % and 100 %"
  for the first drag on an image). No `CHANGELOG.md` line from the agent. After Done the owner
  re-checks the drag at 51–59 % (the first drag on an image still, the later ones smooth) and
  measures stepping with `latency.ps1` at 51.2 % and 64 %, sharpening off and on (V1, V4); the
  result goes into ADR 0027 §4.3. Accepted with point 2: in the compare, a new held image
  shown at the same zoom with another centre (`set_view` from `pw-loupe`) counts as dragged for
  the second core, without a drag (the reviewer notes it in `known-limits.md` at Done).
- **Rules:** `AGENTS.md`, `docs/tasks/README.md`, `docs/TESTING.md`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Open | Added after the check of T325 (owner, 2026-11-25): alternative A chosen of A (the pan view only after a drag), B (a smaller pan view) and C (a render that a step cancels) |
| 2026-11-25 | Open | Cold read (a fresh agent, 2026-11-25): doable with the named file and points, line references right, no other existing test expected to fail. Changed after it: `set_view`'s comparison after the clamping, `go` compares the clamped position, the "not dragged" test builds its state or sets the flag back, `an_accepted_pan_view_…` marks both its states by hand, and the compare's `set_view` of a new held image at the same zoom counting as a drag written under "Not in this task" |
| 2026-11-25 | Ready for review | Done by the agent: `dragged` in `State` (set in `pan` and `set_view`, cleared in `go`, `open_folder`, `set_files`), the pan view in `wanted` only when dragged; six T325 tests mark the image as dragged, no assertion changed; 8 new tests; `check.ps1` all OK (2698 passed) |
| 2026-11-25 | Done | Reviewed and approved by the owner on 2026-11-25; the owner's hand check follows (the first drag on an image still, the later ones smooth, at 51–59 % and one more level between 50 % and 100 %; `latency.ps1` at 51.2 % and 64 %, sharpening off and on; the result goes into ADR 0027 §4.3). Leftovers in `known-limits.md` (Sharpening): "A drag between 50 % and 100 %" rewritten (the first drag), "A drag and a step at once between 50 % and 100 %" and "The compare's second core counts a new image as dragged" added; cancelling a render is in `backlog.md` ("A render that a step cancels"). `CHANGELOG.md`: T325's line under Changed changed (after the first drag), no new line |
