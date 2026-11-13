# T29. ADR 0011 K4d: switch between grid and loupe, size control — Done

- **Start after T28 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/lib.rs` only
  if a small plain function is needed (with a test).
- **What:**
  1. **Grid → loupe:** Enter, G, Space (`GridKeyAction::OpenLoupe`) or a double-click on a cell:
     `set_mode(ViewMode::Loupe)`, then `core.open_folder(files)` and `core.go(current)` (through the
     metadata cache as `load_active_tab` does today), and `ThumbWorker::pause()`.
  2. **Loupe → grid:** Escape, G, Enter (`loupe_key_action`): `set_mode(ViewMode::Grid)`; the image
     shown in the loupe becomes the current cell (`tab.current` is already kept up to date by the
     loupe); `resume()`; scroll so that cell is visible. These three keys are checked **before**
     `key_action` in loupe mode (they are reserved there and do nothing today).
  3. **The view core only gets the folder in the loupe (ADR 0011 §4.2):** when a tab in `Grid` mode is
     loaded, call `core.open_folder(Vec::new())` instead of giving it the files, so the loupe does not
     decode in the background; a tab in `Loupe` mode is loaded as today. Switching tabs therefore
     shows each tab in its own mode (Q5).
  4. **Command line (Q1):** a folder opens in the grid; a single file opens the loupe on that file
     (set `ViewMode::Loupe` after opening when the path is a file).
  5. **Size:** a slider (96–480, `clamp_thumbnail_size`) in the status line while the grid is shown,
     and Ctrl + mouse wheel over the grid with `step_thumbnail_size` (egui turns Ctrl + wheel into
     `zoom_delta()`; use its direction, and make sure the scroll area does not scroll at the same
     time). The size is kept for the session in `Loupe`, shared by all tabs.
  6. The display-mode button of the loupe's status line is only shown in loupe mode.
- **Rules:** as T28; the loupe's own keys, mouse and zoom behaviour are unchanged in loupe mode.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (ADR 0011 §8.2): Enter
  and double-click open the loupe on the right image; Escape returns with the same image framed; two
  tabs keep their own mode; the slider and Ctrl + wheel change the size; `pw-loupe <file>` opens the
  loupe and `pw-loupe <folder>` the grid.
