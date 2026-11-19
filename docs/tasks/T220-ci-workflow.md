# T220. A CI workflow on GitHub Actions — Open

- **Why:** public-release plan step C1 (`docs/tasks/plans/public-release.md`) and the backlog item
  "CI before the repository becomes public": before the repository is public, a workflow on
  GitHub builds the project on a clean Windows machine and runs the same check as every commit
  (`.\tools\check.ps1`). The first run also tests `docs/building.md` on a machine that has only
  what the workflow installs (step C2, after this task).
- **Crates:** none. **Files:** `.github/workflows/ci.yml` (new), `docs/building.md`.
- **What:**
  1. **Triggers:** `workflow_dispatch` and `pull_request` to `main` only, nothing else (the
     repository is private, and Windows minutes count double against the free quota; step P3
     widens the trigger after the repository is public). `concurrency` per branch with
     `cancel-in-progress: true`. `permissions: contents: read` at the top of the file, nothing
     more. One job on `windows-latest`, `timeout-minutes: 120`. Job environment:
     `CARGO_INCREMENTAL: 0` (a smaller `target` to cache, item 8).
  2. **Actions:** only GitHub's own actions (`actions/checkout`, `actions/cache`,
     `actions/cache/restore`, `actions/cache/save`, `actions/upload-artifact`), each pinned to a
     full commit SHA with the version in a comment (`uses: actions/checkout@<40 hex> # v4.2.2`).
     No third-party actions. `actions/checkout` with `persist-credentials: false`.
  3. **Git long paths:** `git config --global core.longpaths true` before the checkout
     (`docs/building.md` §1 asks for it).
  4. **Rust:** a step runs `rustup toolchain install` without arguments (it reads
     `rust-toolchain.toml`: 1.99.1 with `rustfmt` and `clippy`; newer rustup versions do not
     install it on `rustup show`), then `rustup show` and `cargo --version` so the log shows the
     version. Do not name the version in the workflow; `rust-toolchain.toml` is the only place.
  5. **vcpkg:** do not use the runner's preinstalled vcpkg (`C:\vcpkg`). Clone `microsoft/vcpkg`
     into a folder outside the checkout (for example `${{ runner.temp }}\vcpkg`) so that it holds
     the commit in `vcpkg.json`'s `builtin-baseline` (read from the file in the step, not written
     twice): a full clone, or `git init`, `git fetch --depth 1 origin <sha>` and
     `git checkout FETCH_HEAD`; a plain shallow clone lacks the commit. Set `VCPKG_ROOT` to that
     folder for the later steps, run `bootstrap-vcpkg.bat -disableMetrics`, and run
     `vcpkg install --triplet x64-windows-static-md` in the repository root, so the libraries land
     in `.\vcpkg_installed` as `check.ps1` and the `build.rs` files expect.
     **Binary cache:** vcpkg's `files` provider
     (`VCPKG_BINARY_SOURCES=clear;files,<folder>,readwrite`), the folder outside the checkout (for
     example `${{ runner.temp }}\vcpkg-bincache`) and created before `vcpkg install`. vcpkg's own
     package hashes include the compiler version, so a cache keyed only on `vcpkg.json` would be
     restored after a runner image update but not used, and never saved again. Therefore:
     `actions/cache/restore` with `restore-keys` set to the prefix (runner OS, triplet, hash of
     `vcpkg.json`), and `actions/cache/save` after `vcpkg install` with that prefix plus
     `${{ github.run_id }}`, so the folder is saved with whatever vcpkg added. A run with an
     unchanged `vcpkg.json` and compiler then does not build LibRaw again.
  6. **ExifTool:** the Windows 64-bit package (`exiftool-<version>_64.zip`) of a pinned version,
     12.67 or newer (use 13.59, the version in `docs/dev-setup.md`, unless it can no longer be
     downloaded; then the oldest later one that can, and say so in the report), from a URL that
     keeps that version (exiftool.org serves only the newest; the SourceForge file archive keeps
     older ones; its `/download` URLs redirect, so use `Invoke-WebRequest` or `curl.exe -L`). The
     step checks the zip's SHA-256 against a value written in the workflow and fails on a
     mismatch. The value: from exiftool.org's `checksums.txt` if the pinned version is still
     listed there; otherwise computed from a hand download and checked against the SHA-1 or MD5
     that SourceForge shows. A comment in the workflow names which. The 13.x zip holds a top
     folder `exiftool-<version>_64\` with `exiftool(-k).exe` and `exiftool_files\`: unpack it
     outside the checkout, rename `exiftool(-k).exe` to `exiftool.exe`, keep `exiftool_files`
     beside it, add that folder to `PATH` with
     `Add-Content -Path $env:GITHUB_PATH -Value <folder> -Encoding ascii` (in Windows PowerShell
     5.1, `>>` writes UTF-16, which the runner misreads), and in the next step run
     `exiftool -ver` so the log shows it.
  7. **cargo-audit:** `cargo install cargo-audit --locked --version <pinned>` (the newest release
     when the task is done). Only `~/.cargo/bin/cargo-audit.exe` is kept by `actions/cache`, keyed
     on that version (not the whole `bin` folder, which holds rustup's proxies); the install step
     runs only on a cache miss. `check.ps1`'s `cargo audit` step then runs instead of being
     `SKIPPED`.
  8. **Cargo cache:** `actions/cache` for `~/.cargo/registry/index`, `~/.cargo/registry/cache`,
     `~/.cargo/git/db` and `target`, the key made from the runner OS and the hashes of
     `rust-toolchain.toml` and `Cargo.lock`, with a `restore-keys` prefix without `Cargo.lock`.
     Old entries are left to GitHub's own eviction (10 GB per repository, shared with item 5).
  9. **The check:** `.\tools\check.ps1` without `-Ignored`, in Windows PowerShell (`shell:
     powershell`, the shell `docs/building.md` describes), so a failed step fails the job through
     the script's exit code. `PW_TESTDATA` is not set; the ignored tests do not run.
  10. **The report:** `test-output.txt` uploaded with `actions/upload-artifact` under the name
      `test-output`, with `if: always()`, so it is there when the check fails too.
  11. **`docs/building.md` §4:** one sentence after the paragraph on `.\tools\check.ps1`: the
      workflow `.github/workflows/ci.yml` runs the same check on GitHub, on a clean Windows
      machine, and keeps `test-output.txt` from the run. Nothing else in the file changes (what
      the first run finds goes into `docs/building.md` in step C2).
- **Tests to add:** none (no Rust code changes). The workflow cannot run before it is pushed;
  the first run is step C2, by the owner, and it is the real test of the YAML (no YAML parser is
  installed on the development machine, and none is installed for this task). Before reporting:
  every `uses:` names a full 40-character SHA, and the ExifTool SHA-256 in the workflow matches
  the zip downloaded once by hand (outside the repository, for example in `%TEMP%`, and deleted
  afterwards). Name in the report each pinned value (action SHAs and versions, ExifTool version,
  URL and SHA-256 and where the value came from, cargo-audit version).
- **Not in this task:** running the workflow, or pushing (the owner, C2); changes to
  `tools/check.ps1`, `.cargo/config.toml`, `vcpkg.json` or `rust-toolchain.toml`; a trigger on
  push (P3); the real-file tests in CI (they need free test files, plan R9); release builds or
  uploading the exe; Linux or macOS jobs; caching beyond items 5, 7 and 8; issue templates.
- **Rules:** no personal data in the workflow and no literal runner paths such as
  `C:\Users\runneradmin\…` (`check.ps1`'s `personal data` step searches the new file and would
  fail on them): use `~`, `${{ runner.temp }}` or `$env:USERPROFILE`; no text of the form
  `name@host.tld` (the step's e-mail pattern), the `@<sha>` of a `uses:` line is fine; nothing is
  unpacked, cached or downloaded inside the checkout besides `vcpkg_installed`, `target` and
  `test-output.txt` (all git-ignored). No secrets, the workflow needs none. No change to the build
  itself: if `check.ps1` or a `build.rs` would have to change for CI, stop and say why.
- **Done when:** `.\tools\check.ps1` is all OK, and `git status` shows, besides the changes that
  were in the worktree when the task started, only the two files above and this file's status and
  Log row.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Open | Added (public-release plan C1). Cold read (fresh agent, 2026-11-19): the build needs no LLVM, NASM or CMake beyond what vcpkg fetches; fixed in the text: the vcpkg cache saved per run (restore/save, `run_id`) since vcpkg's hashes include the compiler; the baseline commit fetched explicitly and `VCPKG_ROOT` set; `rustup toolchain install` first; the ExifTool zip layout, redirecting URL and where the SHA-256 comes from; `$env:GITHUB_PATH` written as ASCII; `CARGO_INCREMENTAL=0`; only `cargo-audit.exe` cached; the YAML check left to C2 (no PyYAML); runner paths and `@` text kept out for the `personal data` step; "Done when" allows the changes already in the worktree and this file's status and Log row |
