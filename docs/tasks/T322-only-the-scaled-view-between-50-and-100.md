# T322. ADR 0027: only the exact scaled view between 50 % and 100 % — Blocked: waits for T321

- **Why:** found in W3's hand check (owner, 2026-11-25; ADR 0027 §4.3, "W3's hand check"):
  between about 51 % and 99 %, the pixels move when sharpening is turned on or off. The cause
  (reviewer, from the code): the current image's ring at 100 % is always prefetched
  (`State::wanted` adds `(ck, Want::Ring)` at every zoom level), and between 50 % and 100 % it
  **satisfies** the shown requirement whenever its region covers the visible one and its
  sharpening is the requirement's (`State::satisfied`). With sharpening off the unsharpened ring
  is therefore drawn, reduced by the GPU; with it on, a `Scaled` view is made and reduced with the
  Lanczos filter on the worker. Two reductions, two pictures. **Decided by the owner on
  2026-11-25:** between 50 % and 100 % only the exact `Scaled` view satisfies the shown
  requirement, as the reduced view below 50 % (W1-Q3) does; a denser view (in practice the ring;
  the 100 % view covers the visible region below 100 % only when the image fits the view) is drawn
  meanwhile and replaced in one swap (V2). Every view below 100 % is then reduced
  with the Lanczos filter (§3.2), with sharpening on and off alike. Waits for T321 (both change
  `state.rs`; after T321 the ring carries the sharpening too, so it would satisfy the requirement
  with sharpening on as well).
- **What the reviewer checked (2026-11-25):** in `crates/pw-viewcore/src/state.rs`:
  - `State::req` (about line 1120): between 50 % and 100 % (`HALF_DENSITY + EPS < scale < 1.0 -
    EPS`) the job is `ViewKind::Scaled` over `cover` (the visible region's sensor box) at
    `density_q: quantise(scale)`.
  - `State::satisfied` (about line 1313): a `Reduced` or `Fine` requirement counts only as itself
    (`exact`, or the same job in flight); every other requirement is satisfied by any resident or
    in-flight view (not fine) with the requirement's sharpening that `serves` it — region
    contains the cover, density at least the need — so a ring (density 1.0, twice the visible
    region at 100 %) satisfies a scaled requirement down to about 51 %.
  - `State::serving_of` (about line 1262), which chooses what is drawn, takes any view that
    serves, the requirement's sharpening first, then the least dense: a resident scaled view at
    the requirement's scale is drawn before the ring, and the ring is drawn while it is missing.
    It needs no change.
  - `exact` and `same_reduction` (about lines 1293 and 381) compare a resident view's region and
    reduction with a job's. `exact` also requires the same region and is not used here: a scaled
    view whose region still contains the cover (a sub-pixel pan) keeps counting. A scaled view's
    density is `min(w / region.w, h / region.h)` of the reduced image (`finish_render`), and the
    worker reduces it to exactly `scaled_size(region, density)`, so `same_reduction` holds for it as
    for a reduced view.
  - The rule is keyed on the job's kind, so it covers every scaled requirement: when an image's fit
    scale lies between 50 % and 100 %, `Want::Fit` (the current image's, the neighbours' and the
    other display mode's) is a scaled job too, and the ring (then the whole image) no longer
    satisfies it. That is the decision too.
  - `State::accept` discards a `Scaled` or `Reduced` result that no wanted requirement is served
    by; unchanged.
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/state.rs`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. In `State::satisfied`, a requirement whose job is `ViewKind::Scaled` is satisfied only by a
     resident view of kind `Scaled` with the requirement's sharpening that `serves` it and was
     reduced at the requirement's scale (`same_reduction(r.region, r.density, req.job)`), or by an
     in-flight job of kind `Scaled` with the requirement's sharpening and `density_q` that
     `serves` it. A denser view (the ring, the 100 % view, a scaled view of a larger scale) no
     longer counts, so the plan asks for the scaled view. `exact` is not used (see above); the doc
     comment of `same_reduction` names the scaled view with the reduced one.
  2. **The pan's pause** (owner, 2026-11-25): `State::pan` (about line 817) does not count as a
     gesture today, so after item 1 every frame of a drag between 50 % and 100 % would ask for a
     new scaled view and throw the last one away. `State` gets the time of the last pan step
     (`pan_at: Option<Instant>`, set in `pan` when the centre changes); while it is younger than
     `config.gesture_pause` and the current image's shown requirement is a scaled view,
     `State::wanted` does not add the current image's `Want::Shown` (the other wants are
     unchanged), so nothing scaled is planned for it; `next_deadline` (about line 2814) also
     considers `pan_at + gesture_pause`, so the plan is made again when the drag pauses. A pan at
     100 % and above or below 50 % is unchanged.
  3. Nothing else changes: what is drawn meanwhile (`serving_of`), the jobs, the cache, the
     prefetch. The doc comment of `satisfied` names the scaled view with the reduced and fine
     views, and gives the reason (ADR 0027 §4.3, owner 2026-11-25).
  4. If an existing test expects a denser view to satisfy a requirement between 50 % and 100 %
     (for example that nothing is planned at 0.75 with a ring resident), stop and report it by
     name (rule 5); do not change it.
- **Tests to add** (in `state.rs`):
  - With the current image's unsharpened ring resident (density 1.0, its region covering the
    visible region at 0.75), at 0.75 with sharpening off: the plan asks for the scaled view at
    0.75; the frame draws the ring until the scaled view is accepted, then the scaled view.
  - The same with sharpening on and the ring sharpened with the same values: the scaled view is
    asked for too (the case that differed before).
  - At 0.512 (100 % and three steps out) with a ring whose region still covers the visible
    region: the scaled view is asked for.
  - A resident scaled view at 0.75 with the requirement's sharpening over the same region:
    nothing is planned for the shown view. With no ring resident, a scaled view with the other
    sharpening is drawn until the new one is accepted (W3-Q4, unchanged); with a ring of the
    requirement's sharpening resident, the ring is drawn meanwhile (`serving_of` ranks the
    sharpening before the density).
  - An image whose fit scale lies between 50 % and 100 %, at fit, with its ring resident: the
    scaled view at the fit scale is asked for.
  - An in-flight scaled job at 0.75 with the requirement's sharpening: it is not planned twice.
  - At 0.75, right after a pan step: no scaled job is planned for the current image and the ring
    is drawn; `next_deadline` is at most the gesture's pause; once `pan_at` is older than the
    pause (set back, as the gesture tests do with `gesture_at`), the scaled view is planned. At
    1.0 a pan step plans as before.
- **Not in this task:** the neighbours' prefetch between 50 % and 100 % (T324). After a pan
  between 50 % and 100 %, the ring (GPU-reduced) is drawn until the drag has paused and the new
  scaled view has arrived, then one swap with a slightly sharper look; the reviewer adds it to
  `known-limits.md` at Done. Also noted there: while the ring is drawn meanwhile with
  sharpening on, the mark says shown (the ring carries the sharpening), as below 50 % with the
  half-size view. No `CHANGELOG.md` line from the agent (the reviewer widens the unreleased
  "Changed" line of the reduction from below 50 % to below 100 % at Done; the moving pixels never
  shipped).
- **Rules:** `AGENTS.md`, `docs/tasks/README.md`, `docs/TESTING.md`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added after W3's hand check (owner, 2026-11-25); waits for T321 |
| 2026-11-25 | Blocked | Cold read and chain review (fresh agents): the cause confirmed from the code, no existing test breaks. Added: `exact` not used and why `same_reduction` holds for a scaled view; the rule covers every scaled requirement (fit between 50 % and 100 % too) with a test; test 4's setup; the mark says shown while the sharpened ring is drawn (known-limits); the CHANGELOG line under Changed. The owner decided the pan's pause on 2026-11-25 (item 2, ADR 0027 §4.3) |
