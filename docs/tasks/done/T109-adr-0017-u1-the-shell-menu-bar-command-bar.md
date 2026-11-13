# T109. ADR 0017 U1: the shell – menu bar, command bar, status bar, side columns, themes — Done

- **Can start now.** The owner approved the proposals of U1-Q1–U1-Q5 (at the end of this task) on
  2026-11-13; they are written into the points below. Not at the same time as any other task that
  changes `main.rs`.
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/lib.rs` (the new key function of
  point 10, the doc comment of `key_action`, and the `mod`/`pub use` lines); new
  `crates/pw-loupe/src/shell.rs` (layout state, menu table, themes, contrast; pure functions); new
  `crates/pw-loupe/src/shell_text.rs` (every new text of this task, rule 13);
  `crates/pw-loupe/src/settings.rs` (the new keys, point 9); `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.1 (main window), §2.4 (the keys of stage A), §2.5 (look and remembered state)
  and §7 U1. Today's commands move into a menu bar and a command bar, and the main window gets the
  frame that the inspector (U2), the full-screen preview (U3), the library (U4) and the tasks area
  (U5) are later put into. **The only new functions are "Öppna mapp", the two themes and the
  remembered state.**
- **Not in this task:** sort order (U14); the inspector's content and the new meaning of `I` (U2);
  the full-screen preview and the filmstrip (U3: in U1 the loupe stays in the central area as
  today); folder tree, favourites and recent folders (U4); the tasks area with per-file progress and
  a stop button (U5); icons (a new dependency, §2.5); the key map file (U8); eframe's `persistence`
  feature (stays off, GUI_REQUIREMENTS §6).
- **What:**
  1. **Layout** (`egui::Panel::top`/`bottom`/`left`/`right` and `egui::CentralPanel`), top to
     bottom: the menu bar; the command bar; then left column | central area | right column; the
     status bar across the whole width at the bottom. The central area holds the row of folder tabs
     and under it the contact sheet or the loupe, as today. The Windows-native frame is unchanged
     (§2.5).
  2. **Menu bar** (`egui::MenuBar`, egui 0.37) with every existing command, each showing its key on
     the right (`egui::Button::shortcut_text`; only the main key where a command has several). A
     menu item does exactly what its key does, **through the same function** as the key (no second
     copy of the logic), with the same targets (ratings and labels: the marked images, else the
     current one, ADR 0014 §2.6) and the same refusals. The menus are one table in `shell.rs`
     (menu, item, key text, command), so the test of point 11 can check every key text.
     Menus and items (U1-Q1, the reviewer's proposal; names from RawStride's own commands, §2.1):
     - **Arkiv:** Öppna mapp… (Ctrl+O), Importera… (Ctrl+I), Stäng flik (Ctrl+W); Inställningar…;
       Avsluta (Alt+F4; through the normal exit, so the write wait of T105 runs).
     - **Redigera:** Markera alla (Ctrl+A), Avmarkera (Esc); Metadata… (M).
     - **Bild:** Betyg ▸ Inga stjärnor (0), 1–5 stjärnor (1–5); Avvisa (X); Etikett ▸ Röd (6),
       Gul (7), Grön (8), Blå (9), Lila (Ctrl+5), Ingen etikett (Ctrl+0); RAW ↔ JPEG (R).
     - **Visa:** Kontaktark ↔ lupp (G); Anpassa (F), Anpassa ↔ 100 % (Z), Zooma in (+), Zooma ut
       (−), Zooma vid pekaren (A, a check item); Information (I, a check item); Filter ▸ the same
       choices as the command bar's three lists, as radio items, and "Visa alla" (clears the
       filter); Nästa flik (Ctrl+Tab), Föregående flik (Ctrl+Shift+Tab); Sidokolumner (the hide
       key, U1-Q2), Vänster kolumn and Höger kolumn (check items); Helskärm (F11); Tema ▸ Mörkt,
       Ljust (radio items).

     An item that does nothing in the current view (the zoom items in the contact sheet, "Markera
     alla" in the loupe) is shown disabled, not hidden. While the metadata window, the import dialog
     or the settings window is open, the menu bar and the command bar take no clicks (the metadata
     window stays modal, D16).
  3. **Command bar** (one row under the menu bar; it wraps when the window is narrow): "Importera…",
     "Öppna mapp…", the active tab's filter (today's three lists moved unchanged, ADR 0014 §5.2,
     amendment 1) and the thumbnail size (today's slider moved from the status line; Ctrl + mouse
     wheel unchanged). The "Inställningar" button leaves the top row (it is in Arkiv). After a click
     in the command bar no widget keeps the keyboard, as today (ADR 0014 §4.5). The ADR's "Visa
     (alla/markerade/…)" is not built (U1-Q5).
  4. **Öppna mapp (Ctrl+O, new; ADR 0009 §3.3 reserved it):** the Windows folder dialog (`rfd`, as
     `pick_folder`), its title from `shell_text`, starting in the active tab's folder when it has
     one. The chosen folder opens exactly as a dropped folder does (`open(path, true)`: as a contact
     sheet in a new tab, an empty active tab is filled, an already open folder is activated; GR12,
     GR13). Cancelling changes nothing. Not while the metadata window, the import dialog or the
     settings window is open. The doc comment of `key_action` no longer lists Ctrl+O as reserved.
  5. **Status bar** (its own bottom panel, always present; its height follows the text size):
     in the contact sheet today's line (position, "n markerade", name, message, notice, hidden
     import status); in the loupe today's line and the RAW/JPEG control (G4). Its text and colours
     come from the theme. `I` keeps today's meaning until U2 (it hides the line and the rating badge
     in the loupe). The selection's size in MB is not added (U1-Q5).
  6. **Side columns** (§2.1): left and right, each resizable by dragging its inner edge (at least
     160 points, at most 40 % of the window width; default 240 left, 300 right) and collapsible to
     a thin strip with a button in its header. The **hide key** (U1-Q2, proposal: `H` without
     modifiers; not Tab, which changes tab with Ctrl) hides both columns and, pressed again, brings
     them back as they were; it works in the contact sheet and the loupe. Content in U1 (U1-Q3,
     proposal): the left column has the headings "Bibliotek" and "Uppgifter", the right column
     "Inspektör", each with one line in weak text from `shell_text`; under "Uppgifter" the hidden
     import's status (today's `hidden_import_status`) when there is one. U2, U4 and U5 fill them.
  7. **F11:** full screen for the main window (`egui::ViewportCommand::Fullscreen`); F11 again
     returns to the earlier size and position. Escape keeps its meanings (back to the contact sheet,
     clear the marking) and does not leave full screen in U1.
  8. **Themes (§2.5, U-Q3):** Mörkt (default) and Ljust, chosen in Visa ▸ Tema (applied at once and
     saved) and in the settings window as a list "Tema" (applied and saved with "Spara", as T106's
     settings). Each theme is an `egui::Visuals` built in `shell.rs` from one small palette:
     backgrounds, panels, text, weak text, **one accent colour** (selection, active controls, the
     contact sheet's current and marked frames), warning and error. Backgrounds and text are
     colour-neutral greys (R = G = B). The font, sizes and the image area's background follow U1-Q4
     (proposal: egui's bundled font; body text 14, small text 11, headings 16 points; the status
     bar in the proportional font at 13; spacing as egui's default; the contact sheet and the image
     area stay a neutral dark grey in **both** themes, so an image is judged against the same
     surround; the light theme lightens menus, bars and columns). Every colour used for text in
     `main.rs` comes from the theme: no `Color32::LIGHT_GRAY` or `from_gray(…)` left for text. The
     label colours of ADR 0013 are not text and stay.
     **WCAG AA:** `contrast_ratio(a, b) -> f32` in `shell.rs` (WCAG 2.x relative luminance with the
     sRGB curve), and a table in `shell.rs` of every pair the program draws (text, weak text,
     warning, error and the accent text on each background they are drawn on, including the centre
     texts on the image area's background). Text pairs must reach **4.5:1**, weak text included;
     the accent, the focus outline and widget borders against their backgrounds **3:1** (WCAG
     1.4.11). Text drawn over an image (the rating badge) is not in the table.
  9. **Remembered state (§2.5)** in `settings.txt` (D10), with the rules `load_settings` has today
     (an unknown key or a bad value keeps the default and gives one problem naming the line; a
     file without the new keys gives their defaults and no problem): `theme` (`dark`, `light`);
     `window` (`x,y,width,height,scale`: the outer rectangle in points as eframe reports it, and the
     `native_pixels_per_point` at that time); `window_maximized`; `fullscreen`;
     `left_column_width` and `right_column_width`; `left_column` and `right_column` (`open`,
     `collapsed`); `columns_hidden`.
     - **Saved** at exit (`on_exit`; a failure is only written with `eprintln!`, no dialog at exit)
       and at once after a theme change. Every save writes the current values of **all** keys, so
       "Spara" in the settings window does not write back an old layout.
     - **At start** the theme and font sizes are set before the first frame. No saved window, or
       `window_maximized`, gives a maximized window as today. Otherwise the saved size and position
       are used, but only when the rectangle (in pixels, from `scale`) lies at least partly on a
       screen (Windows: `MonitorFromRect` with `MONITOR_DEFAULTTONULL`; `windows-sys` already has
       `Win32_Graphics_Gdi`); the decision is a pure function that takes the screen check as a
       closure, so it is tested without a screen. Off every screen, or smaller than 400 × 300
       points: maximized. `fullscreen` starts in full screen.
  10. **Keys** (`lib.rs`): `shell_key_action(key, ctrl, shift, alt) -> Option<ShellAction>`:
      Ctrl+O → `OpenFolder`; F11 without modifiers → `ToggleFullscreen`; the hide key (U1-Q2) →
      `ToggleColumns`. Tab gives `None`; Ctrl+Tab stays `tab_action`. In the key loop they come
      after the modal checks (metadata window, import dialog) and before the rating keys, and do
      nothing while a text field has the keyboard (`egui_wants_keyboard_input`).
- **Rules:** rules 3–7 and 10–13. No `Cargo.toml` change (`egui::MenuBar`, `rfd` and `windows-sys`
  with `Win32_Graphics_Gdi` are already there). The behaviour of the contact sheet, the loupe, the
  ratings, the filter, the import, the metadata window and the settings window is unchanged apart
  from where their controls are. V1–V8 apply: the shell must not slow stepping in the loupe.
- **Tests to add:** `shell_key_action` (Ctrl+O, F11, the hide key; none with other modifiers; Tab
  gives `None`); every item of the menu table whose key text names a key gives the same command
  through the key functions (`shell_key_action`, `key_action`, `rating_key_action`,
  `grid_key_action`, `tab_action`, `opens_import`), so a menu never shows a wrong key;
  `contrast_ratio` against known values (black on white 21:1; `#777777` on white about 4.48:1) and
  every pair of the table in both themes; the column width clamp (minimum, 40 %, a window narrower
  than two minimum columns); `settings.rs`: the new keys round-trip through `save_settings` and
  `load_settings`, a bad value of each gives its default and one problem naming the line, and a
  file with only `create_iim` and `fallback_encoding` gives the defaults with no problem; the start
  window decision (nothing saved, maximized, on a screen, off every screen, too small); one test
  of every text of `shell_text` (as the test of `editor_text`).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: the menus
  with their names and keys; a menu item does what its key does (a rating from the menu on three
  marked images, a label, RAW ↔ JPEG); Ctrl+O and "Öppna mapp…" open a folder in a new tab and
  activate an already open one; the side columns resized, collapsed, hidden with the hide key and
  back; F11 and back; every text readable in both themes; after a restart the theme, the window's
  size and position, full screen and the columns are as they were; `latency.ps1` stepping in the
  loupe as in T75 (V1–V8 not slowed by the shell).
- **Questions (answered by the owner, 2026-11-13: every proposal approved):**
  - **U1-Q1** The menus and their items (point 2): Arkiv, Redigera, Bild, Visa, in that order?
  - **U1-Q2** The hide key (points 6 and 10): `H`?
  - **U1-Q3** The side columns' content before U2, U4 and U5 (point 6): headings and one line each,
    with the hidden import's status under "Uppgifter"?
  - **U1-Q4** Font sizes, and a neutral dark image area and contact sheet in both themes (point 8)?
  - **U1-Q5** The ADR's picture shows two things that are new function: "Visa (alla/markerade/…)"
    in the command bar (the filter of ADR 0014 has no "only the marked") and the selection's size
    in MB in the status bar (the folder listing keeps no file sizes). Proposal: neither in U1;
    both are placed when the owner reorders after stage A.
