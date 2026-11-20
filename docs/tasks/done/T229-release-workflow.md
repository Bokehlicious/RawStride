# T229. ADR 0031 RL6: CHANGELOG.md, tools/release-build.ps1 and the release workflow on v* tags (tools, CI, docs) — Done

- **Why:** ADR 0031 (accepted by the owner 2026-11-20), step RL6, §3.3, §3.6, §3.7 and RL-Q3,
  RL-Q7, RL-Q12: a release is built by CI on a pushed `v*` tag, not on the owner's machine; the
  release exe has no build-machine paths and no `.pdb` folder in it; the run packs the zip
  (`tools/package-dist.ps1`, T228), writes `SHA256SUMS.txt` and creates a **draft** GitHub
  Release that the owner reads and publishes by hand (RL7). The change log `CHANGELOG.md` holds
  what each version brings. Two leftovers come with this step: the release run regenerates
  `THIRD-PARTY-NOTICES.txt` and fails if it differs from the committed file (T227), and the zip
  is not byte-for-byte reproducible (entry times), so `SHA256SUMS.txt` is of the zip the run
  uploads, never of a local one (T228).
- **What the reviewer checked (2026-11-20):**
  - `.github/workflows/ci.yml` (T220, T222): its steps for long paths, checkout, the Rust
    toolchain from `rust-toolchain.toml`, the cargo and vcpkg caches, the vcpkg clone at
    `vcpkg.json`'s baseline and its install into `.\vcpkg_installed`, and ExifTool; every action
    is one of GitHub's own, pinned to a full commit SHA with its version in a comment; every
    `run:` uses Windows PowerShell and checks `$LASTEXITCODE` after a native program.
  - `tools/package-dist.ps1` (T228) has `-NoBuild` (uses `target\release\rawstride.exe`, prints
    its time) and downloads and checks ExifTool itself; it prints the zip's SHA-256 but writes no
    checksum file. It needs `vswhere.exe`, `dumpbin.exe` and the redistributable folder (VC143 on
    a runner with Visual Studio 2022 is handled).
  - `tools/third-party-notices.ps1` (T227) needs cargo-about 0.9.3
    (`cargo install cargo-about --locked --version 0.9.3 --features cli`) and the vcpkg packages'
    copyright files under `vcpkg_installed\x64-windows-static-md\share\`; it runs cargo with
    `--locked --offline`, so every package in `Cargo.lock` must already be in the cargo registry
    (a `cargo build` on a clean runner downloads only those for Windows; `cargo fetch --locked`
    downloads them all). Two runs give the same bytes. `THIRD-PARTY-NOTICES.txt` is stored with
    LF (`.gitattributes`: `* text=auto eol=lf`).
  - `[profile.release]` in `Cargo.toml` has `debug = "line-tables-only"` and is not changed
    (RL-Q12: the local release profile keeps its line tables for latency measurements). The
    remapping is given only in the release run, through the environment.
  - `crates/pw-loupe/build.rs` takes the commit from `git rev-parse HEAD` and the icon/version
    resource from the Windows SDK's `rc.exe`; both work on a runner (a tag checkout is a detached
    HEAD, which `git rev-parse` handles).
  - The workspace version is `0.1.0` (`[workspace.package]`); `pw-loupe`'s version is read the way
    `package-dist.ps1` reads it (`cargo metadata --format-version 1 --no-deps --locked --offline`).
  - The GitHub CLI `gh` is installed on GitHub's Windows runners and creates a release with
    `gh release create`.
- **Crates:** none (no Rust code changes). **Files:** `CHANGELOG.md` (new, point 1),
  `tools/release-build.ps1` (new, points 2–4), `.github/workflows/release.yml` (new, points 5–7),
  `docs/distribution.md` (point 8), `tools/README.md` (point 8), `docs/README.md` (point 8), and
  this task's own file (status and log).
- **What:**
  1. **`CHANGELOG.md`** in the repository root, LF line endings, English, in the
     ["Keep a Changelog" 1.1.0](https://keepachangelog.com/en/1.1.0/) form, exactly this text:
     ```markdown
     # Changelog

     What changes in each version of RawStride, written for its users. The form follows
     [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version numbers follow
     [Semantic Versioning](https://semver.org/spec/v2.0.0.html): while the first number is 0,
     anything may change between versions, the files RawStride writes included.

     Downloads are on the [Releases page](https://github.com/Bokehlicious/RawStride/releases).

     ## [Unreleased]

     The first release, for Windows 10 and 11 (x64), as a portable zip. RawStride copies photos
     from a memory card to a destination and an optional backup, with folder and file name
     templates; shows them in a contact sheet and a fast loupe (RAW files from their embedded
     preview or rendered with LibRaw, colour managed); sets star ratings, reject and colour
     labels in XMP the way other photo programs read them; and edits IPTC metadata (Core and
     Extension in XMP, and the older IIM), into an XMP sidecar for a proprietary RAW file and
     safely into DNG, JPEG and TIFF files. Folders open in tabs, with filters and multiple
     selection; images can be moved, deleted to the Recycle Bin and opened in another program.
     ExifTool and the Microsoft Visual C++ runtime are included; the program is not signed yet.
     ```
     (At RL7 the reviewer renames `[Unreleased]` to `[0.1.0] - <date>` and adds an empty
     `[Unreleased]` above it; not this task.)
  2. **`tools/release-build.ps1`** (Windows PowerShell 5.1, run from the repository root, ASCII
     only, a comment header naming ADR 0031 RL6, T229, what it does and how to run it;
     `$ErrorActionPreference = 'Stop'`; every native program run through one helper that sets
     `Continue` around the call and checks `$LASTEXITCODE`, as `Invoke-Native` in
     `tools/package-dist.ps1`). Parameters:
     - `-Section <name>` (required): the `CHANGELOG.md` section whose text becomes the release
       notes: `Unreleased`, or a version such as `0.1.0`;
     - `-SkipNotices`: do not run the notices check (step 3 below), for a local run without
       cargo-about.

     The native-program helper takes a list of allowed exit codes (default only 0) and returns
     the exit code; `git diff` and `git merge-base` use it that way, since their exit code 1 is
     an answer, not an error.

     It does, in this order, and stops with a clear message at the first failure:
     1. Read `pw-loupe`'s version and the `workspace_root` from one call of `cargo metadata` as
        `package-dist.ps1` does (the repository root is taken from `workspace_root`, as cargo and
        rustc spell it, not from `$PSScriptRoot`, since `--remap-path-prefix` matches with case).
        If `-Section` is not `Unreleased`, stop unless it equals that version.
     2. Write the release notes (point 4) to `target\dist\release-notes.md`.
     3. Unless `-SkipNotices`: (a) `cargo fetch --locked`; (b) run `.\tools\third-party-notices.ps1`;
        (c) `git diff --exit-code --stat HEAD -- THIRD-PARTY-NOTICES.txt` (against `HEAD`, not
        the index), its output shown; exit code 0 passes, 1 stops with
        "THIRD-PARTY-NOTICES.txt is out of date: run .\tools\third-party-notices.ps1 and commit the
        result" (the regenerated file is left in place so the difference can be read), any other
        code is an error.
     4. Build the release exe with the paths remapped:
        `cargo build --release --locked -p pw-loupe`, with the environment variable
        `CARGO_ENCODED_RUSTFLAGS` set for that call only (restored afterwards) to these flags,
        joined by the character `0x1F` (so a path with a space needs no quoting):
        `--remap-path-prefix=<repository root>=rawstride`,
        `--remap-path-prefix=<CARGO_HOME>=cargo` (`$env:CARGO_HOME`, or `%USERPROFILE%\.cargo`
        when it is not set), and `-Clink-arg=/PDBALTPATH:%_PDB%` (the exe names only
        `rawstride.pdb`, not its folder; the cold read found that today's rustc already does
        this, so the flag is a safeguard). If `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` is already
        set, stop (they would be replaced silently); after the build the variable is removed
        again (also when the build fails). The cold read found the build-machine paths in today's
        exe are only cargo registry sources and OUT_DIR files under `target\`, both covered by
        these two prefixes. On a runner vcpkg builds under `RUNNER_TEMP`, which is not checked
        (it holds no user folder).
     5. **Check the exe:** read `target\release\rawstride.exe`'s bytes and stop if any of these
        is found in them, as ASCII and as UTF-16LE, compared without regard to case: the
        repository root, the cargo home folder, `%USERPROFILE%`, and the same three written with
        `/` instead of `\`. The message names which one was found, never the path's value (the
        log must hold no user folder). Then run `dumpbin /nologo /headers` on the exe (found as in
        `package-dist.ps1`) and stop unless the line holding `Format: RSDS` ends, after its last
        `, `, exactly in `rawstride.pdb` (the line looks like
        `... cv ... Format: RSDS, {GUID}, 1, rawstride.pdb`).
     6. Run `.\tools\package-dist.ps1 -NoBuild`.
     7. Write `target\dist\SHA256SUMS.txt`: one line `<sha-256 lower case>  <file name>` (two
        spaces, the `sha256sum` form, LF, UTF-8 without BOM) for the zip
        `RawStride-<version>-windows-x64.zip`. Print the file's content.
  3. **No other changes to the build:** `Cargo.toml`, `.cargo/`, `tools/package-dist.ps1` and
     `tools/third-party-notices.ps1` are not changed. A local run of this script rebuilds
     `target\release\` with the remapping; a later plain `cargo build --release` rebuilds it back
     (stated in the script's header).
  4. **The release notes** (`target\dist\release-notes.md`, LF, UTF-8 without BOM, written with
     `[System.IO.File]::WriteAllText`): `CHANGELOG.md` is read with
     `[System.IO.File]::ReadAllText` as UTF-8 and its line endings made LF. The section is the
     lines after the heading that is exactly `## [<Section>]` or starts with
     `## [<Section>] - ` (compared as plain text; a regex, if used, gets `<Section>` through
     `[regex]::Escape`), up to the next line starting with `## `, or a link-reference line
     (`[...]: ` at the start of a line), or the end of the file. Trailing whitespace is removed
     from each line and blank lines from the start and end. Stop if the section is missing or
     empty, or if two headings match. Then one blank line, then this fixed text (in the script as
     a single-quoted here-string, whose CRLF from the `.ps1` checkout is made LF, `<version>`
     replaced), and the file ends with one LF:
     ```markdown
     ## Download

     `RawStride-<version>-windows-x64.zip`: unpack it anywhere and start `rawstride.exe`; the zip's
     `README.txt` says more. Windows 10 or 11, x64.

     The program is not signed yet, so Windows SmartScreen may warn the first time it is started
     ("Windows protected your PC": choose "More info", then "Run anyway"). Compare the zip's
     SHA-256 checksum with `SHA256SUMS.txt` first, for example with
     `Get-FileHash RawStride-<version>-windows-x64.zip` in PowerShell.

     The source code of this version is the tag's source archive below; how to build it is in
     `docs/building.md`.
     ```
  5. **`.github/workflows/release.yml`**, a header comment like `ci.yml`'s (what it does, ADR
     0031 RL6, T229, that it creates only a draft that the owner publishes, and that it runs no
     tests: a tag is put only on a `main` commit whose CI run is green, RL7). Name `Release`.
     Triggers: `push` of tags `v*`, and `workflow_dispatch` (a test run on any branch that makes
     the zip but no release) with one input `section` (a string, default `Unreleased`; after RL7
     the `Unreleased` section may be empty, and a test run then names a version). A tag run is
     `github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')`; every "on a tag
     only" below means exactly that, so a manual run on a tag ref never makes a release.
     `permissions: contents: read` at the top. `concurrency` by
     `${{ github.workflow }}-${{ github.ref }}`, not cancelling a running release
     (`cancel-in-progress: false`). The tag name and the input reach scripts only through
     `env:` (never pasted into `run:` with `${{ }}`); the leading `v` is stripped in PowerShell.
  6. **Job `build`** (`windows-latest`, `timeout-minutes: 120`, `shell: powershell`, the same
     `env` as `ci.yml` without `CARGO_AUDIT_VERSION`, plus `CARGO_ABOUT_VERSION: "0.9.3"`):
     - the steps of `ci.yml` for long paths, checkout (with `fetch-depth: 0`, so `origin/main` is
       there), the Rust toolchain and vcpkg (clone, bootstrap, restore the binary cache, install)
       are copied with the same pinned SHAs. ExifTool is **not** installed (`package-dist.ps1`
       downloads and checks it itself). The cargo cache and the vcpkg binary cache are **only
       restored** (`actions/cache/restore`; the cargo cache with exactly `ci.yml`'s `path` list,
       key and restore key, since the path list is part of the cache's identity), never saved,
       so a tag run makes no such cache entries; the header comment says its `target` part does
       not help (the remapping flags rebuild everything) and only the registry part does;
     - cargo-about: a cache keyed on its version like cargo-audit's in `ci.yml` (restored and
       saved, `actions/cache` as there, path `~/.cargo/bin/cargo-about.exe`), installed with
       `cargo install cargo-about --locked --version $env:CARGO_ABOUT_VERSION --features cli`
       when the cache missed;
     - **on a tag only**, a step that stops unless the commit is on `main`
       (`git merge-base --is-ancestor HEAD origin/main`, its exit code handled explicitly: 0 on
       `main`, 1 not on `main` with a clear message, other an error; GitHub's `powershell` shell
       otherwise ends the step on the exit code without a message) and the tag is `v` +
       `pw-loupe`'s version (read as in point 2.1);
     - `.\tools\release-build.ps1 -Section <s>`, where `<s>` is the version (the tag name without
       `v`) on a tag and the input `section` on a test run;
     - upload `target/dist/RawStride-*-windows-x64.zip`, `target/dist/SHA256SUMS.txt` and
       `target/dist/release-notes.md` as the artifact `release` (`actions/upload-artifact` at
       `ci.yml`'s pinned SHA, `if-no-files-found: error`); `test-output.txt` is not involved.
  7. **Job `release`** (`needs: build`, on a tag only, `windows-latest`, `shell: powershell`,
     `timeout-minutes: 15`, `permissions: contents: write`, no checkout): download the artifact
     `release` (`actions/download-artifact` at v8.0.2, its newest release, pinned to the full
     commit SHA of that tag, found with
     `git ls-remote https://github.com/actions/download-artifact refs/tags/v8.0.2` and named in a
     comment; with `name` given it unpacks into the workspace without a subfolder), then
     `gh release create <tag> --repo <repository> --verify-tag --draft [--prerelease]
     --title "RawStride <version>" --notes-file release-notes.md
     RawStride-<version>-windows-x64.zip SHA256SUMS.txt` with `GH_TOKEN: ${{ github.token }}`;
     the tag, version and repository come through `env:`, the zip is named from the version
     (no wildcard: neither PowerShell nor `gh` expands it here), and the arguments are built as
     a PowerShell array so `--prerelease` is added only while the version starts with `0.`
     (ADR 0031 §3.2). `$LASTEXITCODE` is checked. If a release for the tag already exists the
     step fails; that is accepted (the owner deletes the draft and runs it again).
  8. **Docs:** `docs/distribution.md` "Making it": the last point's "(RL6)" made concrete, and a
     new section **"Making a release"** after it: the steps of RL7 in short (CHANGELOG section
     renamed; `main` fast-forwarded and its CI green; the tag `v<version>` on `main`, pushed on
     the owner's yes; the workflow's draft with the zip, `SHA256SUMS.txt` and the notes; the owner
     reads and publishes), the test run (`workflow_dispatch` on any branch, the artifact
     `release`), what `release-build.ps1` checks, and that a local run of it rebuilds
     `target\release\`. `tools/README.md`: a line for `release-build.ps1`. `docs/README.md`: a
     line for `CHANGELOG.md` (in the root) where the other root files are listed, or in the list
     with `distribution.md` if there is no such place.
- **Tests to add:** none in Rust. Runs, shown in the report (the log row sums them up in a
  sentence each; no output with a user folder in either, so cargo or cargo-about lines naming
  `%USERPROFILE%\.cargo\...` are left out or shortened):
  - `.\tools\release-build.ps1 -Section Unreleased` with cargo-about installed (the reviewer has
    it): the notices check passing, the build, the exe check passing, the PDB line from
    `dumpbin`, the zip, `SHA256SUMS.txt`'s content and `release-notes.md`'s content;
  - runs that must stop: `-Section 9.9.9` (not the version); `-Section 0.1.0` (no such section
    yet); and the notices check after one temporary line added at the end of
    `tools/third-party-notices.hbs` (a changed input, so the regenerated file differs; the agent
    may make this one temporary edit, and afterwards restores both files with
    `git checkout -- tools/third-party-notices.hbs THIRD-PARTY-NOTICES.txt` and shows that
    `git status` lists neither).

  The workflow itself is first run by the owner's test run (hand check). After the runs,
  `cargo build --release -p pw-loupe` rebuilds the normal release exe.
- **Not in this task:** the tag, the release, renaming the CHANGELOG section, the tag ruleset and
  `README.md`'s status line "not released" (RL7, owner and reviewer); the rule in
  `docs/tasks/README.md` for adding CHANGELOG lines at Done and ADR 0031's RL7 row (the reviewer
  at Done, also ADR 0031 §1's line that a release build "named its `.pdb`", which the cold read
  found no longer true); signing (RL8); the MSI (RL9); `ci.yml`, `tools/check.ps1`, `Cargo.toml`, any Rust
  code, `tools/package-dist.ps1`, `tools/third-party-notices.ps1`, `THIRD-PARTY-NOTICES.txt`.
- **Rules:** `docs/tasks/README.md` rules 1–12 (rule 13 does not apply: no text in the program).
  No path into a user folder and no e-mail address in any file or in the log (the personal-data
  step of `check.ps1`). Do not push, tag or run the workflow; the owner does that.
- **Hand check (owner, before RL7):** GitHub offers "Run workflow" only for a workflow that is on
  the default branch, so `main` is first fast-forwarded and pushed with this task (the owner's
  yes, the push routine). Then the workflow started by hand (Actions ▸ Release ▸ Run workflow,
  on `main`, section `Unreleased`) goes green; its artifact
  `release` holds the zip, `SHA256SUMS.txt` (whose value matches the zip) and
  `release-notes.md` with the `Unreleased` text and the Download section; the zip unpacks and
  `rawstride.exe` starts.
- **Done when:** `.\tools\check.ps1` is all OK, the runs in "Tests to add" are in the log, and
  `git status` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Written by the reviewer from ADR 0031 RL6 / §3.3, §3.7, RL-Q3, RL-Q7, RL-Q12 and the plan row's leftovers from T227 and T228. Cold read (fresh agent, 2026-11-20): can be done with the named files; it scanned today's release exe (build-machine paths only from the cargo registry and OUT_DIR, both remapped; no C++ or vcpkg paths; the `.pdb` already named without its folder by rustc). Fixed in the text: the hand check needs the workflow on `main` first (GitHub's rule for "Run workflow"); the notices failure test changes an input (`third-party-notices.hbs`), not the output, which is regenerated; the repository root from `cargo metadata`'s `workspace_root` (remapping matches with case); `git diff` against `HEAD` and exit code 1 handled as an answer; the PDB line's form; the notes read as UTF-8, LF, link-reference lines end a section, the here-string's CRLF made LF; a `section` input for test runs; "on a tag" means a push event; the tag only through `env:`; the release job on Windows PowerShell with the zip named, `--prerelease` as an array element; download-artifact v8.0.2; ExifTool step dropped; the cargo-about cache saved; `concurrency` with the workflow name; the report holds the run output, the log row sums it up; ADR 0031 §1's `.pdb` line left to the reviewer at Done |
| 2026-11-20 | Ready for review | Task agent. CHANGELOG.md, tools/release-build.ps1, .github/workflows/release.yml written; docs/distribution.md ("Making it" made concrete, new "Making a release"), tools/README.md and docs/README.md updated. Runs: `-Section 9.9.9` stopped ("neither 'Unreleased' nor pw-loupe's version 0.1.0"); `-Section 0.1.0` stopped ("CHANGELOG.md has no section '## [0.1.0]'"); with one temporary line at the end of tools/third-party-notices.hbs the notices check stopped (`git diff --stat` 2 insertions, "THIRD-PARTY-NOTICES.txt is out of date: ..."), both files then restored with `git checkout --` and absent from `git status`; `-Section Unreleased` passed: notices up to date, remapped build, no repository root, cargo home or user folder in the exe, PDB line ending in `rawstride.pdb`, zip with 517 entries, SHA256SUMS.txt matching `sha256sum` of the zip, release-notes.md with the Unreleased text and the Download section (LF, one final LF). The plain release exe rebuilt afterwards holds the repository root and `.cargo` paths, so the check does find them. Beyond the text: the script removes a stale SHA256SUMS.txt at its start; the native helper has an optional `-Output` [ref] for cargo metadata, vswhere and dumpbin. The workflow is not run (owner's hand check); its YAML was not parsed by a tool (no YAML parser installed). check.ps1 all OK: 1967 passed, 72 ignored |
| 2026-11-20 | Done | Reviewer. Read against the brief: all points as written (steps in order, exit codes, `CARGO_ENCODED_RUSTFLAGS` removed in `finally`, the path check prints labels only, notes as UTF-8 LF); the workflow's pinned SHAs match `ci.yml`, download-artifact v8.0.2's SHA checked with `git ls-remote`; the cargo cache restore has `ci.yml`'s exact path list and keys; `cargo metadata --no-deps --locked --offline` checked to work with an empty cargo home (the tag check runs before any fetch). The agent's extras (stale `SHA256SUMS.txt` removed, `-Output` [ref]) accepted. `target\dist\SHA256SUMS.txt` matches `sha256sum` of the zip; `release-notes.md` read. Noted, not a leftover: the cargo-about cache saved by a tag run is visible only to that tag (GitHub's cache scope), so it helps a re-run; one saved by a test run on `main` helps later tags. Not run: the workflow itself and a YAML parser, both covered by the owner's hand check (the plan's RL6 row). Reviewer at Done: the CHANGELOG rule in `docs/tasks/README.md`; ADR 0031 §1's `.pdb` line and RL7 row; the plan's RL6 and RL7 rows. No CHANGELOG line (no user-visible change). check.ps1 all OK: 1967 passed, 72 ignored; no clippy warnings |
| 2026-11-20 | Hand check OK | `main` fast-forwarded and pushed (b6cde6f); CI push runs green on both branches. The owner started Actions ▸ Release ▸ Run workflow on `main` (section `Unreleased`, run 44764700899, commit b6cde6f): job "Release build on Windows" succeeded (the tag check skipped as it should on a manual run), "Draft GitHub Release" skipped, no release made (`gh release list` empty). The reviewer downloaded the artifact `release`: `RawStride-0.1.0-windows-x64.zip` (517 entries), `SHA256SUMS.txt` whose value matches `Get-FileHash` of the zip, `release-notes.md` with the `Unreleased` text and the Download section; the unpacked `rawstride.exe` started and showed its window. The YAML is thereby parsed by GitHub. Docs only, check.ps1 not run |
