# T15. A script that runs the checks and writes a readable report — Done

- **File:** `tools/check.ps1` (new).
- **Why:** the owner runs the same commands after every task and the output file is written as
  UTF-16, which is hard to read.
- **What:** a PowerShell script that, run from the repository root:
  - uses `$env:VCPKG_INSTALLED_DIR` if it is set; otherwise, if
    `..\PhotoWorkflow\vcpkg_installed` exists next to this worktree, uses that and prints which
    folder it chose; otherwise stops with a clear message;
  - runs, in order, `cargo fmt --all --check`, `cargo test --workspace` and
    `cargo clippy --workspace --all-targets`, and keeps going after a failure so all three are
    reported;
  - writes everything to `test-output.txt` in the repository root as **UTF-8**, and ends with a
    short summary: for each of the three steps "OK" or "FAILED";
  - takes an optional `-Ignored` switch that adds `-- --ignored` to the test step (for the
    real-file runs from T6–T14);
  - exits with code 0 only if all steps passed.
- **Rules:** no new tools or modules; plain PowerShell 5.1 (the version in Windows). Do not
  change any other file; `test-output.txt` is not committed.
- **Done when:** the owner runs `.\tools\check.ps1` and gets a readable `test-output.txt` with the
  summary at the end.
