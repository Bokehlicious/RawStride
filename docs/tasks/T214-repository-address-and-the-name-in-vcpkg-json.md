# T214. The repository's address in `Cargo.toml` and RawStride in `vcpkg.json` — Open

- **Why:** step R2 of `docs/tasks/plans/public-release.md` (accepted by the owner, 2026-11-19),
  backlog "Before the repository is made public" (small fixes; item 4, the old name in
  configuration). The workspace's `repository` field is empty, and no member crate inherits it,
  and `vcpkg.json` still carries the project's old name PhotoWorkflow (ADR 0012 renamed the
  product to RawStride; ADR 0012 decision 4 keeps the old name only for crates, folders,
  branches, `PW_TESTDATA` and the data folder, not for this manifest).
- **Files:** `Cargo.toml` (root), `vcpkg.json`, and the `Cargo.toml` of the 17 workspace members:
  the 14 `crates/*/Cargo.toml` and `tools/rea-probe/Cargo.toml`,
  `tools/preview-bench/Cargo.toml`, `tools/raw-decode-bench/Cargo.toml`.
- **What:**
  1. Root `Cargo.toml`, `[workspace.package]`: `repository = ""` →
     `repository = "https://github.com/Bokehlicious/RawStride"`. Nothing else in the file changes.
  2. Each of the 17 members' `[package]`: one new line `repository.workspace = true`, directly
     after its `license.workspace = true` line (every member has one). Nothing else changes in
     those files. `tools/egui-demo` has its own `[workspace]` and is not touched.
  3. `vcpkg.json`: `"name": "photoworkflow"` → `"name": "rawstride"` (vcpkg allows only lower
     case letters, digits and `-`), and `"description": "Native dependencies for PhotoWorkflow"` →
     `"Native dependencies for RawStride"`. The version, the dependencies and the
     `builtin-baseline` do not change.
- **Tests to add:** none (no code changes); the checks in "Done when" show the files still work.
- **Not in this task:** the binary's name `pw-loupe` (plan step R10, a task of its own after the
  trademark search); the old name in folders, crates and scripts (kept by ADR 0012 decision 4);
  `authors` (three members do not inherit it; not changed here); `tools/egui-demo` and
  `prototypes/`.
- **Rules:** no change to any `[dependencies]` and no change to `Cargo.lock`; if `Cargo.lock`
  changes, stop and explain. No other file changes (rule 2).
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the 19 files above
  and this file's status and log; `cargo metadata --format-version 1 --no-deps --offline` gives
  the new `repository` for all 17 packages (say how many, and paste one package's value); and
  vcpkg reads the manifest without error, run in the repository root:
  `& "$env:VCPKG_ROOT\vcpkg.exe" install --dry-run --triplet x64-windows-static-md` (the triplet
  `.cargo\config.toml` sets; a dry run builds and installs nothing). Paste its last lines and its
  exit code in the report; if it is not 0, or `VCPKG_ROOT` is not set, stop and ask. `check.ps1`
  does not run vcpkg, so it does not show the manifest is read.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (plan step R2). Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README, this file and the named files): the old strings match; but no member crate had `repository.workspace = true`, so the field alone would reach no package and the `cargo metadata` check would fail. Changed after it: the 17 members inherit the field (point 2, files listed); the dry run names the triplet and reports its exit code; said that `check.ps1` does not run vcpkg |
| 2026-11-19 | Open | Cold read done |
