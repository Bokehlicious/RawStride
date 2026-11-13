# T9. Eject polish: readable exit status, fast timeout test, no dead code off Windows — Done

- **Crate:** `pw-ingest`. **File:** `crates/pw-ingest/src/paths.rs`.
- **What:**
  1. In `eject()`, show the exit status with `{status}` instead of `{status:?}`, so the message
     reads "exit code: 1" instead of `ExitStatus(ExitStatus(1))`.
  2. Give `wait_until_gone` the timeout and the poll interval as parameters
     (`fn wait_until_gone(still_present: impl FnMut() -> bool, timeout: Duration, interval: Duration) -> bool`).
     `eject()` passes the current values (5 s and 250 ms) as named constants. The three tests pass
     short values (e.g. 20 ms timeout, 1 ms interval), so the "never gone" test no longer waits
     5 seconds.
  3. `wait_until_gone` is only used by the Windows code and by tests. Mark it
     `#[cfg(any(windows, test))]` so other platforms do not warn about dead code.
- **Rules:** behaviour on Windows unchanged; keep the three existing test names and what they check.
- **Done when:** `cargo test -p pw-ingest` passes and finishes in well under a second;
  `cargo clippy -p pw-ingest --all-targets` shows no warnings.
