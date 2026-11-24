# T306. ADR 0031 RL9: the MSI in the release build and workflow, docs/distribution.md (tools, CI, docs) — Blocked: waits for T305

- **Why:** ADR 0031 RL9, RL9-Q1 and RL9-Q8 (owner, 2026-11-24): every release has both the zip
  and the MSI, both in `SHA256SUMS.txt`, built by the release workflow on a clean runner. T305
  made `tools/package-msi.ps1`, which makes and checks
  `target\dist\RawStride-<version>-windows-x64.msi` from the zip's program folder; this task
  runs it in the release build, attaches the MSI to the draft release, and describes the MSI in
  `docs/distribution.md`.
- **What the reviewer checked (2026-11-24):**
  - `tools/release-build.ps1` (T229) step 6 runs `package-dist.ps1 -NoBuild`, step 7 writes
    `SHA256SUMS.txt` for the zip alone, and step 2 writes the release notes with a fixed Download
    text that names only the zip. It removes an old `SHA256SUMS.txt` first, so no checksum file
    stands next to files from another run.
  - `.github/workflows/release.yml` runs `release-build.ps1`, uploads the zip, `SHA256SUMS.txt`
    and `release-notes.md` as the artifact `release`, and creates the draft with
    `gh release create` and the zip and `SHA256SUMS.txt` as files. Only GitHub's own actions,
    each pinned to a full commit SHA. The runner `windows-latest` has the .NET SDK; WiX is
    installed with `dotnet tool install --global wix --version 7.0.1` (RL9-Q1; [unverified that the
    tools folder is on the runner's PATH in the next step: check it, and add
    `%USERPROFILE%\.dotnet\tools` to `GITHUB_PATH` if it is not]).
  - `README.md`'s "Planned" list has "Signed releases and an installer (MSI)." and its install
    steps name only the zip: the reviewer changes `README.md` at Done, not this task.
- **Crates:** none. **Files:** `tools/release-build.ps1` (points 1–3), `.github/workflows/release.yml`
  (point 4), `docs/distribution.md` (point 5), `tools/README.md` (point 6), and this task's own
  file (status and log).
- **What:**
  1. **`release-build.ps1`, a new step after the zip:** runs `tools\package-msi.ps1` (no
     `-Version`: the release's version) and stops if
     `target\dist\RawStride-<version>-windows-x64.msi` is not there afterwards. The header's
     list of steps names it. An old MSI of the same name is removed at the start, as the old
     checksum file is.
  2. **`SHA256SUMS.txt`** gets two lines in `sha256sum` form, the zip's and the MSI's, in that
     order.
  3. **The release notes' Download text** names both: the MSI first ("installs RawStride for the
     current user in `%LOCALAPPDATA%\Programs\RawStride`, with a Start menu shortcut; no
     administrator rights; close RawStride before installing a new version over an older one,
     which it replaces; uninstall it in Installed apps,
     which keeps your settings in `%LOCALAPPDATA%\RawStride`"), then the zip as today; SmartScreen's
     warning and the checksum check for both (`Get-FileHash` with each file name).
  4. **`release.yml`:** a step before "Run tools/release-build.ps1" that installs WiX 7.0.1 as a
     .NET tool (the version in one `env` value, `WIX_VERSION: "7.0.1"`, with a comment that it
     must match `$wixVersion` in `tools/package-msi.ps1`), checks `wix --version` and makes sure
     `wix` is found in the next steps (the PATH note above). No new action. The artifact
     `release` also holds `target/dist/RawStride-*-windows-x64.msi`; the draft release attaches
     the MSI too (`$msi` beside `$zip` in `gh release create`). The header comment names the MSI
     (today "The three files"; now four).
  5. **`docs/distribution.md`:** a section "The installer (MSI)" after "The zip": what it
     installs and where, no administrator rights, the Start menu shortcut, no dialogs, upgrade and
     the downgrade message, an equal version replaces the installed one, close RawStride before
     upgrading (with no dialogs Windows may otherwise ask for a restart, RL9-Q11), no `README.txt`
     in the installed folder (RL9-Q9), uninstall in Installed apps keeping `%LOCALAPPDATA%\RawStride` and
     never touching images or sidecars, that a zip copy and an installed copy share the settings,
     not signed (RL9-Q3–RL9-Q11). "Making it" gains `.\tools\package-msi.ps1` (after
     `package-dist.ps1`; needs the .NET SDK and WiX 7.0.1, its EULA accepted by the script; checks
     its result without installing). "Making a release" and the `release-build.ps1` list name the
     MSI and the two checksum lines. "Installing and removing" names both ways. Also: the title
     and the intro (today "the portable zip" and "there is no installer and nothing is
     registered") name both; "What the PC needs" holds for the MSI too; "Not signed" speaks of
     both files' checksums.
  6. **`tools/README.md`:** the `release-build.ps1` line names the MSI.
- **Tests to add:** none in Rust. The log shows a local run of
  `.\tools\release-build.ps1 -Section 0.4.0 -SkipNotices` (`[Unreleased]` is empty since 0.4.0,
  and `-Section Unreleased` stops on an empty section; the end of its output,
  `SHA256SUMS.txt` with two lines, the Download text of `release-notes.md`), and
  `git diff .github/workflows/release.yml`. The workflow itself is run by the reviewer after
  Done (a test run, Actions ▸ Release ▸ Run workflow, on the pushed branch, on the owner's yes):
  its artifact must hold the zip, the MSI, `SHA256SUMS.txt` with both, and the notes.
- **Not in this task:** `tools/package-msi.ps1`, `tools/msi/RawStride.wxs`,
  `tools/package-dist.ps1` (T305; if one must change, stop and say why); `ci.yml`,
  `tools/check.ps1`; `README.md`, `CHANGELOG.md` (the reviewer at Done); signing (RL8); any Rust
  code or `Cargo.toml`.
- **Rules:** `docs/tasks/README.md` rules 1–12 (rule 13 does not apply). Workflow: only GitHub's
  own actions, pinned to a full SHA (none new is needed); no secret; `permissions` unchanged.
  No path into a user folder and no e-mail address in any file.
- **Hand check:** the test run of the workflow above (the reviewer, with the owner's yes to push);
  the MSI itself was hand-checked in T305.
- **Done when:** `.\tools\check.ps1` is all OK, the local release build of the log has made both
  files and the two checksum lines, and `git status` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added: waits for T305 (chain T305 → T306) |
