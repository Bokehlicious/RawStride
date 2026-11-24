# T316. ADR 0027 W3: sharpening in the view core — Done

- **Why:** ADR 0027 §3.3 and W3-Q1, W3-Q4, W3-Q6 (answered by the owner 2026-11-24, ADR 0027
  §4.3). While sharpening is on, every view below 100 % made on the worker is sharpened last,
  after the reduction and before the conversion to the monitor profile, with T315's
  `sharpen::sharpen`. The view's key gains the sharpening (on or off, and the three values), so a
  change never shows an old texture as the new one; the drawn view stays until the new one is
  ready and is replaced in one swap (V2). At 100 % and above sharpening is paused (A-Q8). This
  task makes the view core's part and tells the shell the state for the mark; the key, the mark
  and the settings are T317's and T318's. The third task of the chain T315 → T319 → **T316** → T317 →
  T318.
- **What the reviewer checked (2026-11-24):**
  - `pw-viewcore` `state.rs`: `JobView { kind, region, density_q }` (derives `Hash`, `Eq`; in
    `Job::Render`, `Task::Render` and `Inflight`); `ViewOut` (made in `finish_render`, two
    places) and `Resident` (made in `accept` from the `ViewOut`) carry `kind`, `region`,
    `density`. `req(key, want)` makes the `JobView`: `Reduced` below 0.5, `Half` at ≤ 0.5 (and
    for the shown view during a gesture, `in_gesture()`), `Zoom` at ≥ 1, `Scaled` between, `Ring`
    for `Want::Ring`, `Fine` for `Want::Fine`. `serves` (generation, key, region, density, the
    reduced views' 2:1 rule, a `Fine` view only while `fine_details`); `serving_of` (a fine view
    first while fine details is on, then the least dense, then the smallest); `exact` (kind,
    region, `same_reduction`); `satisfied` (`Reduced` and `Fine` only as themselves via `exact`
    and the inflight `f.view == job`; other kinds via `serving_of` or an inflight view that
    serves); `remove_inflight(generation, key, kind, region, density)`; `accept` (keeps a
    `Scaled` or `Reduced` result only if it serves a wanted requirement; `Fine` while fine details
    is on); `protected_views` (what serves each want, the drawn view, and T313's exact reduced
    view). `frame()` builds `Frame` (its `fine: FineState` is the model for a state the shell
    reads). The test helpers `out`, `out_in` and `half_out` build `ViewOut` literals.
  - `core.rs`: `Task::Render` makes the image per kind (`Reduced`, `Scaled` and `Fine` reduced
    with `scale::reduce`), then `to_display`, then the check `is_consistent` that gives
    `DecodeError::new("the decoder returned a malformed image")`. `ViewCore::set_fine_details`
    is the model for a setter (lock, compare, set, `cv.notify_all`, `wake_ui`).
  - `lib.rs` re-exports `FineState`, `FinePause` and the other frame types.
  - `Frame` is built as a literal outside `pw-viewcore` only in `pw-loupe` `loupe_text.rs`'s
    tests (`frame`, `tiff_frame`, `small_tiff_frame`); `ViewOut` only inside `state.rs`.
- **Crates:** `pw-viewcore`, `pw-loupe` (test helpers only). **Files:**
  `crates/pw-viewcore/src/state.rs` (points 1–7 and its tests; its `ViewOut` test helpers get the
  new field), `crates/pw-viewcore/src/core.rs` (points 1 and 8), `crates/pw-viewcore/src/lib.rs`
  (`SharpenState` exported), `crates/pw-viewcore/tests/engine.rs` (the engine test),
  `crates/pw-loupe/src/loupe_text.rs` (the new `Frame` field in its test helpers, as
  `SharpenState::Off`; nothing else), `docs/test-inventory.md` (rule 12), and this task's own
  file (status and log).
- **What:**
  1. **The setting:** `State` gets `sharpening: Option<Sharpening>` (`None` = off; off at start,
     not saved: the shell holds it, W3-Q5) and `State::set_sharpening(s: Option<Sharpening>) ->
     bool`: a `Some` value is first made valid with `Sharpening::clamped()`; a value equal to the
     one held changes nothing and gives `false`. `ViewCore::set_sharpening(s)` calls it under the
     lock and, when it changed, notifies as `set_fine_details` does; not a user request, nothing
     recorded. `Sharpening` (T315, re-exported) and `ViewCore::set_sharpening` are public.
  2. **The job's sharpening:** `JobView` gets `sharpen: Option<Sharpening>`, and `ViewOut` and
     `Resident` the same field, copied from the job (`finish_render`) and the output (`accept`).
     So every place that compares a `JobView` (the inflight `f.view == job`, `Job::Render` in the
     plan) tells a sharpened job from an unsharpened one. Every `JobView` literal gets the field:
     in `req` (point 3); `STATISTICS_LEND` (about line 401) and the test helpers (`reduced_job`,
     `half_job`, `o_job`, `fine_job`, the one at about line 5624, the fallback in `out_in`) get
     `None` unless a test needs another; the `half` of `finish_render`'s unreducible branch
     (about line 1993) and its Half `ViewOut` copy the job's `view.sharpen`. `remove_inflight`
     gets a `sharpen: Option<Sharpening>` parameter and matches it (two jobs that differ only in it
     can be in flight after a quick toggle); its four callers (`finish_render` twice, `accept`,
     `reject`) pass `view.sharpen` or `out.sharpen`.
  3. **Which views are sharpened** (W3-Q1): `req` sets `sharpen` to `self.sharpening` for the
     kinds `Reduced`, `Fine`, `Scaled` and `Half` (also the half-size view a gesture asks for: it
     is drawn at 1:1 at 50 % when the gesture ends), and `None` for `Zoom` and `Ring` (100 % and
     above, A-Q8). In RAW and JPEG mode alike (§2.2). A half-size image that a `Reduced` job
     returns as it is (the unreducible case, T309 R1, `core.rs` about lines 96-106) is sharpened
     too (point 8), and its view carries `view.sharpen` (point 2).
  4. **What is drawn** (W3-Q4, V2): `serves` is unchanged (a view of either sharpening serves by
     region and density). `serving_of` orders the serving views: a fine view first while fine
     details is on (as today), **then a view whose `sharpen` equals the requirement's
     `job.sharpen`**, then the least dense, then the smallest. So the drawn view stays until a
     view with the wanted sharpening is ready and then is replaced in one swap; with fine details
     on, a fine view with the old sharpening stays until the fine view with the new one replaces
     it (never fine → reduced → fine, as T313). `gesture_stand_in` is unchanged: during a gesture
     the stand-in may be a view of either sharpening (whatever is resident), and the mark then
     shows waiting (point 7).
  5. **What the plan counts as done:** `exact` also requires an equal `sharpen`. `satisfied`
     counts, for every kind, only a serving resident view or a serving view in flight (not
     `Fine`, as today) **whose `sharpen` equals `req.job.sharpen`** (not only `serving_of`'s one
     pick); **except** the current image's `Half` requirement for `Want::Shown` while
     `in_gesture()`, which a `Half` view of either sharpening, resident or in flight, satisfies,
     so a gesture never starts a render only for the sharpening (the sharpened one follows when
     the gesture ends, if the view is still wanted). So turning sharpening on or off, or changing
     a value, makes the views the plan wants again (the cost accepted in W3-Q4).
  6. **The cache** (W3-Q4): `accept` is unchanged in what it keeps: its "wanted" check uses
     `serves`, which ignores the sharpening, so a result whose sharpening is no longer the current
     one is kept as the memory budget allows (it is the view drawn at once when sharpening is
     turned back), and evicted first by its age as today. `protected_views` is unchanged: it
     protects what `serving_of` picks, so the old view while it is still drawn; its T313 branch
     uses `exact`, which now matches the sharpening, so it protects the new reduced view. A
     neighbour under a tight budget is not planned forever: `pick` checks `textures_fit` as
     today. Do not change these. **One change (chain review):** when `pick` checks whether a
     `Fine` render fits the budget (about line 1792), the bytes of a resident fine view of the
     **same image with another sharpening** count as free (the new one replaces it, and it is
     evicted when the new one is accepted, also if it is protected then). Otherwise turning
     sharpening on with fine details on and a tight budget would refuse the new fine render and
     pause fine details for the image until a new folder (`fine_failed`), caused only by the
     sharpening.
  7. **What the frame says:** `Frame` gets `sharpen: SharpenState`; `SharpenState` is exported
     from `lib.rs` and derives at least `Clone, Copy, Debug, PartialEq, Eq`: `Off` (no image,
     `Status::Empty`; or sharpening is off and the drawn view, if any, is unsharpened); `Paused`
     (sharpening is on and the current scale is 100 % or more, by **the same rule `req` uses to
     choose `Zoom`**, `scale >= 1.0 - EPS`, A-Q8); `Shown` (sharpening is on, the frame's view is
     ready, `Status::Ready`, and the drawn view is of the current key and generation and its
     `sharpen` equals `self.sharpening`); `Waiting` otherwise: the new view is not ready, a
     gesture is under way (`in_gesture()`), the size is not known yet, the image cannot be shown
     (`Status::Error`), **or sharpening was just turned off and a sharpened view is still drawn**
     (chain review: the mark must not vanish while a sharpened picture is shown). The order:
     `Off`, then `Paused`, then `Waiting` during a gesture, then `Shown`, then `Waiting`.
     Worked out in a `sharpen_state` beside `fine_state` (about line 2404), and set also on
     `frame()`'s early returns (no image, about line 2326: `Off`; a failed image, about line
     2338: `Off` while sharpening is off, else `Waiting` below 100 %, `Paused` at 100 % and above).
  8. **The render** (`core.rs`, `Task::Render`): after the kind's image is made and reduced and
     **before** `to_display`, a view whose `sharpen` is `Some(s)` is sharpened with
     `sharpen::sharpen(&img, s)`; a `None` result gives
     `DecodeError::new("the decoder returned a malformed image")`, the same text as the check
     after `to_display` (W3-Q6: no new text; with a valid `s` it can only fail on inconsistent
     data). For `Reduced`, `Scaled` and `Fine` a malformed image already fails in the reduction
     ("scaling the view failed"); only a `Half` view reaches `sharpen` unreduced, so the existing
     tests of a malformed half-size image (`core.rs` about line 1155) keep their text. Then
     `to_display` and the check as today. The focus, clipping and statistics jobs are
     unchanged (§2.1).
- **Tests to add** (in `state.rs`'s tests with its test backend, unless named otherwise; the
  helpers that build a `ViewOut` (`out`, `out_in`, `half_out`) set `sharpen` themselves, and a
  helper of a chosen sharpening, for example `out_with(…, sharpen)`, is added for the tests that
  need views of both; `half_out` no longer takes it from the `Fit` job):
  - With sharpening off, every job of the plan has `sharpen: None` (fit, 50 %, 75 %, 100 %).
  - With it on: at fit below 0.5 the shown job is `Reduced` with `sharpen: Some(s)`; at exactly
    0.5 `Half` with `Some(s)`; at 0.75 `Scaled` with `Some(s)`; at 1.0 `Zoom` and the ring with
    `None`; the same in JPEG mode; a neighbour's prefetched view at fit has `Some(s)`.
  - During a zoom gesture at ≤ 0.5 the shown job is `Half` with `Some(s)`; a resident
    unsharpened half-size view satisfies it (no new render is planned), and `sharpen` is
    `Waiting` during the gesture.
  - The unreducible case (a half-size render smaller than the reduced size, T309 R1) with
    sharpening on: its view is a `Half` with `Some(s)`, and no further render is planned.
  - `State::set_sharpening` with an invalid value holds `clamped()`, and with the same value again
    returns `false`.
  - Turning it on with the unsharpened reduced view resident: the frame still draws that view's
    id, `sharpen` is `Waiting`, and the plan holds a sharpened `Reduced` render; after it is
    accepted the frame draws the new id and `sharpen` is `Shown`.
  - Turning it off with both views resident: the frame draws the unsharpened view's id at once,
    `sharpen` is `Off`, and no render is planned.
  - Changing a value (`set_sharpening` with another amount) plans new sharpened views; setting
    the same value again plans nothing new.
  - At 100 % with sharpening on: `sharpen` is `Paused`; zooming back below 100 % applies it
    again with no key; at a scale of `1 − EPS/2` the shown job is `Zoom` with `None` and the state
    is `Paused`, and at `1 − 2·EPS` the job is `Scaled` with `Some(s)` and the state is not
    `Paused`.
  - Turning sharpening off while only a sharpened view is resident: `sharpen` is `Waiting` until
    the unsharpened view is drawn, then `Off`.
  - With fine details on, a fine view drawn and a budget that holds only one fine view, turning
    sharpening on: the sharpened fine render is not refused, fine details is not paused for the
    image, and once it is accepted the old fine view is evicted.
  - With fine details on and a fine view drawn, turning sharpening on keeps that fine view drawn
    (the sharpened reduced view, when ready, does not replace it) until the sharpened fine view
    is accepted.
  - Two jobs differing only in sharpening in flight at once: accepting one removes only its own
    inflight entry.
  - `tests/engine.rs`: with the core running, the fit view made with sharpening on has
    `ViewOut::sharpen == Some(s)` and pixels that differ from the same view made with sharpening
    off (`Fake`'s image, engine.rs about line 27, is a sawtooth, so no overshoot is asserted).
- **Known and accepted (W3-Q4; chain review):** turning sharpening on or off, or changing a
  value, renders the shown view again (about 0.3 s for 61 Mpix at fit, about 1.2 s with fine
  details on) and the prefetched neighbours after it; a view of the other sharpening may
  meanwhile be drawn. The cache holds copies of an image at other sharpening values until they are
  evicted by age. Sharpening a half-size view (about 15 Mpix for 61 Mpix, T315's timing) keeps the
  image's session lent a little longer, so a metadata write's release may wait that much more.
- **Not in this task:** the key, the menu item, the mark and the reset (T318); the settings
  (T317).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text is added). No
  new dependency. An extra code review follows (more than one crate).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W3-Q1, W3-Q4, W3-Q6); waits for T315 |
| 2026-11-24 | Blocked | Cold read: every `JobView` literal named (`STATISTICS_LEND`, the unreducible `half`, the test helpers); the unreducible half-size image sharpened and its view carrying `view.sharpen`; `remove_inflight`'s new parameter and its four callers (with `reject`); the gesture's half-size job now **sharpened** (an unsharpened one would be rendered twice at 50 % and make the mark flip), with `satisfied` accepting a half-size view of either sharpening during a gesture and the mark `Waiting` then (W3-Q1 clarified by the owner 2026-11-24, ADR 0027 §4.3); `satisfied`'s wording; `frame()`'s early returns and the order of the states, `sharpen_state`; `State::set_sharpening` for the clamping test; the malformed half-size text; the `ViewOut` test helpers; the engine test asserts the field and a difference, not overshoot; point 6's facts stated. Waits for T315 and the chain review |
| 2026-11-24 | Blocked | Chain review (whole chain, the first task's log holds the summary): a fine render with the new sharpening counts the old fine view of the same image as free in the budget check (else U with D on could pause fine details for good); `Waiting`, not `Off`, while a sharpened view is still drawn after turning off; `Paused` by `req`'s exact rule, tested at both sides of 1; the extra wait of a metadata write's release noted. Waits for T315 |
| 2026-11-24 | Blocked | T319 added before this task (T315's timing above 10 ms after the split; owner 2026-11-24, ADR 0027 §4.3); now waits for T319 |
| 2026-11-24 | Open | T319 Done; set Open |
| 2026-11-24 | Ready for review | Points 1–8 and every listed test done (13 in `state.rs`, 1 in `tests/engine.rs`); `.\tools\check.ps1` all OK, 2655 passed, 81 ignored, 0 failed. `satisfied` now takes the want (its two callers in `plan_of` pass it) so the gesture exception is limited to `Want::Shown`. Point 6's "evicted when the new one is accepted, also if it is protected then" needed a small change in `accept`: for a `Fine` result the image's fine views of another sharpening (new helper `replaced_fine`, also used by `pick`) are taken out of the protected set for the eviction loop only; what `accept` keeps is unchanged. In `pick` only the protected ones of those views are subtracted from the need (the unprotected ones are already evictable in `textures_fit`). `o_job` copies the `ViewOut`'s `sharpen` (None in every existing test) |
| 2026-11-24 | Open | Review (reviewer and an independent `/code-review`): points 1–8 and the tests are as written; sent back for three fixes, all in `state.rs`, each with a test in its tests. **(a)** `serving_of`: a fine view whose `sharpen` differs from `req.job.sharpen` is preferred to a non-fine view with the requirement's sharpening only while the image is not in `fine_failed`; otherwise, when the sharpened fine render fails, is rejected or is refused by the budget, the old fine view stays drawn for good and the mark stays `Waiting`. Test: fine details on, an unsharpened fine view drawn, sharpening on, the sharpened reduced view accepted, the sharpened fine render fails: the frame draws the sharpened reduced view and `sharpen` is `Shown`. **(b)** `accept`: the image's fine views of another sharpening are taken out of the protected set only when `out.sharpen == self.sharpening`; a late fine result of an old sharpening must not make room by evicting the wanted fine view. Test: a budget that holds one fine view, an unsharpened fine view drawn, sharpening turned on and off while the sharpened fine render runs, its result accepted: the unsharpened fine view is not evicted and stays drawn. **(c)** `sharpen_state`, the branch for sharpening off: `Waiting` only when the drawn view is of the current key and generation and sharpened; another image's sharpened view kept on screen while the next one is prepared gives `Off`. Test: sharpening on, image 10's sharpened view drawn, sharpening off, a step to an image with no view yet: `sharpen` is `Off`. Not taken up: a stale scaled or reduced result is still accepted (point 6, W3-Q4); the gesture's stand-in of either sharpening (point 4); the failed sharpening's text (W3-Q6). The step's shown time with a view of the other sharpening went to `known-limits.md` (Sharpening) |
| 2026-11-24 | Ready for review | The review's three fixes done in `state.rs`, each with its test: **(a)** `serving_of` puts the requirement's sharpening before the fine view once the image is in `fine_failed` (`when_the_sharpened_fine_render_fails_the_sharpened_reduced_view_is_drawn`); **(b)** `accept` takes the image's fine views of another sharpening out of the protected set only when `out.sharpen == self.sharpening` (`a_late_fine_result_of_an_old_sharpening_does_not_evict_the_wanted_fine_view`); **(c)** `sharpen_state` with sharpening off gives `Waiting` only for a sharpened view of the current key and generation (`another_images_sharpened_view_kept_on_screen_does_not_make_the_mark_wait`). Each of the three tests was checked to fail with its fix taken out. `docs/test-inventory.md` updated (pw-viewcore 225); `.\tools\check.ps1` all OK, 2658 passed, 81 ignored, 0 failed |
| 2026-11-24 | Done | Second review: the three fixes and their tests as asked; approved by the owner and committed. No hand check per task: W3's hand check follows T318. Leftovers: the step's shown time with a view of the other sharpening in `known-limits.md` (Sharpening), and for `latency.ps1` after T318 in W3's row of `plans/adr-0018-0025-steps.md`; the review's three findings not taken up are decided (point 4, point 6, W3-Q4, W3-Q6). No `CHANGELOG.md` line: sharpening cannot be turned on until T318 |
