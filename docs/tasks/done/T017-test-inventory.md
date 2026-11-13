# T17. Test inventory — Done

- **Start after T12–T14 are done.**
- **File:** `docs/test-inventory.md` (new).
- **What:** one table with a row per test file (unit-test modules counted per crate): crate, file,
  what it verifies (one line), category from TESTING.md §4 (unit, integration, real-file,
  external tool), prerequisites (none / ExifTool / `PW_TESTDATA` / vcpkg libraries / Windows),
  number of tests, and how many are ignored by default.
- **Rules:** describe only tests that exist; read each file, do not guess from names. Do not
  claim a requirement is verified (TESTING.md §7).
- **Done when:** the numbers match a `cargo test --workspace` run (the owner checks the totals).
