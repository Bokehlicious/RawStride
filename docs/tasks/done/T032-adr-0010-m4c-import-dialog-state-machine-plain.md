# T32. ADR 0010 M4c: import dialog state machine (plain Rust) — Done

- **Start after T31 is done.**
- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/src/import_dialog.rs`, declared and
  re-exported in `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`.
- **What (ADR 0010 §4.2):** `ImportDialog` holds an `ImportForm` and the state, and never touches files
  or threads itself. It returns the requests to send and takes the worker's events, so every transition
  is a plain function that is tested without a window or a worker.
  - States: `Editing { preview: Option<Preview>, error: Option<FieldError> }`, `Previewing`,
    `Running { progress: Option<Progress> }`, `Done { done: ImportDone }`.
  - `edit(&mut self, change: impl FnOnce(&mut ImportForm), now: Instant)`: changes the form and marks
    it as changed at `now`; remembers whether the **source** changed (a new scan is needed) or only the
    other fields (the scan can be reused).
  - `tick(&mut self, now: Instant) -> Option<DialogRequest>`: when the form changed at least 300 ms ago
    and nothing is running: validate; on an error go to `Editing` with that error and no request; when
    valid return `DialogRequest::Preview { source, options, rescan }` and go to `Previewing`.
  - `start(&mut self) -> Option<DialogRequest>`: only in `Editing` with a preview that has
    `files_to_copy > 0` and no error → `DialogRequest::Run { source, options }`, state `Running`.
  - `sent(&mut self, id: RequestId)` records the id of the request just sent; `apply(&mut self, event:
    &ImportEvent)` ignores events of any other id (an older preview, as in the loupe's metadata cache):
    `Previewed` → `Editing` with the preview or with the error on `Field::Source`; `Progress` → updates
    `Running`; `Done` → `Done`.
  - `disconnected(&mut self)`: if the worker is gone, `Previewing` and `Running` become an error
    (`Editing` with a `Source` error "Importen avbröts: arbetaren har stoppats.", or `Done` with that
    `error`), never "running" forever.
  - `cancel(&self) -> bool` (true while `Running`, so the UI calls `ImportWorker::cancel`), `can_close()`
    (false while `Running`), `back_to_editing()` from `Done` (keeps the form, clears the preview and
    marks the form as changed so a fresh preview follows), and `result_folder()` from `Done` using
    T30's `result_folder`.
- **Rules:** no egui drawing; no `unwrap`/`expect`/`panic!` in non-test code; no new dependencies.
- **Tests to add:** no request before 300 ms, one after; an invalid form gives its field error and no
  request; a source change asks for `rescan = true`, a template change for `rescan = false`; an old
  preview event is ignored; `start` is refused without a preview, with 0 files to copy, and while
  previewing; progress and done move the state; `disconnected` while previewing and while running;
  `can_close` is false while running; `result_folder` is `None` when nothing was copied and the
  largest folder otherwise; `back_to_editing` asks for a new preview.
- **Done when:** `.\tools\check.ps1` is all OK.
