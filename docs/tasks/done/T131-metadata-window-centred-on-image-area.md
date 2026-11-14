# T131. The metadata window opens in the middle of the image area — Done

- **Can start now.** Asked for by the owner on 2026-11-14. It may run beside T128, T129 and T130
  only if none of them is changing `crates/pw-loupe/src/main.rs` at the same time; otherwise after.
- **Crates:** `pw-loupe` only. **Files:** `crates/pw-loupe/src/main.rs` (`draw_metadata_window`,
  the main window's `ui`, and its `tests` module); `docs/test-inventory.md`.
- **Why:** The metadata window (`M`, ADR 0017 §2.3.1) is drawn with `egui::Window` without a
  position, so egui places it near the top-left of the program window, and the owner sees it open
  in the lower-left corner. The owner wants it to open in the middle of the image area, where the
  eye already is.
- **What:**
  1. **The image area** is the rectangle of the main window's `CentralPanel` (the grid or the
     loupe, between the columns, the toolbar and the status bar) in the frame the window is drawn.
     Keep that rectangle from the main window's `ui` (for example in a field set each frame) so
     `draw_metadata_window` can use it. When it is not known yet, use `ctx.content_rect()`.
  2. **Each time the window opens** (`open_metadata_window` makes a new `MetadataWindow`), its
     centre is placed on the centre of the image area. This also holds when the user moved the
     window the last time it was open: egui remembers a window's position by its id, so check that
     the remembered position does not win on the next opening.
  3. **While the window is open** the user can still move and resize it as today, and it stays
     where the user put it while going to the previous or next image, saving and the questions.
     The window does not follow the image area if a column is opened or closed meanwhile.
  4. **A window larger than the image area** (`WINDOW_SIZE` is 460 × 640) is centred on the
     image area's centre and kept inside the program window (egui's own constraint); its title bar
     is never above the program window's top edge.
  5. Put the placement in a small pure function (for example
     `metadata_window_position(image_area: egui::Rect, window_size: egui::Vec2,
     screen: egui::Rect) -> egui::Pos2`, the top-left corner) so it can be tested without a frame.
  6. Nothing else changes: the backdrop (D16 modal), the window's size, its contents, its keys and
     the settings and import windows stay as they are. No text the user sees is added or changed.
- **Not in this task:** remembering the window's position or size between openings or between
  runs (a setting); the settings and import windows' placement; the preview window.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add** (in `main.rs`'s `tests` module): the position for an image area that is larger
  than the window puts the window's centre on the area's centre; for an image area that is not at
  the program window's origin (a left column open) it is still the area's centre, not the screen's;
  for an image area lower than the window the top edge is not above the program window's top; the
  existing tests stay as they are and pass.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated, and
  `git diff --stat` shows only the files above. The owner checks by hand that `M` opens the window
  in the middle of the image area in the grid and in the loupe, with the left column open and
  closed, and again in the middle after moving it, closing it and pressing `M` once more.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Added at the owner's request |
| 2026-11-14 | Ready for review | `metadata_window_position` and one-frame `current_pos` on opening; 4 tests; check.ps1 all OK (1322 passed, 53 ignored) |
| 2026-11-14 | Ready for review | Reviewer: code and tests accepted, not committed. The position is set once with `current_pos` on the first frame after opening (so egui's remembered position does not win), from the central panel's rectangle of the same frame; the size the window had last time is used. Reviewer's `check.ps1` with T132: fmt, test and clippy OK, 1325 passed, 0 failed, 53 ignored. Waits for the owner's hand check |
| 2026-11-14 | Done | Reviewer's fixes after the owner's hand check (point 4 failed twice): `close_metadata_window` resets `metadata_window_shown` (`M` can reopen the window before a frame is drawn without it); and the remembered area position is replaced too, because egui 0.37 puts a window moved by its title bar back at its remembered position after `current_pos` (`Window::show_dyn`, `pivot_pos_before_begin`). `check.ps1` all OK (1325 passed, 53 ignored). Owner: hand check done and approved; committed by the reviewer |
