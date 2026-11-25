# T324. ADR 0027: the neighbours prefetched at the shown scale between 50 % and 100 % — Done

- **Why:** W3's `latency.ps1` run after T318 (reviewer, 2026-11-24, release build of 160e3d7,
  folder `s6\a1`, 1920×1200, labels `t318-*-51-*`): at 51.2 % (100 % and three steps out) the
  next image's first change takes about 155–180 ms, with sharpening on and off alike (ADR 0008
  V1: 30 ms). Between 50 % and 100 % the neighbours are prefetched at fit and at 100 % only, so
  the next image waits for a full render of its visible region. **Decided by the owner on
  2026-11-25** (ADR 0027 §4.3, "W3's hand check"; W1-Q8 changed): at every free level below
  100 %, 50 % or more included, the neighbours are prefetched at the current scale, as ADR 0008
  §12.1 says ("the shown view and the zoom target"): below 50 % with the 100 % view as today,
  and from 50 % to below 100 % with the **fit view**, since Z goes to fit from there
  (`toggle_zoom`); at fit unchanged. Waits for T322
  (both change `state.rs`; after T322 only the scaled view itself satisfies such a requirement).
- **What the reviewer checked (2026-11-25):** in `crates/pw-viewcore/src/state.rs`,
  `State::wanted` (about line 1053) chooses the neighbours' two views (about lines 1075–1083):
  `(Want::Target, Want::Fit)` at 100 % and above, `(Want::Shown, Want::Target)` below 50 %
  (W1-Q8), and `(Want::Fit, Want::Target)` otherwise, that is at fit and between 50 % and 100 %;
  the comment above it says free levels of 50 % or more are not prefetched. `Want::Shown` for a
  neighbour gives, through `State::req`, its view at the current scale and centre: at exactly
  50 % the half-size view, between 50 % and 100 % a `Scaled` view of its visible region. The test
  `below_50_percent_the_neighbours_prefetch_the_view_at_the_current_scale` (about line 3368)
  ends with "at 50 % or more, unchanged: the fit view" at 0.6.
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/state.rs`,
  `crates/pw-viewcore/src/lib.rs` (the crate's doc line "Free levels are not prefetched.", about
  line 9),
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. In `State::wanted`, a free level below 50 % keeps `(Want::Shown, Want::Target)`; a free level
     from 50 % to below 100 % gives `(Want::Shown, Want::Fit)`; `Zoom::Fit` keeps
     `(Want::Fit, Want::Target)`; 100 % and above unchanged. The comment says so, with ADR 0008 §12.1 and ADR 0027 W1-Q8 (changed by the
     owner 2026-11-25).
     The crate's doc line in `lib.rs` says the neighbours are prefetched at the current scale.
  2. Nothing else changes: the order of the wanted list, the neighbour behind (it gets `primary`,
     now the shown view), the budgets and the eviction.
- **Tests to add / change** (rule 5):
  - `free_zoom_waits_for_the_gesture_to_pause_and_is_not_prefetched` (about line 2982) asserts at
    0.7 that every neighbour's want is `Fit` or `Target` (about lines 3000–3004): changed so the
    neighbours' wants at 0.7 are `Shown` and `Fit`, and renamed
    `free_zoom_waits_for_the_gesture_to_pause_then_prefetches_the_neighbours` (asked for here, not a
    weakening); its part on the gesture's pause is kept.
  - The end of `below_50_percent_the_neighbours_prefetch_the_view_at_the_current_scale` (at 0.6:
    `Want::Fit` wanted, `Want::Shown` not) is changed to the new rule: at 0.6 `Want::Shown` is
    wanted, and `Want::Fit` too, as the second view (asked for here, not a weakening). The test is renamed
    `below_100_percent_the_neighbours_prefetch_the_view_at_the_current_scale`.
  - At 0.512 and 0.75: the next neighbour's first prefetched job is a `Scaled` view at that scale,
    the second its fit view; with sharpening on both carry it.
  - At 0.4: unchanged, the view at 0.4 first, then the 100 % view.
  - At exactly 0.5: the neighbour's first job is the half-size view.
  - At fit: unchanged, the fit view first.
  - At 0.75, the other display mode's prefetched job and the neighbour behind's (−1) are the
    scaled view at 0.75.
  - Stepping to the prefetched neighbour at 0.75 draws its scaled view at once (nothing is
    planned for its shown view).
- **Not in this task:** what is drawn while a view is missing (T322); the thread or worker
  layout. After Done the owner measures with `latency.ps1` at 51.2 % and at one more level
  between 50 % and 100 %, with sharpening off and on (V1, V4); the result goes into ADR 0027 §4.3.
  Accepted cost (owner, 2026-11-25, ADR 0027 §4.3): after the dwell, up to four full-resolution region renders on the
  background worker (+1, +2, the other display mode and −1 get the primary view), each about 3.8
  times a 100 % render at 51.2 %; they depend on the centre, so a pan, a zoom or a resized window
  makes them again and a late result is discarded.
  No `CHANGELOG.md` line from the agent (the reviewer writes one at Done).
- **Rules:** `AGENTS.md`, `docs/tasks/README.md`, `docs/TESTING.md`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added after W3's hand check (owner, 2026-11-25); waits for T322 |
| 2026-11-25 | Blocked | Cold read and chain review (fresh agents): the prefetched view serves after a step (same centre, zoom and size). Added: `lib.rs` (its doc line); the existing test `free_zoom_waits_for_the_gesture_to_pause_and_is_not_prefetched` to change; tests of the other mode and −1; the cost. The owner decided on 2026-11-25: from 50 % to below 100 % the second view is fit, not 100 % (Z's target there), and the cost is accepted, measured after Done (ADR 0027 §4.3) |
| 2026-11-25 | Blocked | T322 Done; waits for T323 (the owner chose T323 first, 2026-11-25; both change `docs/test-inventory.md`) |
| 2026-11-25 | Open | T323 Done |
| 2026-11-25 | Ready for review | Done by the agent: `State::wanted`, `lib.rs` doc line, tests, inventory |
| 2026-11-25 | Done | Reviewed: item 1 as written (50 % to below 100 %: `(Want::Shown, Want::Fit)`), the `lib.rs` line; every test of the list, 0.4 and fit in the kept parts of the renamed test, two new tests (the other mode and −1, the step to a prefetched neighbour). The agent left `docs/test-inventory.md`'s counts unchanged: the reviewer set them by script (2683 passed; `pw-viewcore` unit row 234 → 236, from `test-output.txt`). `.\tools\check.ps1` all OK (the agent's run, newer than the code), 2683 passed, 0 failed, 81 ignored, no clippy warnings. `CHANGELOG.md`: a Changed line (stepping between 50 % and 100 % is quicker). No leftovers; the owner's `latency.ps1` and the hand check of T320–T324 follow (`plans/`, W3's row) |
