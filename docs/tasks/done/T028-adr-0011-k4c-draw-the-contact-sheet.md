# T28. ADR 0011 K4c: draw the contact sheet — Done

- **Start after T27 is done.**
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/main.rs` only.
- **What:** when the active tab's `mode` is `ViewMode::Grid`, the image area shows the contact sheet
  instead of the loupe. Switching between the modes is T29; in this task a tab is shown in the grid
  as soon as it is opened (that is already the default of `Tab`), and the loupe code path stays as
  it is for tabs in `Loupe` mode.
  - Start a `pw_thumbcache::ThumbWorker` in `run()` next to the metadata worker, and keep a
    `ThumbGrid`, a generation counter and a `HashMap<usize, egui::TextureHandle>` in `Loupe`.
    Every time a tab is loaded (`load_active_tab`), increase the generation, call
    `set_generation`, create a new `ThumbGrid` and clear the textures.
  - Draw the grid in an `egui::ScrollArea::vertical()` with `show_viewport`: content height =
    rows × (cell + spacing); spacing 8 pt; use `columns`, `visible_range` (margin 1 row) from T25.
    The cell size comes from a field `thumb_size: u32` in `Loupe`, starting at
    `DEFAULT_THUMBNAIL_SIZE`.
  - Each frame: `set_visible` with the visible range, then for `to_request(range, cell)` send a
    `ThumbnailRequest` (path from the tab's file list, `cell = request_cell(thumb_size, ppp)`) and
    call `mark_requested`; poll `try_recv` until empty, `apply` each event and turn a ready image
    into a texture with `color_image` + `ctx.load_texture` (linear filtering); call
    `forget_outside` with the visible range plus 2 rows and remove those textures.
  - A cell: the thumbnail scaled to fit the square, centred, keeping its proportions; a grey
    placeholder while missing or asked for; the text "ingen förhandsvisning" for `NoPreview` and a
    short error text for other failures; the file name under the cell, shortened when too long
    (egui `Label` with `truncate()` or a clipped painter text). The current image (`tab.current`)
    gets a clear frame.
  - Clicking a cell makes it current (`Tabs::set_current`).
  - Keys in the grid: `grid_key_action` + `navigate` (visible rows for Page Up/Down = view height ÷
    (cell + spacing), at least 1); after a key move, scroll so the current cell is visible
    (`ui.scroll_to_rect`). `OpenLoupe` does nothing yet (T29). The tab keys keep working; the loupe's
    `key_action` is **not** used while the grid is shown.
  - Status line in the grid: file name of the current image, "position / count", and the message as
    today.
  - Ask for a repaint while thumbnails are still asked for (e.g. `request_repaint_after` 50 ms), so
    results are shown without moving the mouse.
- **Rules:** no file I/O or decoding on the UI thread (G2); no `unwrap`/`expect`/`panic!`; no new
  dependencies; the loupe behaviour for tabs in `Loupe` mode is unchanged.
- **Tests:** the logic is in T25 and T27; this task adds none unless a new plain function is needed
  (put it in the library with a test).
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: drop the test folder → a
  grid appears at once with placeholders that fill in; arrows, Page Up/Down, Home/End move the frame
  and scroll; clicking selects; a vertical A7R IVA image stands upright.
