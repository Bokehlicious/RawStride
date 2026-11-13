# T111. ADR 0017 U3: the preview as a full-screen mode, its right column and the filmstrip — Done

- **Can start after T110 is Done** (it reuses T110's Info section and changes the same files). The
  owner approved the proposals of U3-Q1–U3-Q6 (at the end of this task) on 2026-11-13; they are
  written into the points below. Not at the same time as any other task that changes `main.rs`.
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/lib.rs` (the preview's keys,
  point 9, and the doc comment of `key_action`); `crates/pw-loupe/src/browse.rs` (the marking kept
  on a step and the steps between the marked images, point 7); `crates/pw-loupe/src/tabs.rs` (only
  if a move to a given position is needed for a click in the filmstrip); new
  `crates/pw-loupe/src/preview.rs` (the preview's layout state, the filmstrip's range, scroll target
  and hit test, the thumbnail gate of point 6; pure functions); new
  `crates/pw-loupe/src/preview_text.rs` (every new text of this task, rule 13);
  `crates/pw-loupe/src/inspector.rs` (only to let the preview's Info use the same reader and rows as
  the main window's); `crates/pw-loupe/src/shell.rs` and `crates/pw-loupe/src/shell_text.rs` (the
  Visa item "Förhandsvisning", point 1); `crates/pw-loupe/src/settings.rs` (the new keys, point
  10); `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.2 (the preview, current image and selection, the filmstrip), §2.4 (the keys
  of stage A), §5 (the filmstrip must not slow stepping) and §7 U3, the last step of stage A before
  the owner uses it on real work (§7.1). **New function:** the full-screen mode, the preview's right
  column with Zoom, and the filmstrip. Everything else is today's loupe moved.
- **Not in this task:** a separate preview window and a second screen (U0, §7.2); two-image compare
  (U6); histogram and clipping (U7); the metadata section in the preview (the ADR's preview column
  has Info and Zoom only); sort order (U14); previous/next in the `M` window (U12).
- **What:**
  1. **Open and close** (§2.2, §2.4; keys of today): from the contact sheet Enter, Space, G and a
     double-click open the preview on the current image; Escape, G and Enter close it, back to the
     contact sheet in the same tab with the last shown image as the current cell, scrolled into
     view. Today's switch between contact sheet and loupe (`switch_mode`) is the one place that
     does it. The Visa item "Kontaktark ↔ lupp (G)" is renamed "Förhandsvisning (G)" (`shell_text`;
     the menu test of T109 still passes). Started with a file path, RawStride opens in the preview
     as it opens in the loupe today (U3-Q6).
  2. **Full screen with only the preview** (U3-Q1): opening the preview puts the main window in
     full screen (`egui::ViewportCommand::Fullscreen(true)`) and shows only the preview: no menu
     bar, command bar, folder tabs, status bar or side columns. Closing it returns the window to
     its state from before (full screen if F11 had made it so, else maximized or its size and
     position). That earlier state, not the preview's full screen, is what `settings.txt` gets if
     the program is closed in the preview. F11 does nothing in the preview.
     The preview, top to bottom: a **title line** (one row in the status bar's font and theme
     colours: today's loupe status line with position, name, source, message, notice and the
     hidden import's status, and the RAW/JPEG control, G1, G4), then the image area | the right
     column, then the filmstrip. The image area is today's loupe unchanged (V1–V8, G1–G4; zoom and
     position persist, V6), and its rating badge is always drawn (T110).
  3. **The preview's right column** (§2.2, U3-Q2): its own column, resizable as T109's columns
     (160 points to 40 % of the width, default 300) and collapsible, **open by default** (the main
     window's right column still starts collapsed, owner 2026-11-13; the two are remembered apart).
     Two collapsible sections, both open by default:
     - **Info:** the same Exif rows as T110's Info, from the same reader with the same rules (a read
       only while this column and section are open, after 200 ms, one in flight, stale results
       dropped), for the shown image.
     - **Zoom** (U3-Q5): the zoom of the shown image as a text (`Anpassad (34 %)`, `100 %`,
       `200 %`), and buttons Anpassa (F), Anpassa ↔ 100 % (Z), Zooma in (+), Zooma ut (−) and a
       check box Zooma vid pekaren (A), each calling **the same function as its key** (no second
       copy of the logic). After a click no widget keeps the keyboard (as T109's command bar).
  4. **The filmstrip** (§2.2, U3-Q3): one row across the whole width under the image area and the
     right column; each cell the image's thumbnail (96 points high, aspect kept), the label bar of
     ADR 0013, the current cell framed in the accent and the marked cells marked as in the contact
     sheet. It holds the tab's visible list in the contact sheet's order and filter, or only the
     marked images (point 7). It is scrolled so the current image is in view after every step;
     the mouse wheel scrolls it sideways; a click on a cell shows that image (the same path as a
     step, so the rules of point 7 apply); a click never changes the marking.
  5. **Thumbnails of the filmstrip** come from the same thumbnail worker, `ThumbGrid` and texture
     cache as the contact sheet (no second cache), for the cells in view plus a margin; cells
     without a thumbnail yet are drawn as the contact sheet draws them.
  6. **Priority below the shown image** (§5, U3-Q4): today the thumbnail worker is paused while
     the loupe is shown (`switch_mode`). In the preview it runs **only while the shown image is
     drawn** (core status `Ready` or `Error`); a step pauses it until the new image is drawn. The
     gate is a pure function in `preview.rs` (status in, run or pause out), tested without a
     worker. With the filmstrip hidden no filmstrip thumbnails are asked for.
  7. **Current image and selection** (§2.2, owner 2026-11-13, review finding 6; **this replaces
     ADR 0014 Q5 for the preview**): a step in the preview (arrows, Page Up/Down, Home, End, a
     click in the filmstrip) moves the current image in the preview and the contact sheet together
     and **does not change the marking**. The existing test
     `loupe_moves_step_within_the_filter_and_clear_the_marking` in `browse.rs` asserts the old Q5
     rule; this task (and only this point) allows changing its marking assertion to the new rule
     and its name to match; the rest of the test stays. **Only the marked** (U3-Q3): when it is on
     and at least two visible images are marked, the filmstrip shows only the marked ones and every
     step goes only between them (Home and End to the first and last marked); with fewer than two
     marked it shows the whole visible list and steps as usual. When the preview opens with "only
     the marked" on and the current image not marked, it opens on the first marked image in the
     visible order.
  8. **Hide key `H` and `I` in the preview** (§2.2, §2.4, U3-Q2): `H` hides the right column and
     the filmstrip together, leaving only the title line and the image; pressed again they come
     back as they were. `I` opens and collapses the preview's right column (as `I` does for the
     main window's right column since T110).
  9. **Keys in the preview** (`lib.rs`): the loupe's keys, the rating and label keys and `M` work
     as in today's loupe. `S` (U3-Q3) turns "only the marked" on and off; there is also a toggle
     button at the left end of the filmstrip. Ctrl+O, Ctrl+I, Ctrl+W, Ctrl+Tab, Ctrl+Shift+Tab and
     F11 do nothing in the preview (U3-Q1). A dropped folder or file closes the preview first and
     then opens as today. The `M` window opens over the preview and stays modal (D16). Keys still
     do nothing while a text field has the keyboard.
  10. **Remembered state** in `settings.txt` with the rules of T109's keys (a bad value keeps the
      default and gives one problem naming the line; a file without the keys gives the defaults
      and no problem; every save writes all keys): `preview_column` (`open`, `collapsed`; default
      `open`), `preview_column_width` (default 300), `preview_info` and `preview_zoom` (`open`,
      `collapsed`; default `open`), `filmstrip` (`shown`, `hidden`; default `shown`),
      `filmstrip_marked` (`all`, `marked`; default `all`), `preview_hidden` (the state of `H` in
      the preview; default `false`).
- **Rules:** rules 3–7 and 10–13. No `Cargo.toml` change. The behaviour of the contact sheet,
  ratings, labels, filter, import, the `M` window and the settings window is unchanged apart from
  point 7. V1–V8 apply: stepping in the preview with the filmstrip and the right column shown must
  be as fast as in today's loupe.
- **Tests to add:** the thumbnail gate (`Ready` and `Error` run, `Preparing` and `Empty` pause);
  the filmstrip's range and scroll target (the current image in view after a step, at the first
  and last image, a list shorter than the strip) and hit test (a click gives the right index, a
  click between cells gives none); `browse.rs`: a step keeps the marking; with "only the marked"
  steps go only between the marked (Next, Previous, Home, End), and fewer than two marked steps
  through all; the start image when the preview opens with "only the marked" on and the current
  image not marked; the preview's keys (`S`; Ctrl+O, Ctrl+I, Ctrl+W, the tab keys and F11 give
  nothing in the preview; `H` and `I`); the window state to restore and to save when closing in
  the preview (a pure function: before maximized, before full screen, before a saved size);
  `settings.rs`: the new keys round-trip, a bad value of each gives its default and one problem
  naming the line, a file without them gives the defaults with no problem; the menu table with
  "Förhandsvisning"; one test of every text of `preview_text`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: Enter,
  Space, G and a double-click open the preview in full screen with only the title line, image,
  right column and filmstrip; Escape, G and Enter return to the contact sheet on the last image,
  in the window state from before (also after F11); three marked images stay marked after steps
  in the preview, and with "only the marked" (`S`) the steps go only between them; the filmstrip
  follows the current image and a click jumps; `H` leaves only the image and brings the rest
  back; `I` collapses and opens the right column; the Zoom buttons do what their keys do; after a
  restart the preview's column, sections and filmstrip are as they were; **`latency.ps1` stepping
  in the preview with the filmstrip shown and hidden, as in T75** (V1–V8 not slowed by the
  filmstrip; RawStride started with a file path now opens the preview in full screen, so the
  owner checks that the measurement still finds the window).
- **Questions (answered by the owner, 2026-11-13: every proposal approved):**
  - **U3-Q1** In the preview (point 2): no menu bar, command bar, tabs, status bar or side
    columns; a title line with today's loupe status line and the RAW/JPEG control; Ctrl+O,
    Ctrl+I, Ctrl+W, the tab keys and F11 do nothing until the preview is closed?
  - **U3-Q2** The preview's right column (points 3 and 8): open by default and remembered apart
    from the main window's right column (which starts collapsed); `I` opens and collapses it, `H`
    hides it together with the filmstrip?
  - **U3-Q3** The filmstrip (points 4, 7 and 9): one row of 96-point thumbnails with the label bar
    and the current and marked frames; "only the marked" with the key `S` and a button, used only
    when at least two images are marked; then steps go only between the marked?
  - **U3-Q4** Thumbnail priority (point 6): the thumbnail worker runs in the preview only while
    the shown image is drawn and pauses on each step until the new one is drawn? This keeps
    stepping as fast as today, but the filmstrip fills in only when the user stops.
  - **U3-Q5** The Zoom section (point 3): the zoom as text, buttons for Anpassa, Anpassa ↔ 100 %,
    Zooma in, Zooma ut and the check box Zooma vid pekaren?
  - **U3-Q6** Window state (points 1 and 2): the preview always full screen; closing it returns
    the window as it was; started with a file path, RawStride opens in the preview (full screen),
    which changes how `latency.ps1` sees the window?
