# T283. A larger Settings window, in the middle of the program's window — Done

- **Why:** ADR 0017 U8-Q14 (owner, 2026-11-22, at the backlog sort after U8's hand check): the page
  Keyboard Shortcuts feels cramped. The Settings window has a fixed width of 440 points, and the
  scroll areas of the pages Metadata Window and Keyboard Shortcuts are at most 420 points high
  (`LAYOUT_PAGE_MAX_HEIGHT`, never more than half the program's window). The owner chose: the
  window opens **in the middle** of the program's window, **larger** (about 640 × 560 points,
  smaller when the program's window is small), and **can be resized** by dragging its edge; the
  size is **not saved** (each opening starts at the default size, in the middle).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/shell.rs`
  (only the size function and its test), `crates/pw-loupe/src/lib.rs` (only its re-export),
  `docs/test-inventory.md`.
- **Background (read before starting):** `draw_settings_window` in `main.rs` (about lines
  1213–1458: `SETTINGS_WINDOW_WIDTH`, `LAYOUT_PAGE_MAX_HEIGHT`, `layout_height`, the four pages,
  the scroll areas of Metadata Window and Keyboard Shortcuts with their own `id_salt`, "Restore
  Defaults" under each, Save and Cancel under every page); `open_settings_window` (about line 1024;
  the window opens from Settings… on General and from "Open in Another Program…" on External
  Programs); the metadata window (about lines 4093–4140) for its buttons in an
  `egui::Panel::bottom` inside the window. In egui 0.37 a resizable window's `Resize` grows to at
  least the last frame's content size (`resize.rs`), so content must never be taller than the room
  left, and its maximum size is capped at the program's window (`window.rs`, the constrain rect),
  applied after the minimum. The key handling of the page Keyboard Shortcuts (T281:
  `take_key_for_change` at the top of `ui()`, the key loop skipped while `settings_window` is
  `Some`, `wait_hovered`) does not depend on the window's id, size or layer and stays as it is.
- **What:**
  1. **The size** (a pure function in `shell.rs`, for example `settings_window_size(screen: Vec2)
     -> Vec2`, re-exported from `lib.rs`): the program window's size (`ctx.content_rect().size()`)
     minus 2 × 24 points on each side, clamped between a minimum of 440 × 320 and the default
     640 × 560, and never larger than the program window itself: `(screen − 48).clamp(MIN,
     DEFAULT).min(screen)`, each side apart. **Fitting wins** over the minimum in a program window
     smaller than 440 × 320. The sizes are the window's outer size, title bar included, as egui
     counts them. The window gets `resizable(true)`, `min_size(MIN)` and that size as its default;
     `default_width`, `max_width`, `SETTINGS_WINDOW_WIDTH`, `LAYOUT_PAGE_MAX_HEIGHT` and
     `layout_height` are removed.
  2. **The place, not saved** (U8-Q14): each opening gets a new `egui::Id` (a counter field of
     `Loupe`, increased in `open_settings_window`, set in the constructor in `run()`), so egui
     remembers nothing of the last opening; with a fresh id, `default_pos` at the program window's
     centre minus half the size (or `pivot(CENTER_CENTER)` with `default_pos` at the centre) is
     enough; the metadata window's `current_pos` work-around is not needed. The next opening, also
     in the same session, is back at the default size in the middle. Each opening leaves a small
     entry in egui's memory; accepted. The window keeps egui's default order (not
     `Order::Foreground`, no backdrop) and can still be moved by its title bar; it is not anchored.
  3. **The pages fill the window:** Save and Cancel, and on the pages Metadata Window and Keyboard
     Shortcuts "Restore Defaults" above them, go in an `egui::Panel::bottom` inside the window, as
     in the metadata window, so they stay at the bottom. Above it, the page's content (after the
     page buttons and the separator, which stay at the top) is one `ScrollArea::vertical()` with
     `auto_shrink([false, false])` and an `id_salt` of its own per page, on **every page**
     (General and External Programs too, so a small window or five programs scroll instead of
     pushing Save away; no page gets new content). A taller window shows more rows; a long program
     path still wraps, now at the window's current width; the rows of Keyboard Shortcuts wrap less
     at the larger width.
  4. Nothing else changes: the pages, their order and texts, Save, Cancel, the key handling of the
     page Keyboard Shortcuts and what is saved. The About window keeps its own width.
- **Tests to add:** the size function: a large program window gives 640 × 560; a mid-sized one
  gives each side less by 48; one that leaves exactly 440 × 320 gives 440 × 320; one between 440 ×
  320 and 488 × 368 gives the minimum; one smaller than 440 × 320 gives the program window's own
  size. Say in the report what was only checked by hand (the place, the resizing, the bottom panel,
  the scroll areas filling it, the next opening back at the default).
- **Hand check (the owner, after Done):** Settings… opens in the middle of the program's window,
  larger than before, on General; the window can be made larger and smaller by its edge, not
  smaller than its minimum; on Keyboard Shortcuts and Metadata Window a taller window shows more
  rows, the key rows wrap less, and Save and Cancel stay at the bottom; a long program path on
  External Programs wraps; closed and opened again (Save or Cancel), it is back in the middle at its
  default size; "Open in Another Program…" with no programs opens it on External Programs, also in
  the middle; in a small program window the window still fits and every page scrolls with Save and
  Cancel visible; the window can still be moved by its title bar; a Change on Keyboard Shortcuts
  still takes the next key.
- **Not in this task:** saving the size or place (U8-Q14: not saved); the About window, the
  metadata window and the dialogs; any new setting.
- **Rules:** `docs/tasks/README.md`; no new dependency; no panic paths (`AGENTS.md` §3).
- **Done when:** `.\tools\check.ps1` is all OK, no clippy warnings, and `git diff --stat` shows
  only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Open | Written by the reviewer at the backlog sort after ADR 0017 U8, from the owner's wish at U8's hand check and the owner's choice (U8-Q14: in the middle, larger, resizable, not saved). Cold read: `lib.rs` added for the re-export; the size as `(screen − 48).clamp(MIN, DEFAULT).min(screen)`, fitting winning over the minimum (egui caps the size at the program window after the minimum); egui's `Resize` grows to its content, so the buttons in a bottom panel and every page in one scroll area with `auto_shrink([false, false])`, General and External Programs included; the width settings and `LAYOUT_PAGE_MAX_HEIGHT` removed; a new id per opening, so `default_pos` is enough; the default order kept; the T281 key handling unaffected; the size tests extended with the boundaries and a too-small program window |
| 2026-11-22 | Ready for review | Done by the agent: `settings_window_size` and its three constants (`SETTINGS_WINDOW_MIN`, `SETTINGS_WINDOW_DEFAULT`, `SETTINGS_WINDOW_MARGIN`) in `shell.rs`, the function and `SETTINGS_WINDOW_MIN` re-exported from `lib.rs`; `main.rs`: a `settings_opening` counter in `Loupe` (increased in `open_settings_window`) as part of the window's id, `resizable(true)`, `min_size`, `default_size`, `default_pos` at the centre less half the size; Restore Defaults, Save and Cancel in `egui::Panel::bottom("settings-window-buttons")`; every page in one `ScrollArea::vertical()` with `auto_shrink([false, false])` and its own `id_salt`; the width constants, `LAYOUT_PAGE_MAX_HEIGHT` and `layout_height` removed. 5 tests; test inventory pw-loupe 1300 -> 1305, totals 2373 passed, 78 ignored. check.ps1 all OK, no clippy warnings |
| 2026-11-22 | Done | Reviewed: no bugs; the code follows points 1–4. `settings_window_size` is `(screen − 48).clamp(MIN, DEFAULT).min(screen)` each side apart, the tests cover every boundary of the brief; checked against egui 0.37 (`window.rs`): `min_size`, `default_size` and the constrain cap are the outer size, title bar included, the cap applied after the minimum, so fitting wins; `Id::unique` (not the deprecated `Id::new`) with the opening counter (`wrapping_add`, no panic path), `default_pos` at the centre less half the outer size; Restore Defaults, Save and Cancel in a frameless bottom panel as in the metadata window; one scroll area per page with `auto_shrink([false, false])` and its own `id_salt`; the T281 key handling untouched; the test inventory row and totals match. Only the files of the brief changed. check.ps1 all OK (2373 passed, 78 ignored), no clippy warnings (searched). Not checked by test: the place, the resizing, the bottom panel, the scroll areas and the next opening; the owner's hand check covers them |
| 2026-11-22 | Done | Hand-checked by the owner: every point passed (Settings… opens in the middle, larger, on General; resized by its edge, not below its minimum; on Keyboard Shortcuts and Metadata Window a taller window shows more rows, the key rows wrap less, Restore Defaults, Save and Cancel stay at the bottom; a long program path wraps; a Change still takes the next key; after Save and after Cancel the next opening is back in the middle at its default size; "Open in Another Program…" with no programs opens External Programs in the middle; in a small program window the window fits, every page scrolls and Save and Cancel stay visible; moved by its title bar) |
