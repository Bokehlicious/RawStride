# T38. ADR 0010 M7b: job name and first running number — Done

- **Start after T37 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`, `crates/pw-loupe/src/main.rs`.
- **What:**
  1. `ImportForm` gets `job: String` (default empty) and `first_seq: u32` (default 1). `validate()`
     passes them on instead of `""` and `1` (the job is trimmed by `pw-ingest`).
  2. Dialog: after "Filnamnsmall", a row "Jobb" with a text field and "Första nr" with an
     `egui::DragValue` (range 0 to 999 999, as the Tauri field allows 0), and the help text
     "Variabler: {year} {year2} {month} {day} {hour} {minute} {second} {date} {time} {make} {model}
     {name} {seq} {seq:4} {job}" in a small, grey font (the list in the `pw_ingest::template` module
     documentation).
- **Rules:** rules 6 and 7.
- **Tests to add:** the defaults; `job` and `first_seq` reach `Options`; a job with spaces around it is
  trimmed.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand that `{job}` in the folder
  template and `{seq:4}` in the file-name template give the expected names in the preview.
