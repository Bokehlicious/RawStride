# T16. Developer setup: document what the tests need — Done

- **Start after T6 and T15 are done.**
- **File:** `docs/dev-setup.md`.
- **What:** add a short "Running the tests" section:
  - in a worktree, `vcpkg_installed` is not shared; set `VCPKG_INSTALLED_DIR` to the main checkout's
    folder (one line for the current window, one for a permanent user variable);
  - ExifTool must be on `PATH` (or `PW_EXIFTOOL_PL` set), otherwise the `pw-metadata` integration
    tests fail on purpose;
  - `PW_TESTDATA` and `-- --ignored` for the real-file tests (T6–T14);
  - `.\tools\check.ps1` (T15) runs all checks.
- **Rules:** only document what exists and has been verified (TESTING.md §12). Keep the existing
  sections.
- **Done when:** the section is there and every command in it has been run once by the owner.
