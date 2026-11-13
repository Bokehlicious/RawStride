# T35. ADR 0010 M5b: draw the import dialog (editing and preview) — Done

- **Start after T34 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/Cargo.toml` (add
  `rfd = { version = "=0.17.2", default-features = false }`, approved 2026-11-09, GUI_REQUIREMENTS §6.1),
  `crates/pw-loupe/src/main.rs`. `Cargo.lock` may change **only** by listing `rfd` among `pw-loupe`'s
  dependencies; `rfd` 0.17.2 is already in it. Check with `git diff Cargo.lock` and report it.
- **What (ADR 0010 §3.1 step A, without the copy):**
  - Start an `ImportWorker` in `run()` next to the other workers. `Loupe` gets the worker, an
    `Option<ImportDialog>` (open or not), `last_form: ImportForm` (the settings of this session, Q5) and
    the detected cards.
  - **Open:** Ctrl+I (`opens_import`) or an "Importera…" button at the end of the tab bar:
    `ImportDialog::new(last_form.clone())` and ask the worker for `cards()`. **Close:** Escape
    (`dialog_key_action`) or a "Stäng" button, only when `can_close()`; the form is then saved in
    `last_form`. While the dialog is open, keys go to the dialog only (not to the grid, the loupe or the
    tabs); text fields keep their normal keys.
  - **Window:** an `egui::Window` titled "Importera", centred, not collapsible. In it, in this order:
    - **Källa:** a text field, a "Välj…" button (`rfd::FileDialog::new().set_title("Välj källa")
      .pick_folder()`; add `.set_parent(…)` with the eframe window if it compiles, otherwise leave it
      out), and one button per detected card (its drive, for example `E:\`) that sets the source, plus
      "Sök igen" (`rescan`) next to the preview.
    - **Mål:** a text field and "Välj…" (title "Välj mål").
    - **Mappmall** and **Filnamnsmall:** text fields.
    - Under each field, its `FieldError` in red when there is one.
    - **Preview:** "Förhandsvisar …" while previewing; otherwise `preview_summary`, then the lines
      (`preview_line`) in a small scroll area and `more_lines`.
  - **Each frame while open:** apply text changes with `edit(…, Instant::now())`; call `tick()` and send
    a request with `preview()`, then `sent(id)`; a `SubmitError` is shown as an error line in the dialog.
    Poll the worker until empty: `Cards` updates the card list, other events go to `apply`;
    `Disconnected` calls `disconnected()`. Ask for a repaint (about 100 ms) while a preview is pending.
  - No start button yet (T36).
- **Rules:** no file I/O on the UI thread (the folder dialog itself is the accepted exception, ADR 0010
  §10); no `unwrap`/`expect`/`panic!`; the grid and loupe behave as before when the dialog is closed;
  rules 6 and 7.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: Ctrl+I and the button open
  the dialog; a card in the reader shows as a button; "Välj…" opens the Windows folder dialog; a bad
  template or an empty destination shows its message under the right field; typing does not freeze the
  window; the preview updates shortly after typing stops; Escape closes, and Ctrl+I shows the same
  settings again.
