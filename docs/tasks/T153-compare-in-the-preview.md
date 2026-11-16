# T153. ADR 0017 U6, fourth part: two images in the preview — Open

- **Can start now.** T152 (the second view core `second_core` with `second_textures` and
  `second_clip_overlay`, the image check releasing both, `draw_image_area`), T151 and T150 are
  Done. The owner's answers to U6-Q1–U6-Q12 are in
  `docs/tasks/done/T150-locked-view-in-the-view-core.md` and ADR 0017 §6.1. Chain: T150 → T151 →
  T152 → T153 → T154.
- **Why:** ADR 0017 §2.2 and step U6: the preview shows the held image beside (or above) the
  current one, so two images can be judged together. T152 started the second view core and made
  the image area one function; this task opens and closes the compare, draws the two sides and
  wires the keys and menu items of opening, holding, swapping and layout, with T151's rules. The
  zoom and position are **free** in this task (each side its own); T154 locks them.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/lib.rs`
  (the keys C, K, Shift+K and V in `preview_key_action`, and the test helper `command_of_key`
  taught them), `crates/pw-loupe/src/shell.rs` (four commands with their keys in the View menu,
  and its menu-order test extended with them), `crates/pw-loupe/src/shell_text.rs` (their texts);
  `docs/glossary.md` (the menu items); `docs/test-inventory.md`.
- **What:**
  1. **Opening** (U6-Q4), with **C** in the preview and the View menu item "Compare": T151's
     `open_indexes` on the stepped list; the held image's path given to the second core
     (`set_files(vec![held], 0)`); the current image moved with `LoupeMove::To(index)` as a
     filmstrip click moves it (ADR 0014 §4.4). With fewer than two images in the stepped list the
     key does nothing and the menu item is greyed. The compare remembers the active tab's index
     and folder it was opened in.
  2. **The held side's view and mode:** when the compare opens and whenever the held image
     changes (K, Shift+K), the second core is given the current side's zoom and centre once
     (T150's `set_view`, from the current frame's `zoom` and `centre`), so the held image starts
     as the current one is shown. The second core's display mode is set to the first core's
     (U6-Q9) when the compare opens and whenever `R` changes it.
  3. **Drawing** (U6-Q5, U6-Q10): the image area split by T151's `sides` with the remembered
     layout; T152's function called once per side, each with its own core, `Textures`,
     `OverlayCache`, half and `interact` id. On each side, at its top, T151's
     `compare_side_text` of that side's frame (from `loupe_text.rs`; no text in the drawing
     code). The current side gets a frame in the accent colour. The title line, the right column
     (Info, Zoom, Histogram, Clipping), the thumbnail gate, the inspector's statistics and the
     status line come from the current side's frame only, as today. While comparing, T148's wish
     for a clipping map is given to both cores (U6-Q9: the marks on both sides); otherwise the
     second core's wish is off. Outside the compare everything drawn is as after T152.
  4. **The mouse** (U6-Q8, free): a drag, the wheel and a double-click act on the side they are
     made on, with that side's core and its own anchor; on the current side exactly as today.
  5. **The keys** (U6-Q6, U6-Q7, U6-Q11), each with exactly these modifiers and only in the
     preview; each also a View menu item with its key shown, placed after "Show Deep Shadows"
     (Shift+J) and before the separator, in this order: Compare, Hold Current Image, Swap Images,
     Above and Below; the last three greyed outside the compare:
     - **C** "Compare": opens (point 1) or closes the compare;
     - **K** "Hold Current Image": T151's hold; the second core gets the new held path;
     - **Shift+K** "Swap Images": T151's swap; the current image moves with `LoupeMove::To` to
       the old held image's index and the second core gets the old current path; nothing when
       T151 says `None`;
     - **V** "Above and Below": the layout switched and remembered (`compare_above_below`),
       ticked when on.
     The steps, zoom keys, `R`, `M`, the rating and label keys act on the current image as today.
  6. **Closing** (U6-Q11): **C** again, or **Escape** while comparing, closes the compare and the
     current image stays in the preview; Escape is taken before `loupe_key_action` (`grid.rs`)
     only while a compare is open, so without one Escape, G and Enter close the preview as
     today. **G** and **Enter** close the preview and the compare with it. Closing the compare
     gives the second core an empty list and releases its clipping marks (as the contact sheet
     does). It also closes by itself, checked **every frame** (not at each entry point): when the
     preview is no longer shown, when the active tab's index or folder differs from those it was
     opened in, or when T151's "still valid" is false for the tab's files.
     **The second core's textures (from T152's review):** `second_textures.sync` runs today only
     in the preview's frame, not on the contact sheet's path (which returns early, as for the first
     core). When the compare closes because the preview closed, the emptied second core's dropped
     textures must still be taken back (`take_dropped`) and its views drained: sync
     `second_textures` on the contact sheet's path too (with an empty list it does nothing).
  7. **The filmstrip** (U6-Q10): the held image's cell, when it is in the strip, gets a second,
     dashed frame beside the current and marked frames.
  8. **Texts** (U6-Q12, rule 13): "Compare", "Hold Current Image", "Swap Images", "Above and
     Below" in `shell_text.rs`; the terms in `docs/glossary.md`.
- **Tests to add:** `lib.rs`: C, K, Shift+K and V map to their preview keys and not with other
  modifiers; K does not fire on Shift+K; the existing preview keys are unchanged; `command_of_key`
  knows the four, so `every_menu_key_gives_its_items_command` covers them. `shell.rs`: the four
  commands follow Show Deep Shadows in the View menu with their keys (the existing menu-order test
  extended, as T148 did for J and Shift+J). The drawing is checked by the owner's hand check after
  T154.
- **Not in this task:** the lock and the L key (T154); a second preview window (U0, §7.2); compare
  in the contact sheet.
- **Known limits (accepted with U6-Q2):** when the compare opens, the held image (already decoded
  in the first core) is decoded again in the second, so the held side appears after its own
  decode; while comparing, the second core's renders and its prefetch of the other view and the
  ring share the computer with the first core's steps. `latency.ps1` measures with the compare
  closed; the owner's hand check judges stepping while comparing.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency. Run
  `D:\PhotoWorkflowData\pm-study\tools\latency.ps1` on a release build before and after (with the
  compare closed) and report V1; it must not be worse. It changes the shell's frame in many
  places, so it gets an independent review of the diff before the owner is asked to approve
  (README, "An extra code review").
- **Done when:** `.\tools\check.ps1` is all OK, V1 is not worse, and `git diff --stat` shows only
  the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Blocked | Added with T150, T151, T154 (the chain of U6) as the first T152; after the cold read split, the second core and the image area went to T152 and this became T153 (menu position, Escape before `loupe_key_action`, closing checked every frame, the held side's starting view, `command_of_key` and the menu test named); waits for T152 |
| 2026-11-16 | Open | T152 Done; point 6 got the second core's textures on the contact sheet's path (from T152's review) |
