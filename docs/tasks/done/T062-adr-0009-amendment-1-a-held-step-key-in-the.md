# T62. ADR 0009 amendment 1: a held step key in the loupe waits for the shown image — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Can start now.** Small; for a Claude agent or a local agent.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/lib.rs` (a plain function and its tests),
  `crates/pw-loupe/src/main.rs`.
- **Why:** ADR 0009 §13 (amendment 1, owner 2026-11-10), found in the F7 check of ADR 0014: in RAW
  mode a held arrow runs to the end of the list and only the last image is shown.
- **What:**
  1. `lib.rs`: `pub fn step_allowed(action: Action, repeat: bool, status: &Status) -> bool`: false
     only when `repeat` is true, `action` is `Action::Next` or `Action::Previous`, and `status` is
     `Status::Preparing { .. }`; true otherwise (`Ready`, `Error`, `Empty`, every other action, and
     every press that is not a repeat).
  2. `main.rs`: keep egui's `repeat` flag of each key event in the collected key actions (today
     `Event::Key { key, pressed: true, modifiers, .. }`). Remember whether the loupe's last frame was
     shown (the `Status` of the last `core.frame`, for example as a field updated where the frame is
     taken). In the loupe, call `act` for a step only when `step_allowed` says so; an ignored repeat
     does nothing else. The grid, the dialog keys, the rating keys and the tab keys are unchanged.
- **Rules:** no `unwrap`/`expect`/`panic!` in non-test code; rules 6, 7, 10–12. No change of
  `pw-viewcore`.
- **Tests to add:** `step_allowed` for Next and Previous with and without `repeat` against `Ready`,
  `Preparing`, `Error` and `Empty`; a repeat of `First`, `Last`, `ToggleZoom` and `ZoomStep` is always
  allowed.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: in RAW mode, holding an
  arrow shows every image passed (stepping follows the rendering) and stops at once when the key is
  released; in JPEG mode a held arrow moves as before; single quick presses move at once in both
  modes; a damaged file (error message) does not stop a held arrow; the grid's arrows are unchanged.
