# T321. ADR 0027 W3: sharpening at 100 % and above — Blocked: waits for T320

- **Why:** W3's hand check (owner, 2026-11-25; ADR 0027 A-Q8, W3-Q1 and W3-Q6 changed, §3.3 and
  §4.3, "W3's hand check"): sharpening applies at **every zoom level**, 100 % and above included,
  since the user often judges sharpness there and the key (and T323's button) make it a visible
  choice that is undone at once. The views above 100 % are the 100 % views drawn larger, so they
  are sharpened with them. The mark "USM" is never paused (never dimmed) for sharpening any more.
  Waits for T320 (both change `preview_text.rs`'s tests).
- **What the reviewer checked (2026-11-25):** in `crates/pw-viewcore/src/state.rs`:
  - `State::req`: the ring's job (`Want::Ring`, about line 1154) has `sharpen: None` with the
    comment "100 % and above is never sharpened (ADR 0027 A-Q8)"; the 100 % view's job (the branch
    `scale >= 1.0 - EPS`, `ViewKind::Zoom`, about line 1196) has `sharpen: None`. Every other job
    has `sharpen: self.sharpening`. The comment before the branches (about lines 1182–1185) says
    100 % and above never is sharpened.
  - `SharpenState::Paused` (about line 242, "the current scale is 100 % or more"), made in two
    places: `State::frame`'s branch for a failed image (about lines 2485–2490, `Paused` if
    `sharpen_paused(key)`, else `Waiting`) and `sharpen_state` (about line 2583). Its helper
    `sharpen_paused` (about line 2561) is used only there.
  - The cache and the plan need no change: the view's key and `satisfied` already compare the
    job's `sharpen` for every kind but the gesture's half-size view (W3-Q4), so a 100 % view or a
    ring of the other sharpening is replaced in one swap, as a scaled view is today.
  - The worker (`core.rs`, about line 116) sharpens any job whose `sharpen` is `Some`, after the
    region render and before the conversion to the display; nothing there depends on the kind.
    The doc comment of `ViewCore::set_sharpening` (about line 380) says "every view below 100 %".
  - Tests in `state.rs` that hold the old rule: `with_sharpening_on_every_view_below_100_percent_is_sharpened`
    (about line 6306; at 1.0 it expects `(ViewKind::Zoom, None)` and the ring `(ViewKind::Ring,
    None)`), and `at_100_percent_sharpening_is_paused_and_below_it_applies_again` (about line
    6503). `crates/pw-viewcore/tests/engine.rs` uses only `Shown` and `Off`.
  - `crates/pw-loupe/src/preview_text.rs`: `sharpen_mark` has two arms for
    `SharpenState::Paused` (about lines 330–341, dimmed, "Sharpening (U) is paused: it applies only
    below 100%."), and the mark's test has two cases for them (about lines 668–680). The doc
    comment of `adjustment_marks` says "dimmed while it is paused", which stays true for fine
    details (`FineState::Paused`). `loupe_text.rs` uses only `SharpenState::Off`.
  - `docs/glossary.md`, the row "Sharpening, Show Sharpening, USM": "applied to the preview below
    100%" and "dimmed while paused".
  - `crates/pw-loupe/src/shell_text.rs`: the Settings page Preview's note `SharpeningNote` (about
    line 377), "Used in the preview below 100% while sharpening is turned on. Not saved in the
    images.", and its test (about line 1301).
  - **Tests that hold the old rule through their fixtures** (cold read, 2026-11-25): the fixture
    `state()` is past the dwell, so at fit the plan also wants the current image's 100 % target and
    ring; after this task they are sharpened while sharpening is on, and the unsharpened ones the
    helpers accept no longer satisfy the plan. Affected: `with_full_size_views` (about line 6271,
    doc "unsharpened, as at 100 % always") used by
    `changing_a_value_plans_new_sharpened_views_and_the_same_value_plans_nothing` (about line
    6483); and `ready_for_fine` (about line 5868, doc "only the fine view is left") with
    `pick_fine`, used by `with_fine_details_on_a_tight_budget_does_not_refuse_the_sharpened_fine_view`
    (about 6549), `when_the_sharpened_fine_render_fails_the_sharpened_reduced_view_is_drawn`
    (about 6637) and `a_late_fine_result_of_an_old_sharpening_does_not_evict_the_wanted_fine_view`
    (about 6665): the sharpened 100 % render is planned before the fine one and takes the image's
    session.
- **Crates:** `pw-viewcore`, `pw-loupe` (one match, one text and their tests). **Files:**
  `crates/pw-viewcore/src/state.rs`, `crates/pw-viewcore/src/core.rs` (a doc comment),
  `crates/pw-loupe/src/preview_text.rs`, `crates/pw-loupe/src/shell_text.rs`, `docs/glossary.md`, `docs/test-inventory.md` (rule 12),
  and this task's own file (status and log).
- **What:**
  1. **The 100 % view and the ring are sharpened:** both jobs in `State::req` get
     `sharpen: self.sharpening`; the two comments say that every view is sharpened while
     sharpening is on (ADR 0027 A-Q8, W3-Q1, changed 2026-11-25), the gesture's stand-in rule
     (W3-Q1) unchanged.
  2. **No paused state for sharpening:** `SharpenState::Paused` and `sharpen_paused` are removed.
     A failed image with sharpening on gives `Waiting` (as it does below 100 % today);
     `sharpen_state` goes off, then waiting during a gesture, then shown, else waiting. The doc
     comments of `SharpenState`, `sharpen_state` ("then paused") and `ViewCore::set_sharpening`
     say so.
  3. **The shell:** `sharpen_mark` loses its two `Paused` arms; the mark is never dimmed for
     sharpening. The texts of the other states do not change.
  4. **The Settings page's note** `SharpeningNote`: "Used in the preview while sharpening is turned
     on. Not saved in the images."; its test checks the new text (asked for here).
  5. **The glossary row** says the unsharp mask is applied to the preview at every zoom level,
     and drops "dimmed while paused" (fine details' row is not touched).
- **Tests to add / change** (rule 5; the changes are asked for here):
  - `with_sharpening_on_every_view_below_100_percent_is_sharpened` is renamed
    `with_sharpening_on_every_view_is_sharpened`, and at 1.0 it expects `(ViewKind::Zoom, on)` and
    the ring `(ViewKind::Ring, on)`, in RAW and JPEG mode; with sharpening off both stay `None`
    (the test above it, which plans nothing sharpened with it off, keeps checking 1.0).
  - `at_100_percent_sharpening_is_paused_and_below_it_applies_again` is replaced by a test that
    at 1.0 and at 2.0 the shown job is the 100 % view with the sharpening, the frame's state is
    `Waiting` until that view is accepted and then `Shown`; a resident unsharpened 100 % view
    does not satisfy the requirement (the sharpened one is planned) but is drawn until it
    arrives; turning sharpening off at 1.0 draws a resident unsharpened 100 % view and gives
    `Off`.
  - A test that the neighbours' prefetched 100 % target (`Want::Target`) carries the sharpening
    while it is on.
  - A test that a failed image with sharpening on at 1.0 gives `SharpenState::Waiting`.
  - A test that an image whose fit scale is 100 % or more (for example 800×500 in the test view),
    at fit with sharpening on, gets its 100 % view sharpened (it was never sharpened before).
  - `preview_text.rs`: the two `Paused` cases are removed from the mark's test; no other case
    changes.
  - The helpers `with_full_size_views` and `ready_for_fine` (and `pick_fine` if needed) are
    changed so the 100 % target and the ring they make resident carry the sharpening the test uses
    (a parameter, or made after sharpening is turned on), and their doc comments say so; the four
    tests named above then keep checking what they check, with their assertions unchanged (asked
    for here, not a weakening, rule 5). If one of them still fails, or another test fails, stop and
    report it by name.
- **Not in this task:** which view serves a requirement between 50 % and 100 % (T322); the
  neighbours' prefetch there (T324); the button (T323). The pixels at the edge of a 100 % view or
  ring (about 3 pixels at radius 0.8) are sharpened with the edge pixels repeated, as a scaled
  view's are today: accepted, the reviewer adds it to `known-limits.md` at Done. Also accepted and
  noted there: `wants_a_full_size_view` uses `serving`, which ignores sharpening, so after U at
  100 % the statistics, clipping and focus jobs are not held back for the sharpened view (minor);
  and with fine details and sharpening on at fit, the fine render waits behind the sharpened 100 %
  target and ring. Between 50 % and 100 % a sharpened ring now satisfies a sharpened scaled
  requirement until T322. No
  `CHANGELOG.md` line from the agent (the reviewer changes the unreleased Sharpening line, "below
  100 %", at Done).
- **Rules:** `AGENTS.md`, `docs/tasks/README.md`, `docs/TESTING.md`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added after W3's hand check (owner, 2026-11-25); waits for T320 |
| 2026-11-25 | Blocked | Cold read (fresh agent): line numbers and every use of `Paused` checked. Added: `shell_text.rs` (`SharpeningNote` and its test); the four tests whose fixtures accept unsharpened 100 % views and rings, with their helpers to be changed (rule 5); the doc comment of `sharpen_state`; three accepted side effects for `known-limits.md` |
| 2026-11-25 | Blocked | Chain review (fresh agent, T320–T324): added the test of an image whose fit scale is 100 % or more. Between T321 and T322 a sharpened ring satisfies a sharpened scaled requirement (weaker sharpening at 51–99 %): T322 follows at once, no hand check, `main` fast-forward or release in between (`plans/`, W3's row) |
