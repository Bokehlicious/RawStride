# T36. ADR 0010 M5c: run the import and open the result in a new tab — Done

- **Start after T35 is done.**
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/main.rs` only.
- **What (ADR 0010 §3.1, GR12, Q2):**
  - **Start:** a "Starta import" button, enabled when `can_start()`, and Enter (`DialogKey::Start`):
    `start()`, then `run(form)` on the worker and `sent(id)`.
  - **Running:** the fields and buttons of the form are disabled; a progress bar (bytes done of bytes
    total) with `progress_text`; "Avbryt" calls `ImportWorker::cancel()` when `dialog.cancel()` is true.
    The dialog cannot be closed (Escape does nothing), but a **"Dölj"** button hides it so the grid and
    loupe can be used during the copy (ADR 0010 §8.2); Ctrl+I shows it again. While it is hidden and
    running, the status line shows `progress_text` in short form ("Importerar {done} / {total}").
    Keep polling the worker every frame whether the dialog is shown or hidden, and ask for repaints
    while running.
  - **Done:** show `done_lines` (error lines in red). When the state becomes `Done` and `result_folder()`
    is `Some`, open that folder **once** in a new tab with the existing `open(&folder, true)` (it starts
    in the grid, GR13; an already open folder is activated, GR12). If the dialog is hidden at that
    moment, the status line shows "Importen klar: {copied} filer" or "Importen avbruten: {copied} filer"
    (or the error). Buttons: "Ny import" (`back_to_editing`) and "Stäng".
- **Rules:** as T35; the copy never runs on the UI thread; rules 6 and 7.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (ADR 0010 §8.2, step A):
  import from a real card into an empty folder: progress, summary, and the folder opens in a new tab as
  a contact sheet; cancel during the copy leaves no final-named partial files and the summary says
  "Avbruten"; "Dölj" during the copy, then stepping in the grid and the loupe stays smooth; a second
  import of the same card shows the files as already present and opens no new tab when nothing was
  copied.
