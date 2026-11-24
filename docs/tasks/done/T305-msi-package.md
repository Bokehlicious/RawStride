# T305. ADR 0031 RL9: the MSI, made by tools/package-msi.ps1 from the zip's program folder (tools) — Done

- **Why:** ADR 0031 (accepted 2026-11-20), step RL9, §3.5, RL-Q5 and RL9-Q1–RL9-Q8 (approved by
  the owner 2026-11-24, ADR 0031 §5): besides the portable zip, every release gets an installer,
  `RawStride-<version>-windows-x64.msi`, made with the WiX Toolset 7.0.1. It installs per user
  without administrator rights, puts one shortcut in the Start menu, replaces an older version and
  refuses a downgrade. This task makes the MSI locally from the folder `tools/package-dist.ps1`
  already makes; T306 (Blocked until this task is Done) puts it into the release build, the
  release workflow and `docs/distribution.md`.
- **What the reviewer checked (2026-11-24):**
  - `tools/package-dist.ps1` (T228) makes `target\dist\RawStride-<version>-windows-x64\` with
    exactly: `rawstride.exe`, `MSVCP140.dll`, `VCOMP140.DLL`, `VCRUNTIME140.dll`,
    `VCRUNTIME140_1.dll`, `exiftool.exe`, the folder `exiftool_files` (many files in subfolders),
    `LICENSE`, `THIRD-PARTY-NOTICES.txt` and `README.txt`. The MSI installs exactly these files,
    unchanged. RawStride finds `exiftool.exe` in its own folder (`pw-metadata` `exiftool.rs`), and
    writes nothing into the program folder (`docs/distribution.md`, RL5), only into
    `%LOCALAPPDATA%\RawStride\`.
  - The program's icon is `crates/pw-loupe/assets/rawstride.ico` (RL2). The version and the
    repository URL are `pw-loupe`'s, read with `cargo metadata` as `package-dist.ps1` reads them.
  - The reviewer installs the .NET 10 SDK and WiX 7.0.1 (`dotnet tool install --global wix
    --version 7.0.1`) on the owner's PC before this task is set to Open (RL9-Q2); the agent installs
    nothing. Installed 2026-11-24: `wix --version` gives `7.0.1+642f1a0`. WiX 7 refuses every
    command until its EULA is accepted (error WIX7015); `wix --help` offers `--acceptEula`
    ("Accept the EULA for this invocation without creating an acceptance file"), and `wix eula
    accept wix7` would accept it for good. The script uses the per-run option only (ADR 0031
    §3.5), never `wix eula accept`. Check the exact form (with or without the id `wix7`) with
    `wix build --help` and name it in the log.
- **Crates:** none (no Rust code changes). **Files:** `tools/msi/RawStride.wxs` (new, point 1),
  `tools/package-msi.ps1` (new, points 2–4), `tools/README.md` (point 5), and this task's own file
  (status and log).
- **What:**
  1. **`tools/msi/RawStride.wxs`** (WiX 7 source, ASCII only, a comment header naming ADR 0031
     RL9 and this task):
     - one `Package`: `Name="RawStride"`, `Manufacturer="RawStride"` (RL9-Q7: no person's name),
       `Version="$(var.Version)"`, `Scope="perUser"` (RL9-Q3), and the **UpgradeCode
       `9DA7D7C8-8D15-4CC5-9C80-2D12BB33937B`**, with a comment that it must never change (every
       later version finds the installed one by it). The cabinet is embedded in the `.msi` (one
       file to download).
     - `MajorUpgrade` with `DowngradeErrorMessage="A newer version of RawStride is already
       installed."` (RL9-Q5) and `AllowSameVersionUpgrades="yes"` (RL9-Q10): a newer or equal
       version replaces the installed one, so Installed apps always has one RawStride (the ICE61
       warning this gives is expected; name it in the log).
     - The install folder `[LocalAppDataFolder]Programs\RawStride` (RL9-Q3), written out in the
       source, not left to a per-user redirection of the Program Files folder.
     - Every file of the program folder, the whole of `exiftool_files` included, from a fragment
       the script writes (point 2), `target\dist\msi\files.wxs`, included by the source. Every
       component here is per user, so each follows Windows Installer's rules for the user's
       profile (ICE38, ICE64): **one component per folder**, holding all files of that folder,
       with a registry value under `HKCU\Software\RawStride\Installer` as its key path (one value
       per folder, named by the folder's relative path), and one `RemoveFolder` per folder, so
       uninstalling leaves no folder or key behind. Not WiX's `Files` wildcard: it makes one
       component per file with the file as key path, which ICE38 refuses in the profile. Nothing
       is added or renamed; only `README.txt` is left out (RL9-Q9: it describes the zip), and the
       script's listing skips it by name.
     - One shortcut "RawStride" in the user's Start menu (`ProgramMenuFolder`), to
       `rawstride.exe`, working folder the install folder, in a component of its own with an
       `HKCU` key path and a `RemoveFolder` as above; no desktop shortcut and no file type
       associations (RL9-Q3).
     - The icon from `$(var.IconPath)` as the product's icon in Installed apps (`ARPPRODUCTICON`)
       and the shortcut's icon; `ARPURLINFOABOUT` the repository URL from
       `$(var.RepositoryUrl)`; Modify hidden (`ARPNOMODIFY`), since there is nothing to modify.
     - **No UI** (RL9-Q4): no `WixUI` reference, no licence page, no dialogs; Windows shows only
       its progress bar. **No custom actions.** Nothing under `%LOCALAPPDATA%\RawStride` is
       touched by install or uninstall (RL9-Q6).
  2. **`tools/package-msi.ps1`** (Windows PowerShell 5.1, run from the repository root, ASCII
     only; a comment header naming ADR 0031 RL9, T305, what it makes and how to run it;
     `$ErrorActionPreference = 'Stop'`; every native program through one helper as
     `Invoke-Native` in `package-dist.ps1`, `$LASTEXITCODE` checked; temporary folders removed in a
     `finally`). Constants at the top: `$wixVersion = '7.0.1'` and the EULA id `wix7`, with a
     comment that the release workflow (T306) installs the same version. Parameter
     `-Version <x.y.z>`: makes the MSI with that version instead of `pw-loupe`'s, **for the
     upgrade test only** (the file name says the given version; the files are still the folder's);
     it must be three numbers each 0–255 for the first two and 0–65535 for the third, as Windows
     Installer requires, else stop with a clear message. It:
     - reads `pw-loupe`'s version and repository URL from `cargo metadata` (as
       `package-dist.ps1`); a version (its own or `-Version`) that is not three plain numbers
       within the limits above (a pre-release suffix such as `-rc.1` included) stops the script
       with a clear message;
     - writes `target\dist\msi\files.wxs` (point 1) from the program folder: one directory
       element per folder, one component per folder with its files, its `HKCU` key path and its
       `RemoveFolder`; ids made from the relative path so they are stable between runs and valid
       WiX ids (letters, digits, `_` and `.`, at most 72 characters; a long path gets a short
       hash), with a check that no two ids are equal;
     - requires `target\dist\RawStride-<pw-loupe's version>-windows-x64\` to exist and to hold
       `rawstride.exe`; if it does not, stops with "run `.\tools\package-dist.ps1` first";
     - requires `wix` on PATH and `wix --version` starting with `7.0.1`; if not, stops with a
       message naming `dotnet tool install --global wix --version 7.0.1` (it installs nothing);
     - runs `wix build` with the EULA accepted (`-acceptEula wix7`), x64, the defines `Version`,
       `ProgramDir`, `IconPath`, `RepositoryUrl`, and the output
       `target\dist\RawStride-<version>-windows-x64.msi` (an old one removed first);
     - runs Windows Installer's validation (the ICE checks; WiX 7's command for it, named in the
       log) and stops on an error. A suppressed ICE is allowed only with a comment in the script
       or the `.wxs` giving the reason, and is named in the log;
     - **checks the contents without installing:** an administrative image
       (`msiexec /a <msi> /qn TARGETDIR=<a temporary folder under target\dist>`, started with
       `Start-Process -Wait -PassThru` so its exit code is read; `TARGETDIR`'s path quoted in the
       argument list; only exit code 0 counts as success) and compares it with the program
       folder minus `README.txt`. The files land in a subfolder, `<TARGETDIR>\<the LocalAppData folder's name>\Programs\RawStride\`
       (the script finds the one folder holding `rawstride.exe` and stops if there is not exactly
       one), and `TARGETDIR` also gets an uncompressed copy of the `.msi`, which is ignored. From
       that subfolder: the same relative file names, and each file's SHA-256 equal; stops on any
       difference, listing it;
     - prints the MSI's path, size, SHA-256, its version, and the number of files.
     It writes only under `target\dist\`, and never installs or uninstalls anything.
  3. **Tested by the agent** (in the log): a run after `.\tools\package-dist.ps1 -NoBuild` (or a
     full one) with the output; a run with `-Version 0.4.1` (another file name, the same check); a
     run without the program folder (the message); `wix` absent from PATH in that shell only (the
     message). No `msiexec /i` and no `/x` on the owner's PC: installing is the owner's hand check.
  4. **The MSI's properties** read back in the log (for example with PowerShell's
     `WindowsInstaller.Installer` COM object, opened read-only): `ProductName`, `Manufacturer`,
     `ProductVersion`, `UpgradeCode`, `ALLUSERS` and `MSIINSTALLPERUSER` (for `Scope="perUser"`
     both are expected to be absent [unverified]; the log says what was found),
     `ARPPRODUCTICON`, `ARPNOMODIFY`, and the `Shortcut` table's row.
  5. **`tools/README.md`:** a line for `package-msi.ps1` (what it makes, from what, that it needs
     WiX 7.0.1 and does not install anything; a release MSI is made by the release workflow after
     T306).
- **Tests to add:** none in Rust. The script checks its own result (point 2); the log shows the
  runs of point 3 and the properties of point 4.
- **Not in this task:** `tools/release-build.ps1`, `.github/workflows/release.yml`,
  `SHA256SUMS.txt`, the release notes' Download text, `docs/distribution.md`, `README.md`,
  `CHANGELOG.md` (T306 and the reviewer); `tools/package-dist.ps1` (unchanged: the MSI is made
  from its folder); signing (RL8); any Rust code, `Cargo.toml`, `tools/check.ps1`, `.gitignore`;
  installing or uninstalling the MSI (the owner's hand check).
- **Rules:** `docs/tasks/README.md` rules 1–12 (rule 13 does not apply: no text in the program;
  the downgrade message is Windows Installer's, in English as GR14 asks). No path into a user
  folder and no e-mail address in any file (the personal-data step of `check.ps1`).
- **Hand check (the owner, after the review, with the reviewer):**
  1. Double-click the MSI (version `0.4.0`): no question but SmartScreen's; RawStride is in the
     Start menu and starts; Installed apps shows "RawStride", publisher "RawStride", version 0.4.0,
     the icon; the folder is `%LOCALAPPDATA%\Programs\RawStride`, without `README.txt`; a RAW is rendered and a rating
     written (the shipped ExifTool is used).
  2. Install the `-Version 0.4.1` MSI while RawStride is running: what Windows does is noted
     (with no dialogs it may ask for a restart, RL9-Q11); after it, Installed apps shows one
     RawStride, 0.4.1, and the settings are kept.
  3. Then the 0.4.0 MSI again: it stops with "A newer version of RawStride is already
     installed." and changes nothing. Then a new build of 0.4.1 (`-Version 0.4.1` run again, so
     a new ProductCode): it replaces the installed one, and Installed apps still shows one
     RawStride (RL9-Q10). (The same `.msi` file installed twice only opens Windows Installer's
     maintenance mode, which has nothing to do here.)
  4. Uninstall in Installed apps: the program folder and the shortcut are gone, and so is the
     registry key `HKCU\Software\RawStride` (the components' key paths);
     `%LOCALAPPDATA%\RawStride` is still there.
- **Done when:** `.\tools\check.ps1` is all OK, the script has made and checked the MSI as in the
  log, and `git status` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Open | Added (ADR 0031 RL9, RL9-Q1–RL9-Q8 approved by the owner 2026-11-24); chain T305 → T306 |
| 2026-11-24 | Open | Cold read (T305 and T306 by one fresh agent): `README.txt` describes the zip (owner: left out of the MSI, RL9-Q9); WiX's `Files` wildcard gives one component per file, which ICE38/ICE64 refuse in the user's profile (now a script-written fragment, one component per folder with an `HKCU` key path and `RemoveFolder`); the admin image puts the files in a subfolder and copies the `.msi` (comparison made from the subfolder, the `.msi` ignored); `ALLUSERS`/`MSIINSTALLPERUSER` expected absent; a version with a suffix stops the script; two builds of one version would give two entries (owner: replaces, RL9-Q10); with no dialogs Windows cannot ask to close RawStride (owner: accepted with advice, RL9-Q11; hand check point 2 changed). T306: test with `-Section 0.4.0`, more places in `distribution.md` and the workflow header named |
| 2026-11-24 | Ready for review | `tools/msi/RawStride.wxs` (new): Package as point 1 (UpgradeCode with its never-change comment, perUser, `MajorUpgrade` with the downgrade message and `AllowSameVersionUpgrades`, `MediaTemplate EmbedCab`, install folder `LocalAppDataFolder\Programs\RawStride` written out, icon, `ARPPRODUCTICON`, `ARPURLINFOABOUT`, `ARPNOMODIFY`, no UI, no custom actions); the shortcut component in `ProgramMenuFolder` holds the `RemoveFolder` of `%LOCALAPPDATA%\Programs` (it belongs to no program folder; ICE64; removed only if empty); `files.wxs` included with `<?include files.wxs ?>` through `wix build -includepath target\dist\msi`. `tools/package-msi.ps1` (new): point 2; found in the runs: WiX generates no component GUID for a component with files and a registry key path (error WIX0230), so the script writes one per folder, name-based from the UpgradeCode and the relative path (stable between runs and versions; safe because MajorUpgrade removes the old version first); `Invoke-Native` also prints the program's output before stopping (WiX writes its errors to stdout); build with `-intermediatefolder target\dist\msi\obj` and `-pdbtype none` (nothing in `%TMP%`, no `.wixpdb`). EULA form: `-acceptEula wix7` (accepted by `wix build` and `wix msi validate`; `wix --help` names `--acceptEula` without an id). Validation: `wix msi validate`; warnings ICE61 (expected) and ICE91 once per file (517); ICE91 suppressed with `-sice ICE91`, reason in the script's comment (per-user-only package, it warns about a per-machine install that cannot happen, and hid other warnings); no errors, no ICE38/ICE64. `tools/README.md`: one paragraph. Runs: without the program folder: "...RawStride-0.4.0-windows-x64 with rawstride.exe does not exist: run .\tools\package-dist.ps1 first."; `-Version 0.4.1-rc.1`, `256.0.0`, `0.4`: stop with "not three plain numbers ..." / "out of Windows Installer's range ..."; `wix` not on PATH (VS Code's shell before its restart): "wix is not on PATH. Install WiX 7.0.1 with: dotnet tool install --global wix --version 7.0.1 (this script installs nothing)."; after a full `.\tools\package-dist.ps1` (518 zip entries): `files.wxs` 517 files in 119 folders, admin image "the same 517 files, each with the same SHA-256, in LocalApp\Programs\RawStride", `RawStride-0.4.0-windows-x64.msi` 14,565,464 bytes, SHA-256 9b5d6a3e...008e, version 0.4.0; `-Version 0.4.1`: `RawStride-0.4.1-windows-x64.msi`, same check, 517 files, version 0.4.1 (each build gets a new ProductCode, so the SHA-256 differs between runs). Properties read back (COM, read-only, 0.4.0): ProductName RawStride, Manufacturer RawStride, ProductVersion 0.4.0, UpgradeCode {9DA7D7C8-8D15-4CC5-9C80-2D12BB33937B}, ALLUSERS absent, MSIINSTALLPERUSER absent, ARPPRODUCTICON RawStride.ico, ARPNOMODIFY 1, ARPURLINFOABOUT the repository URL, ProductLanguage 0; Summary platform `x64;0`, word count 10 (compressed, no elevation); Shortcut row `RawStrideShortcut \| ProgramMenuFolder \| RawStride \| StartMenuShortcut \| [INSTALLFOLDER]rawstride.exe \| icon RawStride.ico \| WkDir INSTALLFOLDER`; Upgrade rows: max 0.4.0 inclusive (attributes 513) and the downgrade row; no CustomAction and no Dialog table; 120 Registry rows (`HKCU\Software\RawStride\Installer`) and 120 RemoveFile rows (119 folders and Programs, the shortcut's value). Nothing installed (only `msiexec /a`). `.\tools\check.ps1` all OK (fmt, personal data, audit, test 2574 passed 0 failed 78 ignored, clippy), no clippy warnings |
| 2026-11-24 | Done | Reviewed against every point; the reviewer ran `tools/package-msi.ps1` (517 files in 119 folders, ICE61 only, admin image equal, version 0.4.0) and read the MSI: `RemoveExistingProducts` at 1401, right after `InstallValidate`, so the stable component GUIDs are safe as the log says. Fixed by the reviewer (owner, 2026-11-24): `Invoke-Native`'s comment (it differs from `package-dist.ps1`'s); the GUID comment (SHA-256, version 5 bits, not a true version 5 UUID); the UpgradeCode as one constant `$upgradeCode` in the script, checked against the built MSI's; hand check point 4 also checks that `HKCU\Software\RawStride` is gone. Script rerun after the fixes (same result). No owner decision to record, nothing left over. No CHANGELOG line (tools; the MSI reaches users with T306). Task moved to done/, task index and plan updated; the owner's hand check follows. `.\tools\check.ps1` all OK (run by the reviewer: 2574 passed, 0 failed, 78 ignored), no clippy warnings |
| 2026-11-24 | Done | Hand-checked and approved by the owner: point 1 (installed 0.4.0: Start menu, Installed apps with name, publisher, version and icon, the folder without `README.txt`, a RAW rendered and a rating written); point 2 (0.4.1 over a running RawStride: Windows Installer's files-in-use box asked to close ExifTool and RawStride, then the upgrade went on; recorded in ADR 0031 RL9-Q11, revision 7, and T306 point 5); point 3 (0.4.0 refused with the downgrade message; the same 0.4.1 file again only opened maintenance mode, so a new build of 0.4.1 was installed over it and Installed apps showed one RawStride; point 3's text corrected); point 4 (uninstall: program folder, shortcut and `HKCU\Software\RawStride` gone, `%LOCALAPPDATA%\RawStride` kept) |
