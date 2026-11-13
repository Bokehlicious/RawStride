# T5. Card eject in pw-ingest: safer PowerShell call and a real wait — Done

- **Start after T8 is done.**
- **Crates:** `pw-ingest`, `photoworkflow`.
- **Files:** `crates/pw-ingest/src/` (same place as `drive_of` after T8), `src-tauri/src/ingest.rs`.
- **What:**
  - Move `eject()` from `src-tauri/src/ingest.rs` to `pw-ingest` next to `drive_of`, as
    `pub fn eject(source: &Path) -> String`. `src-tauri` calls it; its own copy is deleted.
  - Start PowerShell by full path:
    `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe` (read `SystemRoot` with
    `std::env::var_os`; if it is missing, return a clear message instead of using `PATH`).
  - Treat a non-zero exit status as a failure and include it in the message.
  - Replace the fixed 1.5 s sleep with polling: check every 250 ms, for at most 5 s, whether
    `<drive>\DCIM` is gone.
- **Tests to add:** put the polling in a small function that takes a "still present?" closure and
  test it without a real drive (gone at once, gone after a few polls, never gone).
- **Done when:** `cargo test -p pw-ingest` and `cargo test -p photoworkflow` pass. The owner tests
  a real eject by hand.
