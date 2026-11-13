# T55. ADR 0014 F5a: browse logic of a tab under a filter (plain Rust) — Done

- **Second review 2026-11-10: accepted.** The review point below is done.

- **Review 2026-11-10: changes requested (one point).** Accepted: `refilter`, `position`, `click`
  (a hidden index changes nothing), the moves doing nothing when nothing is visible, `Tab::open`
  showing every file, `to_request_indexes`/`forget_not_in`, `ratings_ready`/`read_error` (the
  temporary `allow(dead_code)` on `read_error` is removed in T56), the texts, and the tests. Still to
  do, in `crates/pw-loupe/src/browse.rs` only:
  1. **Refilter before the move too, as `go()` in `app.js` does** (you asked; ADR 0014 §4.2 says "as
     in `go()`"). `grid_move` and `loupe_move` must first `refilter` keeping the **current** image,
     then compute the move over that list, then `refilter` keeping the new current one, as now.
     Otherwise, after a batch rating, images other than the current one that stopped matching are
     still in `visible`, the move lands on one of them, and `keep` keeps it: under "Ej betygsatt",
     rating four marked images and pressing Right walks through the four rated images one by one.
     `click` stays as it is. Add a test: filter Unrated (visible [0, 4, 5, …, 11]), current 5, marked
     5–7, all three get stars; `grid_move(Right)` → current 8 and visible [0, 4, 8, 9, 10, 11]; the
     same case with `loupe_move(Next)`. Update the doc comments.
  Then run `.\tools\check.ps1`.

- **Start after T51 and T52 are done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/tabs.rs` (or a new `browse.rs` with an
  `impl Tab`, declared in `lib.rs`), `crates/pw-loupe/src/thumbs.rs`, `crates/pw-loupe/src/metadata.rs`,
  `crates/pw-loupe/src/lib.rs` (texts and re-exports). **Not** `main.rs`.
- **Why:** ADR 0014 §2.2–§2.4, §4.2, §5.3, Q5, Q7; GR11. Everything T56 needs to draw the grid under a
  filter, as tested plain Rust, so `main.rs` only calls it.
- **What:**
  1. `Tab` gets `pub visible: Vec<usize>` (file indexes shown, in order). It is set only by the
     methods below, never by drawing.
  2. `Tab::refilter(&mut self, ratings_ready: bool, rating_of: &dyn Fn(&Path) -> Option<Rating>, keep: Option<usize>)`:
     - without an active filter: all indexes;
     - with an active filter and `ratings_ready == false`: empty (Q7);
     - otherwise `filter::visible` with `keep`;
     - then `selection.retain_visible`, and if `current` is not visible, `current` =
       `nearest_visible` (unchanged when nothing is visible).
     Returns whether `visible` changed.
  3. Moves (each ends with `refilter` keeping the **new** current, §4.2):
     - `grid_move(&mut self, nav: Navigation, extend: bool, columns, visible_rows, …)`: the new
       position from `grid::navigate` over `visible.len()`; `extend` → `selection.extend_to`, otherwise
       `selection.click(new)` (plain moves clear the marking);
     - `click(&mut self, index, ctrl: bool, shift: bool, …)`: Shift → `extend_to`, Ctrl → `ctrl_click`,
       neither → `click`; then `current = index`;
     - `loupe_move(&mut self, to: LoupeMove, …)` with `LoupeMove::{Next, Previous, First, Last}` over
       the visible list; clears the marking (Q5).
     Each returns the new current file index.
  4. `Tab::position(&self) -> Option<usize>` (position of `current` in `visible`).
  5. `MetadataCache::ratings_ready(&self) -> bool`: false while the folder read is sent or waiting to
     be sent (`load_request` or `pending_read`), true otherwise; `read_error(&self) -> Option<String>`
     when the read failed (Q7: an active filter then shows nothing, and the status line gives the
     error). Treat a read error as `ratings_ready == false` in `refilter`'s caller (say so in a doc
     comment).
  6. `thumbs.rs`: `ThumbGrid::to_request_indexes(&self, indexes: &[usize], cell) -> Vec<usize>` and
     `forget_not_in(&mut self, keep: &[usize]) -> Vec<usize>`, with the same rules as `to_request` and
     `forget_outside`. The range methods stay.
  7. Texts (`lib.rs`, ADR 0014 §5.3): `position_text(pos: usize, shown: usize, total: usize) -> String`
     ("3 / 12 (av 31)", or "3 / 31" when `shown == total`); `no_match_text(total)` → "Inga bilder
     matchar filtret (31 i mappen)"; `marked_text(n) -> Option<String>` → "12 markerade" for n > 1;
     `pub const READING_RATINGS_TEXT: &str = "Läser betyg …";`.
- **Rules:** as T51.
- **Tests to add:** port these checks of `ui/tests/filter-select.test.js` with 12 files `IMG_000.ARW` …
  `IMG_011.ARW` and the ratings 1: 3 stars, 2: 5 stars Red, 3: rejected, 4: Red only (ADR 0014 S6):
  all 12 visible; ≥ 3 → [1, 2] and position text "1 / 2 (av 12)"; ≥ 3 and Red → [2]; Red → [2, 4];
  Purple → empty and the no-match text; Rejected → [3]; Unrated → [0, 4, 5, …, 11]; under Unrated, a
  rating of the current image keeps it visible, a move right drops it and the next one is current;
  click 5, Shift+click 8 → 4 marked; Ctrl+click 10 → 5; Escape (`selection.clear`) → none;
  Shift+Left twice → 3; `all` → 12; a plain Left clears. Also: an active filter before the ratings are
  read shows nothing, and after the read the matching images; the filter hides the current image →
  the nearest following one; a marked image that the filter hides is unmarked; `loupe_move` clears the
  marking; `to_request_indexes` and `forget_not_in`; the four texts.
- **Done when:** `.\tools\check.ps1` is all OK.
