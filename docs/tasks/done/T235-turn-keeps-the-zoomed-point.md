# T235. ADR 0024 O3a: a turn keeps the same image point in the centre (pw-viewcore) — Done

- **Why:** ADR 0024 §2.5, O-Q9 (accepted by the owner 2026-11-17): when an image is turned,
  "fit stays fit; at 100 % the same point of the image stays in the centre". Since T231,
  `State::set_orientation` changes the flip of the opened images of a path and keeps the zoom and
  the relative centre as they are. The centre is a relative point of the *upright* (turned)
  image (`geometry.rs` module comment, V6), so after a turn the same numbers name another point of
  the image, and the scale is not clamped again for the turned size (plan O3 row, need (c), from
  T231's review). This task makes the view follow the turn; O3b (T236) and O3c (T237) add the
  shell's keys and writes. It is the first task of the chain T235 → T236 → T237 (plan O3 row).
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/geometry.rs` (point 1),
  `crates/pw-viewcore/src/state.rs` (point 2 and tests), `crates/pw-viewcore/src/core.rs`
  (point 3, doc comment only), `docs/test-inventory.md`, and this task's own file (status and
  log).
- **What:**
  1. **`geometry.rs`:** two public functions beside `flip_of_orientation` and `turned`:
     - `quarter_turns_of_flip(flip: u8) -> u8`: the clockwise quarter turns of a LibRaw flip
       code as `flip_of_orientation` makes them: `0` → 0, `6` → 1, `3` → 2, `5` → 3; any other
       code → 0.
     - `turned_centre(centre: [f64; 2], quarter_turns: u8) -> [f64; 2]`: the relative point of
       the upright image after the image is turned `quarter_turns` (taken modulo 4) times 90°
       clockwise. One clockwise quarter turn maps `[x, y]` to `[1 - y, x]` (the top-left corner
       `[0, 0]` goes to the top-right `[1, 0]`); two map it to `[1 - x, 1 - y]`; three to
       `[y, 1 - x]`; zero leaves it.
  2. **`State::set_orientation`** (`state.rs`): before the flips change, note the flip of the
     current image (`cur_key()`, its `sizes` entry), if it is opened. After the change, if that
     flip changed and the zoom is `Zoom::Scale(s)`:
     - the centre becomes `turned_centre(drawn, (4 + new − old) % 4)` with the quarter turns of
       the two flips (`quarter_turns_of_flip`; written so the `u8` cannot underflow), clamped to
       0–1 as `set_view` clamps it. `drawn` is the point really in the middle of the view before
       the turn, `geometry::centre_of(old_size, geometry::visible(old_size, view_px, s, centre))`
       with `old_size` the size with the old flip, not the stored centre: `set_view` stores a centre without moving it
       where the view is drawn (state.rs:637–639), and near an edge the two differ. "The same
       point of the image stays in the centre" is the drawn point (ADR 0024 §2.5);
     - the scale is raised to `min_scale` of the turned size when it is below it (as `set_view`
       clamps a scale when the size is known), and stays at most `MAX_SCALE`.
     At `Zoom::Fit` nothing besides the flip changes (fit stays fit; the scale follows the turned
     size already, T231). An image that is not the current one, or a path whose current image is
     not opened yet, changes only its flip, as today. The rule applies to every flip change
     of the current image, not only a key's: an XMP value arriving with a folder read, a
     rollback after a failed write, and a value taken back (`None`) all keep the same image
     point in the centre, which is what the user should see. The return value stays "whether a flip
     changed". No gesture step is started (`gesture_at` is not set): a turn is not a zoom
     gesture.
  3. **The doc comments** of `ViewCore::set_orientation` (`core.rs`) and
     `State::set_orientation` (`state.rs:1352–1357`): each says that at a scale the
     centre is turned with the image so the same image point stays in the centre (ADR 0024 §2.5),
     instead of "the zoom and the relative centre are kept".
- **Tests to add:**
  - `geometry.rs`: `quarter_turns_of_flip` for 0, 3, 5, 6 and an unknown code;
    `turned_centre` for the four corners and `[0.25, 0.1]` with 0–4 quarter turns (4 = 0), and
    four single clockwise turns in a row give back the start point.
  - `state.rs` (beside the T231 orientation tests, with their helpers `state`, `opened`, `k`,
    `path`, `flip`):
    - The view is set up by assigning `s.zoom` and `s.centre` directly, as the T231 test does
      (state.rs:2902), not through `set_view`/`set_zoom` (which would move the centre). With the
      test's `SIZE` (4000×3000, flip 0) and `VIEW` (1000×800). Centres are compared with a
      tolerance of `1e-9`.
    - `a_turn_at_100_percent_keeps_the_same_image_point_in_the_centre`: current image opened,
      `Zoom::Scale(1.0)`, centre `[0.3, 0.4]` (inside the drawable range in both
      orientations, so drawn = stored); told 6 → the centre is `[0.6, 0.3]`; told 3 (from 6,
      one more clockwise) → `[0.7, 0.6]`; told `None` (from 3 back to the file's own 0) →
      `[0.3, 0.4]`.
    - `a_turn_near_an_edge_turns_the_drawn_centre`: `Zoom::Scale(1.0)`, stored centre
      `[0.25, 0.1]`; the drawn centre is `[0.25, 0.4/3]` (the view's half height is
      `800 / 3000 / 2`); told 6 → the centre is `[1 − 0.4/3, 0.25]`.
    - `a_turn_at_fit_stays_fit_and_keeps_the_centre`: at `Zoom::Fit` a told 6 leaves the zoom
      `Fit` and the centre unchanged.
    - `a_turn_of_an_image_that_is_not_current_keeps_the_view`: at a scale, a told value for
      another opened path changes neither zoom nor centre.
    - `a_turn_raises_a_scale_below_the_turned_minimum`: a tall view, `s.view_px = [800, 1000]`
      (with the default `VIEW` a turn of `SIZE` lowers the minimum): the minimum is 0.1 before
      and 0.125 after the turn (`min_scale` = `min(fit, 1) × 0.5`); `Zoom::Scale(0.1)`, told 6
      → `Zoom::Scale(0.125)`.
- **Not in this task:** the keys, the menu items, the writes and the store of the turns (T236,
  T237); the compare: its second core is a `ViewCore` of its own and gets the same rule for
  its own current image, but with the zoom locked (L) the shell copies the current core's zoom
  and centre to the held side (`main.rs`, around line 6544), so turning the current image also
  moves the held side's centre — left as it is, to be looked at in ADR 0024's hand check (T237); turning pixels or showing mirrored images mirrored (ADR 0024 §5).
- **Rules:** `docs/tasks/README.md` rules 1–12. No new dependencies. No `.unwrap()`,
  `.expect()` or unchecked indexing outside tests.
- **Done when:** `.\tools\check.ps1` is all OK and `git status` shows only the files above
  changed.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Added (chain T235 → T236 → T237, ADR 0024 O3) |
| 2026-11-20 | Open | Cold read: doable with the named files; flip directions (6 = 90° clockwise, 5 = 270°), the centre mapping and the helpers checked. Fixed: the drawn centre is turned, not the stored one (they differ near an edge), with an edge test; the 100 % test numbers moved inside the drawable range; the scale test given a tall view and its numbers (with the default view the minimum falls); `State::set_orientation`'s doc comment named too; no `u8` underflow; tests set `zoom`/`centre` directly; the locked compare's copy of the centre named under Not in this task and in T237's hand check |
| 2026-11-20 | Blocked | Waits for the review of the whole chain T235–T237 (`docs/tasks/README.md`, "A review of the whole chain") before it is set to Open |
| 2026-11-20 | Open | Chain review (one fresh agent, T235–T237 with ADR 0024 and the code): no contradiction between the briefs, every ADR 0024 requirement covered. Fixed: here the `centre_of` call's arguments, and that the rule holds for every flip change of the current image; in T236 a stopped reader is told apart from "no answer yet" and the reason `confirmed_by_write` must stay; in T237 a reader that cannot start or has stopped is answered with `pw_raw::inspect` on the UI thread (not 1, which would write the file's own value or jump 180°), `write_pending`'s doc comment, the two queues kept apart, nothing changed for the locked compare, and the hand check given a DNG/TIFF and a RAW+JPEG pair. Backlog: an outside change of IFD0 Orientation not seen during the session, a group move not waiting for the partner's turn, waiting turns dropped at exit. Set to Open |
| 2026-11-20 | Ready for review | Done. `geometry.rs`: `quarter_turns_of_flip` and `turned_centre`, 3 tests. `state.rs`: `set_orientation` notes the current image's key and size before the flips change and calls a new private `turn_view_with_image`, which at `Zoom::Scale` turns the drawn centre (`centre_of(old_size, visible(...))`) by `(4 + new − old) % 4` quarter turns, clamps it to 0–1 and raises the scale to the turned size's `min_scale` (at most `MAX_SCALE`); at fit, for another image or an image not opened yet, only the flip changes; `gesture_at` is not set; doc comment changed; 5 tests. `core.rs`: doc comment only. `docs/test-inventory.md`: pw-viewcore row 155 → 163 and its description, header 2016 passed / 75 ignored (the owner's uncommitted T234 changes to `level_c.rs` and the inventory were in the working tree and are kept). Note: near an edge the turned centre can lie outside the turned image's drawable range (the edge test: x = 1 − 0.4/3 against a drawable maximum of 5/6); it is stored as it is, like `set_view` stores it, and `visible` keeps the drawn region inside the image. check.ps1 all OK, no clippy warnings |
| 2026-11-20 | Done | Reviewed: the diff holds only the named files and matches points 1–3 and the tests; the quarter turns checked against `ImageSize::to_sensor` (flip 6 → `[1 − y, x]`, 5 → three turns), the difference of two flips as the turn, no `u8` underflow, `gesture_at` not set. The agent's `test-output.txt` mixed two `check.ps1` runs (one Summary said fmt FAILED); the reviewer's own clean run: `.\tools\check.ps1` all OK, 2016 passed, 75 ignored, no clippy warnings. Noted for T237's hand check: near an edge a turn and a turn back do not return exactly to the start (the drawn point is clamped in the turned orientation; in the edge test y goes from 0.4/3 to 1/6, 100 pixels at 100 %), which follows from turning the drawn point (ADR 0024 §2.5) |
