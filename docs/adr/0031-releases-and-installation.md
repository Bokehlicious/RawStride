# ADR 0031: Releases, version history and installation

- Status: **accepted by the owner (2026-11-20):** every question answered as proposed (§5). Nothing
  is built, installed or published by this ADR; each step in §6 becomes its own task or owner
  step, with its own approval and commit.
- Date: 2026-11-20
- Related:
  - ADR 0002 (licence `GPL-3.0-or-later`, ExifTool shipped unmodified §2.5) and its §4 list
    "Still open", which every release needs: the distribution folder, `THIRD-PARTY-NOTICES.txt`,
    SPDX headers (optional);
  - `docs/tasks/plans/public-release.md` §4 "Before the first binary release" and `backlog.md`
    (the licence notice in the program, IPTC's terms for the generated tables);
  - [distribution.md](../distribution.md): then the retired Tauri app's folder (rewritten for the
    portable zip by T228, 2026-11-20, those findings kept as its History), whose findings on
    runtime DLLs, a read-only program folder, signing and a clean PC are the starting point here;
  - ADR 0012 (the name RawStride, the exe `rawstride.exe`, crate names kept);
  - `.github/workflows/ci.yml` (T220, T222) and the rule that tooling changes go through a task;
  - `AGENTS.md` §2 and the push routine: `main` moves only by fast-forward, on the owner's yes.

## 1. Context

The repository is public since 2026-11-19, so anyone can build RawStride from source. Nobody
can yet download a program: there is no version history, no tag, no packaged folder and no
installer. `Cargo.toml` already has one version for the whole workspace (`0.1.0`,
`[workspace.package]`, `publish = false`), but nothing reads or shows it.

What is known from the Tauri app's distribution (`docs/distribution.md`, 2026-11-04):

- Copying a folder was the whole installation: the exe (LibRaw, LittleCMS, JasPer,
  libjpeg-turbo and zlib linked statically) and four Microsoft DLLs (`VCOMP140.DLL`,
  `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll`, `MSVCP140.dll`) loaded from the program folder. No
  Visual C++ Redistributable was needed.
- The program wrote nothing into its own folder, so it can live read-only under
  `C:\Program Files`. RawStride keeps its settings in `%LOCALAPPDATA%\RawStride\settings.txt`.
- The exe was unsigned: SmartScreen warns on first start, and the virus scanner delayed the first
  start of a new exe by 4–12 s.
- A release build held the build machine's Rust source paths (with the user name) in its panic
  messages (a check before T229, 2026-11-20, found that rustc already names the `.pdb` without its
  folder). The release profile still has `debug = "line-tables-only"` and no path remapping; the
  release workflow remaps the paths (RL6, `tools/release-build.ps1`).

RawStride itself has since retired WebView2 (eframe + egui) and will ship ExifTool in the
program folder (ADR 0002 §2.5; T120 looks there before PATH).

## 2. What a release is (the usual practice, for reference)

- **A version number** by [Semantic Versioning](https://semver.org): `MAJOR.MINOR.PATCH`. While
  `MAJOR` is 0 anything may change between minor versions; `1.0.0` says the program and the files
  it writes are considered stable.
- **A git tag** (`v0.1.0`) on the exact commit the release was built from.
- **A change log** (`CHANGELOG.md`, usually in the ["Keep a Changelog"](https://keepachangelog.com)
  form): per version, what was Added, Changed, Fixed or Removed, written for users; an
  `Unreleased` section collects changes until the next version.
- **A GitHub Release** for the tag: the change log's section as text, the files to download, and
  GitHub's own source archive of the tag (which, with the build instructions, is the
  corresponding source the GPL asks for).
- **Packages:** a portable zip (unpack and run), an installer (`.msi` from the WiX Toolset, or a
  `setup.exe` from Inno Setup), or an MSIX / winget package. Many projects start with a zip and
  add an installer later.
- **Checksums** (SHA-256) of every file, and ideally a **code signature**, so Windows' SmartScreen
  and virus scanners trust the exe.

## 3. Decision

1. **Version scheme:** SemVer, one version for the whole workspace (`[workspace.package]`), on
   `0.y.z` until the owner declares `1.0.0`. A new feature raises the minor version, a release
   with only fixes the patch version (RL-Q1).
2. **The first release `0.1.0`** is made when the release-only items are done (§6, RL1–RL6), not
   at a feature milestone; it is marked a pre-release on GitHub (RL-Q2).
3. **A change log** `CHANGELOG.md` in the repository root, in the "Keep a Changelog" form. The
   reviewer adds a line under `Unreleased` when a task with a change a user can notice is set to
   Done (a rule in `docs/tasks/README.md`); the history before `0.1.0` is one summary paragraph,
   not every task (RL-Q3).
4. **A portable zip first:** `RawStride-0.1.0-windows-x64.zip` with `rawstride.exe`, the four
   Microsoft DLLs, ExifTool (unmodified, ADR 0002 §2.5), `LICENSE`, `THIRD-PARTY-NOTICES.txt` and
   a short `README.txt`. Unpacking it is the installation; deleting the folder (and
   `%LOCALAPPDATA%\RawStride`) removes it (RL-Q4).
5. **An `.msi` installer later**, as its own step after the zip has been used: the WiX Toolset,
   installed per user (`%LOCALAPPDATA%\Programs\RawStride`, no administrator rights) with a Start
   menu shortcut, an entry in "Installed apps" and an upgrade that replaces the older version
   (RL-Q5). [WiX's licence and its maintenance fee for organisations: unchecked, checked in the
   step.]
6. **Code signing:** none for `0.1.0`; the release text says SmartScreen will warn and gives the
   SHA-256 checksums. After the first release, apply to SignPath Foundation (free signing for open
   source projects) or use Azure Trusted Signing (paid, identity check) (RL-Q6). [Both services'
   terms for an individual in Sweden: unchecked.]
7. **Built by CI, not on the owner's machine:** a release workflow on a pushed tag `v*` builds the
   release exe on a clean runner, packs the zip, writes `SHA256SUMS.txt` and creates a **draft**
   GitHub Release; the owner reads it and presses Publish (RL-Q7). The release build gets path
   remapping (no user folder in the exe) (RL-Q12).
8. **Tags only on `main`**, after the fast-forward, and pushed only on the owner's explicit yes
   (the push routine); a tag ruleset blocks deleting or moving `v*` tags (RL-Q8).
9. **The version in the program:** an "About RawStride" window (Help menu) with the version, the
   commit it was built from, the licence notice the GPL asks for (copyright, no warranty, where
   the licence and the source are) and the third-party notices. This is also the backlog item
   "Licence notice in the program" (RL-Q9).
10. **No update check over the network:** RawStride does not contact any server by itself
    (lightweight, no background services); the Releases page and the change log are where a user
    finds a new version (RL-Q10).
11. **Exe properties:** the exe carries a Windows version resource (version, product name,
    copyright) and an icon, so Explorer's Properties and Task Manager show them (RL-Q11).
12. **`docs/distribution.md`** is rewritten for RawStride when RL4 is done; the Tauri findings
    move to a short history section (RL-Q13).

## 4. What this costs [estimate, 2026-11-20, unmeasured]

- RL1–RL4 are tasks in the size the task rules allow (one layer, a handful of files each); the
  notices (RL3) are mostly a generated text file.
- The release workflow (RL6) is a second workflow file reusing T220's steps; a release run builds
  from the caches of `main`.
- The MSI (RL9) needs the WiX Toolset installed on the runner (and the owner's approval of the
  tool, `AGENTS.md` §3); no new Rust dependency.
- Signing (RL10) may cost money (Azure) or wait for an application (SignPath).

## 5. Questions (answered by the owner, 2026-11-20)

| # | Question | Answer |
|---|---|---|
| RL-Q1 | **Version scheme:** SemVer with one version for the workspace, `0.y.z` until `1.0.0`? | Yes; minor for features, patch for fixes only (as proposed) |
| RL-Q2 | **When is `0.1.0` made?** At a feature milestone, or when the release-only items are done? | When RL1–RL6 are done; marked pre-release on GitHub (as proposed) |
| RL-Q3 | **`CHANGELOG.md`**, and who writes it? | Yes, "Keep a Changelog"; the reviewer adds a line at each user-visible task's Done; before `0.1.0` one summary paragraph (as proposed) |
| RL-Q4 | **First package:** a portable zip, an MSI, or both at once? | The zip first (as proposed) |
| RL-Q5 | **The installer:** MSI (WiX) or `setup.exe` (Inno Setup); per user or for all users; when? | MSI with WiX, per user without administrator rights, after the zip has been used for a release or two (as proposed) |
| RL-Q6 | **Code signing** for `0.1.0`? | No; SHA-256 checksums and a note on SmartScreen; apply for free open-source signing after `0.1.0` (as proposed) |
| RL-Q7 | **Where the release is built:** by CI on a tag, or on the owner's machine? | CI on a pushed `v*` tag, as a draft release the owner publishes by hand (as proposed) |
| RL-Q8 | **Tags:** only on `main`, pushed on the owner's yes, protected by a ruleset? | Yes (as proposed) |
| RL-Q9 | **"About RawStride"** with version, commit, licence notice and third-party notices? | Yes, in the Help menu; it closes the backlog's licence notice item (an ADR 0002 §4 line) (as proposed) |
| RL-Q10 | **An update check** over the network? | No (as proposed) |
| RL-Q11 | **A version resource and icon** in the exe? | Yes; without a new dependency if the existing `build.rs` and the Build Tools' resource compiler can do it, otherwise asked in the task (as proposed) |
| RL-Q12 | **The release build without the build machine's paths** (`--remap-path-prefix`) and without the `.pdb` name? | Yes, in the release workflow (the local release profile unchanged, so latency measurements keep their line tables) (as proposed) |
| RL-Q13 | **`docs/distribution.md`** rewritten for RawStride? | Yes, at RL4 (as proposed) |

## 6. Steps

| Step | Who | Content | After |
|---|---|---|---|
| RL1 | task | "About RawStride" (RL-Q9): version from `CARGO_PKG_VERSION`, the commit from the build, the licence notice (ADR 0002 §4 has the line, 2026-11-20) | now |
| RL2 | task | Version resource and icon in `rawstride.exe` (RL-Q11) | now |
| RL3 | task | `THIRD-PARTY-NOTICES.txt` (ADR 0002 §4): the Rust packages from `cargo metadata`, LibRaw (its version and source), LittleCMS, JasPer, libjpeg-turbo, zlib, the fonts, the MSVC runtime, ExifTool and its Perl, IPTC's attribution for the generated tables | now |
| RL4 | task | `tools/package-dist.ps1`: the zip of §3.4 from a release build, with ExifTool 13.59 checked by SHA-256 as in CI; `docs/distribution.md` rewritten (RL-Q13) | RL3 |
| RL5 | owner | The zip run on a clean PC or in Windows Sandbox: starts, renders a RAW, writes a rating through the shipped ExifTool, leaves the program folder unchanged; done 2026-11-20 in a new Windows account on the owner's PC (no other PC; no Sandbox on Windows 11 Home), all four OK | RL4 |
| RL6 | task | `CHANGELOG.md` with the `0.1.0` section and the rule in `docs/tasks/README.md` (RL-Q3); `.github/workflows/release.yml` on `v*` tags (RL-Q7, RL-Q12); done (T229), its test run on `main` checked OK 2026-11-20 | RL4 |
| RL7 | owner + reviewer | `0.1.0`: the `CHANGELOG.md` section renamed, tag on `main`, the workflow's draft read, published as a pre-release; the tag ruleset (RL-Q8); `README.md`'s status line; steps in `docs/distribution.md`, "Making a release"; done 2026-11-20: `v0.1.0` published as a pre-release, tag ruleset "release tags" active | RL5, RL6 |
| RL8 | owner | Code signing applied for (RL-Q6) | RL7 |
| RL9 | task | The MSI (RL-Q5) | RL7 and the owner's go |

Placed by the owner (2026-11-20) as order 1d of `docs/tasks/plans/adr-0018-0025-steps.md`,
before ADR 0024 O1; RL8 and RL9 do not hold O1 back. RL1–RL7 are done (2026-11-20); RL8 and RL9
were moved by the owner on 2026-11-21 to order 2c of that plan, after ADR 0017's rest (U10b, U8,
U13), so they are not lost.

## 7. Not included

- Linux and macOS packages (RawStride is Windows only today; backlog).
- winget, MSIX or the Microsoft Store.
- Automatic updates, crash reports or any telemetry.
- Publishing the crates to crates.io (`publish = false` stays).
- SPDX headers in the source files (ADR 0002 §4, optional, decided separately).

## 8. Consequences

- Every release can be traced to a tag, a CI run and a change log section; the source the GPL
  asks for is the tag's archive on GitHub.
- The reviewer's Done step gains one line in `CHANGELOG.md` for user-visible tasks.
- Until signing is in place, every new version meets SmartScreen's warning on first start.
- ADR 0002 §4's list is worked off by RL1, RL3 and RL4.

## Revision history

| Revision | Date | Changes |
|---|---|---|
| 1 | 2026-11-20 | Proposed by the reviewer after the repository became public; waits for RL-Q1–RL-Q13 |
| 2 | 2026-11-20 | Accepted: the owner approved RL-Q1–RL-Q13 as proposed; ADR 0002 §4 gets the About window's line |
