# T52. ADR 0014 F2: multi-selection model, Shift in the grid key map (plain Rust) — Done

- **Second review 2026-11-10: accepted.** The review points below are fixed.

- **Review 2026-11-10: changes requested (two points).** The key map, the `Tab` field,
  `active_mut`, the `main.rs` call and the tests are accepted. The change to the existing key test
  (Escape now gives `ClearSelection`) is correct: point 2 of this task gives Escape that meaning, and
  the task's "existing key tests keep passing" was wrong for that one line. The two `ratings.rs` test
  calls outside the file list are accepted (the new argument made them necessary). Still to do, in
  `crates/pw-loupe/src/selection.rs` only:
  1. **`extend_to` must never mark a hidden image** (ADR 0014 S5). When `index` is not in `visible`,
     it marks `index` today. Change it: if `index` is not visible, leave the marking and the anchor
     unchanged. If only the anchor is not visible, mark `index` alone, as today. Update the doc comment
     and add a test that an `index` outside `visible` changes nothing.
  2. **`all` must replace the marking**, as `pickAll` in `app.js` does (`clearPicked`, then every
     visible image), not add to it. Today the result is the same only while every marked image is
     still visible. Add a test: mark 2 and 3, then `all(&[0, 1])` → exactly [0, 1].
  Then run `.\tools\check.ps1`.

- **Can start now.** Independent of T51, T53 and T54. If T51 is done first, keep its `Tab` field.
- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/src/selection.rs` (declared and re-exported
  in `lib.rs`), `crates/pw-loupe/src/grid.rs`, `crates/pw-loupe/src/tabs.rs`, and
  `crates/pw-loupe/src/main.rs` **only** for the changed `grid_key_action` call (new arms do nothing
  yet; T56 wires them).
- **Why:** ADR 0014 §2.5, §2.6, §2.8, §4.5, Q5. The marking rules of `app.js` (`clickCell`,
  `pickRange`, `pickAll`, `clearPicked`, `go`, `ratingTargets`) as tested plain Rust.
- **What:**
  1. `pub struct Selection` with the marked file indexes and an anchor (`Option<usize>`). All
     positions are taken from a `visible: &[usize]` argument (file indexes in display order).
     Methods:
     - `click(&mut self, index)`: clears the marking; anchor = `index`.
     - `ctrl_click(&mut self, current, index)`: if nothing is marked, mark `current` first; then if
       `index` is marked **and** more than one is marked, unmark it, otherwise mark it; anchor = `index`.
     - `extend_to(&mut self, visible, current, index)`: if nothing is marked, anchor = `current`; then
       the marking is replaced by every visible index between the anchor and `index` (inclusive, by
       position in `visible`). If the anchor is not visible, the range is `index` alone. The anchor
       does not change. (Shift+click and Shift+navigation.)
     - `all(&mut self, visible)`: marks every visible index (anchor unchanged).
     - `clear(&mut self)`: unmarks all (anchor unchanged).
     - `retain_visible(&mut self, visible)`: unmarks indexes not in `visible` (§2.8).
     - `count()`, `is_marked(index)`, and `marked_in(&self, visible) -> Vec<usize>` (marked, in
       display order).
     - `targets(&self, visible, current, mode: ViewMode) -> Vec<usize>`: in `Grid`, `marked_in(visible)`
       if more than one is marked and `current` is marked; otherwise (and always in `Loupe`)
       `vec![current]` (§2.6).
  2. `grid.rs`: `grid_key_action(key, ctrl, shift, alt)` (new `shift` argument). New
     `GridKeyAction` variants: `Extend(Navigation)` for Shift + any navigation key, `SelectAll` for
     Ctrl+A (no Shift, no Alt), `ClearSelection` for Escape (no modifier). Shift with Enter, G or Space
     → `None`; every other Ctrl or Alt combination → `None` as today. `loupe_key_action` and the
     loupe's `key_action` are unchanged (Ctrl+A stays `None` there).
  3. `tabs.rs`: `Tab` gets `pub selection: Selection`; `Tab::open` clears it (anchor `None`).
     `Tabs::active_mut(&mut self) -> Option<&mut Tab>`.
  4. `main.rs`: pass `shift` to `grid_key_action`; the new variants do nothing yet.
- **Rules:** as T51.
- **Tests to add:** the `app.js` cases with 12 images, all visible: click 5, Shift+click 8 → 4
  marked; Ctrl+click 10 → 5 marked; Ctrl+click on the only marked image keeps it; Ctrl+click with
  nothing marked marks current and index; Shift range backwards; Shift from an anchor that is no
  longer visible; `all` then `clear`; `retain_visible`; `targets` in grid with current marked, with
  current not marked, with one marked, and in the loupe; `extend_to` twice from the same anchor
  replaces the range. Key map: every navigation key with Shift → `Extend`, Ctrl+A → `SelectAll`,
  Ctrl+Shift+A and Alt+A → `None`, Escape → `ClearSelection`, Shift+Enter → `None`; the existing key
  tests keep passing with `shift = false`.
- **Done when:** `.\tools\check.ps1` is all OK.
