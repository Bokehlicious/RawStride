# T56. ADR 0014 F5b: draw the filter and the marking in the grid, wire clicks and keys — Done

- **Review 2026-11-10: accepted**, after the owner's hand check. The owner's decisions in the check:
  the marking count before the file name in the status line (ADR 0014 §5.3, amendment 1), and
  amendment 1 itself (§13), which becomes T59 and T60 and changes T57. The "Done when" check "under
  'Ej betygsatt' a rated image stays until moving on" no longer applies; T59 replaces it.

- **Note from the T53 review:** remove `#[cfg_attr(not(test), allow(dead_code))]` from
  `MetadataCache::set_ratings` when point 7 calls it.
- **Note from the T55 review:** the same for `MetadataCache::read_error` when the grid shows the
  read error (point 3).

- **Start after T53 and T55 are done.** **For a Claude agent** (drawing and wiring, checked by hand).
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/main.rs` only (a small plain helper with a test
  may go in the library).
- **Why:** ADR 0014 §3.1, §4.5, §5. Until T57, the loupe opens on the visible list with `open_folder`
  (ADR 0014 §9, way A).
- **What:**
  1. **Filter controls:** two `egui::ComboBox` in the tab bar row after "Importera…", listing
     `STAR_FILTERS` and `LABEL_FILTERS` with their texts. An active list has a yellow outline. A choice
     calls `Tabs::set_filter` and `refilter` (keep `None`, as `setFilter` in `app.js`); afterwards no
     widget keeps keyboard focus, so arrows and digits work at once.
  2. **When to refilter:** a filter change, a tab load, and when the active tab's ratings become ready
     (`ratings_ready` turns true). A rating key does **not** refilter (§2.2); moves do (T55).
  3. **The grid over `tab.visible`:** layout, rows, the visible range and scrolling use positions in
     `visible`; cells map to file indexes. Thumbnails: `to_request_indexes` for the file indexes of the
     visible positions; `set_visible` gets the range from the smallest to the largest of those file
     indexes (+1); `forget_not_in` with the file indexes of the keep range. With nothing visible, the
     grid shows `no_match_text` (or `READING_RATINGS_TEXT`, or the read error) in the middle.
  4. **Marked cells (§5.1):** blue outline `#4c8dff` and a faint blue fill behind the thumbnail; the
     current cell keeps its yellow outline (both when current and marked).
  5. **Clicks:** read Ctrl and Shift from the input; `Tab::click`. A double-click opens the loupe as
     today.
  6. **Keys in the grid:** `Navigate` and `Extend` → `grid_move`; `SelectAll` → `selection.all`;
     `ClearSelection` → `selection.clear`; scroll to the new current as today.
  7. **Rating keys:** targets from `selection.targets(visible, current, mode)`, then
     `metadata.set_ratings`; `Unsupported` gives `UNSUPPORTED_TEXT` as today.
  8. **Status line in the grid:** `position_text` (with `visible.len()` and `files.len()`),
     `marked_text`, the read notice; replaces today's "{name}  {n} / {total}".
  9. **Loupe (interim):** opening the loupe gives the core the paths of `visible` with `open_folder`
     and `go(position)`; where `main.rs` sets the tab's current from `frame.pos`, map the position
     through `visible`. With nothing visible, Enter/G/Space and a double-click do nothing.
- **Rules:** drawing reads `tab.visible`, `RatingStore` and `Selection` only, never files; no new
  dependencies; rules 6, 7, 10–12.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (ADR 0014 §8.2): each
  filter; an empty filter; two tabs with different filters; under "Ej betygsatt" a rated image stays
  until moving on; Shift+click, Shift+arrows, Ctrl+click, a label pressed twice on a marking; Ctrl+A;
  Escape; thumbnails appear at once when the filter changes (S1).
