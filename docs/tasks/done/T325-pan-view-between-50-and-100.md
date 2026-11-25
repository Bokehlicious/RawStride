# T325. ADR 0027: a pan view between 50 % and 100 % — Done

- **Why:** found in W3's hand check after T324 (owner, 2026-11-25; ADR 0027 §4.3, "The pan between
  50 % and 100 %"): with the Sony A1's RAW files, dragging the image with the left mouse button
  between about 51 % and 59 % stutters through the whole drag, with sharpening on and off alike;
  at other levels it runs better. The cause (reviewer, from the code, not profiled): while a drag
  goes on between 50 % and 100 %, no scaled view is asked for (T322) and the ring is drawn. The
  ring (`State::req`, `Want::Ring`) is made over the region visible at **50 %**,
  `geometry::visible(size, view_px, 0.5, centre)`, at density 1.0. At scale `s` the visible
  region is `0.5 / s` of it, so the margin left for panning is `1 − 0.5 / s`: about 2 % at 51 %,
  15 % at 59 % and 33 % at 75 %. At 51–59 % a drag leaves the ring at once, nothing sharp covers
  the view, and a new ring (a full-resolution render) is asked for every time the drag has
  paused. **Decided by the owner on 2026-11-25** (alternative B): between 50 % and 100 % the ring
  is replaced by a **pan view**, a scaled view at the shown scale over a region larger than the
  visible one, with the same margin at every level. It is reduced with the same filter as the
  shown view, so the swap to the exact view moves nothing.
- **What the reviewer checked (2026-11-25):** in `crates/pw-viewcore/src/state.rs`:
  - `State::wanted` adds `(ck, Want::Ring)` after the neighbours and the other display mode, at
    every zoom level (about line 1127). `State::req` makes the ring at `Want::Ring` (about line
    1179): kind `ViewKind::Ring`, region the sensor box of the region visible at 0.5, density 1.0,
    `cover` that same region.
  - `State::req` makes the shown view between 50 % and 100 % as `ViewKind::Scaled` over `cover`,
    the sensor box of the visible region, at `density_q: quantise(scale)` (about line 1230).
  - `State::satisfied` (about line 1337): a `Scaled` requirement is satisfied only by a resident
    `Scaled` view with its sharpening that `serves` it (region contains the `cover`, density at
    least the need) and was reduced at its scale (`same_reduction`), or by the same in flight.
    A `Scaled` view over a larger region at the same scale therefore satisfies the shown
    requirement while it contains the visible region, which is what the pan view is for.
  - `State::serving_of` (about line 1288) draws, of the views that serve, the requirement's
    sharpening first, then the least dense, then the smallest region: at the same density the
    exact shown view is drawn while it covers, the pan view once the drag has left it.
  - `State::wanted` skips `(ck, Want::Shown)` while `in_pan()` and the shown requirement is a
    scaled view (about line 1087, T322); `geometry::visible` clamps the region to the image.
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/state.rs`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. In `State::req`, for `Want::Ring` of an image whose **shown requirement is a `Scaled` view**
     (a free scale above 50 % and below 100 %, or fit when the fit scale lies there), the
     requirement is a **pan view** instead of the 100 % ring:
     - `job`: kind `ViewKind::Scaled`, `region` the sensor box of
       `geometry::visible(size, view_px, scale / 2.0, centre)` (two times the visible region on
       each axis, clamped to the image), `density_q: quantise(scale)` (the shown scale),
       `sharpen: self.sharpening`;
     - `cover`: the sensor box of `geometry::visible(size, view_px, scale / 1.5, centre)` (1.5
       times the visible region on each axis, clamped), so a resident pan view that still
       contains it counts as done and a short drag (up to a quarter of the view on each side)
       does not make a new one;
     - `density: scale`.
     Name the two factors as constants next to `HALF_DENSITY` with a doc comment that says what
     they are (the pan view's size and the margin that keeps it), so the owner can tune them.
     Everywhere else (100 % and above, 50 % and below, the half-size view) `Want::Ring` is the
     100 % ring as today.
  2. If the pan view's region is the whole image, or equals the shown requirement's job region
     (an image that is small in the view, or fit), no pan view is wanted: `State::wanted` does
     not add `(ck, Want::Ring)` then, since the shown view already serves every pan.
  3. While `in_pan()`, `State::wanted` does not add the pan view either (as it does not add the
     shown view, T322): a drag leaving the pan view would otherwise ask for a new one every
     frame. After the drag has paused as long as a zoom gesture's pause, the plan is made again
     (`next_deadline` already considers `pan_at`) and asks for the shown view and the pan view.
  4. `State::satisfied`, `State::serving_of` and `State::serves` need no change if the
     reviewer's reading above holds: check that a resident pan view satisfies the shown
     requirement through `same_reduction` (same `density_q`, a larger region) and that the shown
     view alone does not satisfy the pan requirement (its region is smaller than the `cover`). If
     either does not hold, stop and report it (rule 10); do not change `satisfied` on your own.
  5. The memory budget and the eviction treat the pan view as a scaled view; do not change them.
     Check that it is evicted before the current image's shown view when the budget is tight,
     as the ring is, and report what you found.
  6. The comments of `Want::Ring`, `ViewKind::Ring` (the "ring for panning at 100 %" line, about
     line 348) and `State::satisfied` say that between 50 % and 100 % the pan view takes the ring's
     place (ADR 0027 §4.3, owner 2026-11-25).
- **Tests to add / change** (in `state.rs`; rule 5):
  - At 0.512, 0.6 and 0.75 with the current image's shown view resident: the plan asks for a pan
    view; its region contains the visible region with half a view on each side (clamped), its
    density is the shown scale, its kind is `Scaled`; no `Ring` job is planned. With sharpening
    on, both carry it.
  - At 1.0, 2.0 and 0.4: `Want::Ring` is the 100 % ring as before (kind `Ring`, density 1.0).
  - With a pan view resident at 0.512: a pan of a quarter of the view plans nothing for the
    shown view and nothing for the pan view; the frame draws the pan view once the shown view no
    longer serves; a pan past `cover` plans a new pan view only after the pan's pause (`pan_at`
    set back, as the gesture tests do).
  - During a pan (`pan_at` younger than the pause): neither the shown view nor the pan view is
    planned between 50 % and 100 %; at 1.0 a pan step plans as before.
  - An image whose fit scale lies between 50 % and 100 %, at fit: no pan view is wanted (the
    shown view is the whole image); the same for an image smaller than the view.
  - The accepted pan view (`accept`) satisfies the shown requirement while it contains the
    visible region, so no exact shown view is asked for after a short pan; a resident pan view
    of the other sharpening is drawn until the new one is accepted (W3-Q4, unchanged).
  - If an existing test expects the 100 % ring between 50 % and 100 % (for example
    `a_ring_serves_a_pan_inside_it` at a scale in that band), stop and report it by name; do not
    change it.
- **Not in this task:** what is drawn once a drag has left the pan view (`serving_of`, unchanged:
  a denser or less dense view that serves, else the last drawn view); the shown view's size; the
  neighbours' prefetch (T324); the worker or thread layout. The cost, which the owner accepted
  with alternative B (2026-11-25): after the dwell, one scaled render of about 4 times the
  visible region at the shown scale in the ring's place in the plan (after the neighbours and
  the other display mode), made again after a pan beyond `cover` and after every zoom step or
  window resize; its texture is about 4 times a shown view's (about 37 MB at 3840×2400 pixels
  per sharpening kept). After Done the owner re-checks the drag at 51–59 % and at one more level
  between 50 % and 100 %, and measures stepping with `latency.ps1` at 51.2 % once more (V1, V4);
  the result goes into ADR 0027 §4.3. No `CHANGELOG.md` line from the agent (the reviewer writes
  one at Done) and no change to `known-limits.md` (the reviewer edits the item on T322's drag).
- **Rules:** `AGENTS.md`, `docs/tasks/README.md`, `docs/TESTING.md`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Open | Added after W3's hand check (owner, 2026-11-25): alternative B chosen of A (a larger ring), B (a pan view at the shown scale) and C (a known limit) |
| 2026-11-25 | Open | Sent back to the agent after the code (points 1–3, 6) was in place and no test was written. Five T322 tests failed (as the last test point foresaw): `a_ring_does_not_satisfy_the_scaled_view_with_sharpening_off_or_on`, `three_steps_out_from_100_percent_the_scaled_view_is_asked_for`, `a_resident_scaled_view_is_planned_once_and_a_changed_sharpening_asks_again`, `a_fit_scale_between_50_and_100_percent_asks_for_the_scaled_view`, `a_scaled_view_waits_for_a_drag_to_pause`. Cause: their fixture `out(&s, 10, Want::Ring)` now gives the pan view between 50 % and 100 %, which satisfies the shown requirement. Their rule (a 100 % ring does not satisfy the scaled view) still holds, and such a ring stays resident after zooming out from 100 %. **Owner, 2026-11-25:** the reviewer added the test helper `ring_out` (the 100 % ring built explicitly) and used it in `scaled_setup` and the two other places; no assertion changed; all `state::tests` pass. Left for the agent: point 4 and 5's checks and report, every test under **Tests to add** except the last point (done here), `docs/test-inventory.md`, `.\tools\check.ps1`, then Ready for review. Keep `ring_out` for a 100 % ring between 50 % and 100 %; `out(…, Want::Ring)` there is the pan view |
| 2026-11-25 | Ready for review | Points 1–3 and 6 done earlier; 4 checked by the new tests (a resident pan view satisfies the shown requirement through `same_reduction`, the shown view alone does not satisfy the pan requirement); 5 checked in the code (`protected_views` keeps the view that serves each want of the full plan, so the pan view is protected as the ring was; `estimate_bytes` counts it as `Scaled`, area·d²·4). Six tests added in `state.rs`; `docs/test-inventory.md` updated. `check.ps1` all OK, 2690 passed, no clippy warnings |
| 2026-11-25 | Open | Review (reviewer, owner agreed 2026-11-25): the code (points 1–3, 6) and the reports on points 4 and 5 are approved; `check.ps1` all OK. Sent back for the tests only: (1) `a_short_pan_inside_the_pan_view_plans_nothing_and_draws_the_pan_view` and `a_pan_past_the_cover_plans_a_new_pan_view_only_after_the_pause` run at 0.75, but the task asks for 0.512 (the level of the owner's finding): run them at 0.512 (keeping 0.75 as well is fine), and the short pan is **a quarter of the view** as written, not a fifth (the reviewer checked with a temporary test, since removed, that the code passes at 0.512 with a fifth and with a quarter); use `s.pan(...)` in view pixels or derive the centre step from the scale, not a constant tied to 0.75; (2) in `an_accepted_pan_view_satisfies_the_shown_requirement_while_it_contains_the_visible_region`, the `.all(...)` over `scaled_renders` also passes when nothing is planned: assert first that it is not empty; (3) the last log row says six tests were added, it is seven: say so in the new row. Then `docs/test-inventory.md` if the count changes, `.\tools\check.ps1`, Ready for review |
| 2026-11-25 | Ready for review | Review points done: (1) the short-pan and past-the-cover tests run at 0.512 and 0.75, the short pan is a quarter of the view, the centre step derived from the scale (`centre_step`); (2) the accepted-pan-view test asserts the sharpened pan view is planned before checking its sharpening; (3) seven tests were added, not six (the inventory already counts seven: 243 in the row, 2690 in all). `check.ps1` all OK |
| 2026-11-25 | Done | Reviewed and approved by the owner on 2026-11-25; the owner's hand check follows (the drag at 51–59 % and at one more level between 50 % and 100 %, `latency.ps1` at 51.2 %; the result goes into ADR 0027 §4.3). Leftovers: the drag that leaves the pan view or comes before it is resident is in `known-limits.md` (Sharpening, "A drag between 50 % and 100 %", rewritten for T325); the hand check and the measurement in the W3 row of `plans/adr-0018-0025-steps.md`. `CHANGELOG.md`: one line under Changed |
