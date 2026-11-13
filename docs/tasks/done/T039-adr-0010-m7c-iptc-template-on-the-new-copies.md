# T39. ADR 0010 M7c: IPTC template on the new copies — Done

- **Start after T38 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`, `crates/pw-loupe/src/import_worker.rs`,
  `crates/pw-loupe/src/import_text.rs`, `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/lib.rs`.
- **What (ADR 0010 §3.2; the Tauri `run`, "IPTC template on the new copies"):**
  1. **Form:** `ImportForm` gets `iptc: IptcForm` with the strings `creator`, `copyright`, `credit`,
     `email`, `phone`, `url` (the six fields of the Tauri dialog; title and description are not in that
     dialog either). `IptcForm::to_iptc(&self) -> pw_metadata::Iptc`, as the Tauri `From<IptcDto>`:
     copyright as `LocalizedValue::x_default`, title and description empty.
  2. **Worker:** after the copy, when the result is not cancelled and the template is not empty
     (`Iptc::is_empty`), write it to every copied file that is not a companion file
     (`pw_ingest::COMPANION_EXTENSIONS`), in the destination and, when `to_backup`, in the backup — the
     same selection as the Tauri `run`. Use a `pw_metadata::ExifTool` that the worker thread starts
     itself, only when needed, and keeps for later runs. The program name comes from
     `ImportWorker::start()` (`"exiftool"`, as `ExifTool::start`) or from a new
     `ImportWorker::start_with_exiftool(program: &str)` used by the tests.
     - Send `ImportEvent::IptcProgress { id, done, total, current: PathBuf }` before each file.
     - Check the cancel flag before each file; when set, stop and mark the result `cancelled`.
     - `ImportDone` gets `iptc_errors: Vec<String>` (`"{file name}: {error}"`); if ExifTool cannot be
       started, one entry with that error and no further attempts in this run.
  3. **Dialog state:** `Running` also shows IPTC progress: add an `IptcProgress` case to the state
     machine (`Running { progress }` keeps the copy progress; add `iptc: Option<(usize, usize, PathBuf)>`
     or a small enum, your choice, tested).
  4. **Texts:** `iptc_progress_text(done, total, current) -> String` = "Skriver IPTC {done + 1} / {total}
     · {file name}"; `done_lines` adds up to 10 "IPTC: {error}" error lines after the failures (as the
     Tauri dialog).
  5. **Dialog (`main.rs`):** a collapsing section "IPTC-mall" (closed by default) with the six fields
     labelled Fotograf, Copyright (hint "© 2026 Namn"), Credit, E-post, Telefon, Webb, and the help text
     "Skrivs till .xmp-sidofilen för RAW och in i DNG/JPG/TIF. Tomma fält lämnas orörda." While running,
     the progress bar shows the IPTC progress (files) after the copy.
- **Rules:** ExifTool runs only on the worker thread; no `unwrap`/`expect`/`panic!` in non-test code;
  rules 6 and 7.
- **Tests to add:** `to_iptc` (copyright becomes `x-default`; an all-empty form is `is_empty`); the
  worker with an **empty** template never starts ExifTool (use a program name that does not exist: no
  `iptc_errors`); with a template and a program that does not exist: the copy is reported as usual and
  `iptc_errors` has one entry; cancel before the IPTC step writes nothing; the state machine and the
  texts (including the 10-line limit). A real write is checked by hand.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: after an import with
  Fotograf and Copyright filled in, the `.xmp` sidecar of a RAW (and a DNG or JPG itself) carries them,
  for example in Photo Mechanic or with `exiftool -XMP:Creator -XMP:Rights <file>`.
