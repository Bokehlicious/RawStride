# T270. ADR 0007 §9.7 Q20: ship and pin ExifTool 13.64 (CI, packaging, notices, documents) — Done

- **Why:** ADR 0007 revision 30 (§9.7 point 1, Q20; approved by the owner 2026-11-22): ExifTool
  13.64 fixes both reasons for the digest pass and the TIFF restart (T269, the analysis
  `docs/analyses/2026-11-21-metadata-write-time.md`, "Measured in T269"). RawStride ships 13.64
  and CI tests with it. ADR 0002 §2.5: a version is shipped only after the real-file tests of
  `pw-metadata` and `pw-imagecheck` pass with it. This task changes the pinned version everywhere
  and runs those tests. The lowest version the app accepts (`MIN_EXIFTOOL_VERSION`, 12.67) stays
  for now; T271 raises it. No Rust code changes in this task.
- **The owner's approval (2026-11-22):** ExifTool 13.64 is in `D:\PhotoWorkflowData\exiftool-13.64\`
  (from T269). The ExifTool installed on PATH (13.59) is not replaced by this task.
- **Crates:** none. **Files:**
  - `.github/workflows/ci.yml` (the "Install ExifTool" step: `EXIFTOOL_VERSION`, `EXIFTOOL_SHA256`,
    and its comment)
  - `tools/package-dist.ps1` (`$exifToolVersion`, `$exifToolSha256`, the usage comment)
  - `tools/third-party-notices.ps1` (`$exifToolVersion`)
  - `THIRD-PARTY-NOTICES.txt` (the ExifTool paragraph's version)
  - `third_party/README.md` (the ExifTool row)
  - `docs/distribution.md` (the ExifTool row and the `-ExifToolZip` example)
  - `docs/adr/0002-licensing.md` (§2.5: "today **13.59**" → 13.64, with the date and T270)
  - this task's own file (status and log)
- **What:**
  1. **The checksum.** Find the SHA-256 of `exiftool-13.64_64.zip` in exiftool.org's
     `checksums.txt`, or, if 13.64 is no longer listed there, in SourceForge's
     `checksums-13.64.txt`. Download the zip from the URL CI uses
     (`https://sourceforge.net/projects/exiftool/files/exiftool-13.64_64.zip/download`) into a new
     folder under `%TEMP%`, and check that its SHA-256 equals that value. T269 found
     `72c29c44d1e860eebcaefa71692d979af9858fe9a6d7d791a5e7f1dac77804af`; if the value you find
     differs, stop and report. Delete the download afterwards.
  2. **CI.** In `ci.yml`, set `EXIFTOOL_VERSION: "13.64"` and `EXIFTOOL_SHA256` to that value, and
     rewrite the comment above it to say where the value was checked (which file, and the date).
     Nothing else in the workflow changes.
  3. **Packaging and notices.** In `tools/package-dist.ps1` set the version and SHA-256 (they must
     match CI, as its comment says), and the usage comment's file name. In
     `tools/third-party-notices.ps1` set the version. In `THIRD-PARTY-NOTICES.txt` change the
     version in the ExifTool paragraph by hand, to exactly the text the script would write (read
     the script to see it; do not run it, it needs `cargo-about` and vcpkg). Nothing else in that
     file changes.
  4. **Documents.** `third_party/README.md` and `docs/distribution.md`: 13.59 → 13.64 where they
     name the version shipped or pinned. (`docs/dev-setup.md`'s ExifTool row says what is on PATH;
     it changes in T271, after the owner's update.) `THIRD-PARTY-NOTICES.txt` is edited by hand
     here; `release.yml` regenerates it and compares, so the hand edit must equal the script's
     text. In `docs/adr/0002-licensing.md`
     §2.5, "today **13.59**" → "today **13.64** (2026-11-22, T270; ADR 0007 §9.7)". Do not change
     other mentions of 13.59 in the repository: they record what was measured with that version.
  5. **The real-file tests with 13.64.** With `D:\PhotoWorkflowData\exiftool-13.64` first on PATH
     and `PW_TESTDATA=D:\PhotoWorkflowData\RAW`, set in every command (the shell keeps no
     variables between commands): `cargo test --workspace -- --ignored every_real_` (the five
     tests of `pw-metadata` T269 ran), then
     `cargo test --workspace -- --ignored every_dng_is_written_safely_with_the_real_check a_dng_copy_`
     (the three tests of `crates/pw-imagecheck/tests/real_files.rs`; no `-p`, `AGENTS.md` §5).
     then `cargo test --workspace -- --ignored level_c a_sidecar_next_to_a_real_dng` (the 14
     real-file tests of `crates/pw-metadata/tests/level_c.rs`, which write on copies of the samples
     in `D:\PhotoWorkflowData\RAW\level-c`, and `read.rs`'s `a_sidecar_next_to_a_real_dng_is_ignored`;
     not `read.rs`'s test that writes 20,000 files). Check that each run lists exactly those tests
     and no other ignored test (a bare `--ignored` would also run tests that must not run, such as
     the recycle-bin tests). Every test uses `PW_EXIFTOOL_PL` through `perl` when it is set, so
     clear it (`Remove-Item Env:PW_EXIFTOOL_PL -ErrorAction SilentlyContinue`) and run
     `exiftool -ver` in the same command as each test run; log that it answered 13.64. Record each
     summary. A failure is reported, not fixed
     (README rule 5): stop and report it.
  6. **D8's warning with 13.64.** D8 (ADR 0007 §17) allows one extra validation warning,
     "Non-standard format (undef) for IFD0 0x8649 PhotoshopSettings", when the safe write creates
     a TIFF's or DNG's Photoshop block. 13.64's notes say new `PhotoshopSettings` are now written
     with a proper format code. In a new folder under `%TEMP%`, copy one DNG without IIM data
     (`IMG_20261026_062851.dng`, T269) and, with each version (13.64 by its folder, 13.59 by
     `%LOCALAPPDATA%\Programs\ExifTool\exiftool.exe`), write `-o <copy> -IPTC:City=Lund
     -IPTCDigest=new <source copy>` and read `-validate -warning -a <copy>` (without `-j`, so every
     warning is listed) from a new process. Record whether that warning appears with each version.
     Delete the folder afterwards.
- **Tests to add:** none (no code changes).
- **Not in this task:**
  - `MIN_EXIFTOOL_VERSION` and the texts and tests that name 12.67 (T271).
  - The digest pass, the restart, D8's allowance (T272, and a later decision for D8).
  - `CHANGELOG.md` (the reviewer's, at Done).
  - Replacing the ExifTool installed on PATH (the owner's, before T271).
- **Rules:**
  - Nothing is written in `D:\PhotoWorkflowData\RAW`; the tests copy first.
  - No new dependency. No other change to `ci.yml` than the ExifTool step's two values and comment.
  - `.\tools\check.ps1` is run at the end as usual, with the ExifTool on PATH (13.59 or 13.64; the
    app accepts both until T271); log `exiftool -ver` from the same command.
- **Done when:** `.\tools\check.ps1` is all OK, the log has point 5's summaries and point 6's
  result, and `git status` shows only the files above changed by this task.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written after ADR 0007 revision 30 (§9.7, Q20–Q23, approved by the owner 2026-11-22); first of the chain T270 → T271 → T272. Open after the cold read and the chain review |
| 2026-11-22 | Blocked | Cold read: every file that pins 13.59 is listed (nothing in `release.yml` or `release-build.ps1`); the `level_c` tests and `a_sidecar_next_to_a_real_dng_is_ignored` added to point 5 (ADR 0002 §2.5 names all of `pw-metadata`'s real-file tests), `PW_EXIFTOOL_PL` cleared and `exiftool -ver` logged with each run; `docs/dev-setup.md` moved to T271 (it says what is on PATH); `release.yml` compares the hand-edited notices with the script's; "Done when" by `git status` |
| 2026-11-22 | Blocked | Chain review (T270–T272): the gaps between the tasks are safe (after T270 CI has 13.64 and the app's minimum is still 12.67; after T271 the digest pass and the restart are redundant but harmless). Changed: the owner installs 13.64 in a folder of its own and keeps 13.59 (T270 point 6 compares the two); T271 and T272 clear `PW_EXIFTOOL_PL` and log `exiftool -ver`; an undo in reverse order (T272 before T271), in ADR 0007 §9.7; §9.7 point 3 corrected (`ExifTool::restart` is not used by the worker or the pool); T267 runs whole before T270, so this task now waits for T267. For the reviewer at each Done: change log lines (T270 Changed: the zip includes ExifTool 13.64; T271 Changed: writing needs ExifTool 13.64 or newer, reading works with an older one; T272 Changed: faster TIFF and DNG writes, a copy whose IIM checksum is not current is refused); ADR 0007 §11 and D9 at T271, §5.2, §9.2, §9.3 at T272; the D8 item in `backlog.md` at T270 with point 6's result; after T272 the saving re-measured (a task) and a release suggested. A guard that the shipped ExifTool is at least `MIN_EXIFTOOL_VERSION`: `backlog.md` |
| 2026-11-22 | Open | T267 done (measured with 13.59 before this task) |
| 2026-11-22 | Ready for review | Point 1: exiftool.org's `checksums.txt` lists `SHA2-256(exiftool-13.64_64.zip)= 72c29c44d1e860eebcaefa71692d979af9858fe9a6d7d791a5e7f1dac77804af`, the value T269 found; the zip downloaded from CI's SourceForge URL into a new folder under `%TEMP%` (11,217,963 bytes) has that SHA-256; folder deleted. Points 2–4: `ci.yml` (version, SHA-256, comment: checked in exiftool.org's `checksums.txt` on 2026-11-22, a download the same day matched), `package-dist.ps1` (version, SHA-256, usage comment), `third-party-notices.ps1` (version), `THIRD-PARTY-NOTICES.txt` (only "ExifTool 13.59" → "ExifTool 13.64", the script's here-string with the new version; script not run), `third_party/README.md`, `docs/distribution.md` (row and `-ExifToolZip` example), ADR 0002 §2.5. The two `.ps1` files kept CRLF in the working copy. Point 5, each command with `D:\PhotoWorkflowData\exiftool-13.64` first on PATH, `PW_TESTDATA=D:\PhotoWorkflowData\RAW`, `PW_EXIFTOOL_PL` cleared and `exiftool -ver` = 13.64 (from that folder): `every_real_` ran exactly five tests, all ok (edit.rs 1 passed, 50.42 s; safe_write.rs 2 passed, 52.77 s; verify.rs 2 passed, 14.45 s); `every_dng_is_written_safely_with_the_real_check a_dng_copy_` ran exactly the three tests of `real_files.rs`, `ok. 3 passed; 0 failed`, 8.08 s; `level_c a_sidecar_next_to_a_real_dng` ran only `a_sidecar_next_to_a_real_dng_is_ignored` (`ok. 1 passed`, 0.40 s). The filter `level_c` matches no test name, since cargo filters test names, not file names, so `level_c.rs`'s tests were run as `cargo test --workspace --test level_c -- --ignored` (no `-p`, workspace feature set): exactly its 14 ignored tests, `ok. 14 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out`, 2.68 s. No other ignored test ran in any of the runs. Point 6 (`IMG_20261026_062851.dng` copied to a new folder under `%TEMP%`, `-o <copy> -IPTC:City=Lund -IPTCDigest=new`, then `-validate -warning -a` from a new process): with 13.59, 8 warnings (1 minor), including "Non-standard format (undef) for IFD0 0x8649 PhotoshopSettings"; with 13.64, 7 warnings (1 minor) and no PhotoshopSettings warning. The other seven are the same with both versions (six "Wrong IFD …" warnings and the minor XiaomiModel warning). A cross-read shows it is the writer that differs: 13.64 reading 13.59's copy still lists the warning, and 13.59 reading 13.64's copy lists 7. Folder deleted. `.\tools\check.ps1` with the ExifTool on PATH (`exiftool -ver` 13.59 in the same command): fmt, personal data, cargo audit, tests, clippy all OK (2286 passed, 0 failed, 77 ignored), no clippy warnings |
| 2026-11-22 | Done | Reviewed: the diff is only the listed files (`ci.yml`: the two values and the comment); no other file pins 13.59 (the rest record measurements); the notices line equals the script's here-string. The reviewer checked exiftool.org's `checksums.txt` (same SHA-256) and re-ran, with 13.64 first on PATH and `PW_EXIFTOOL_PL` cleared, `real_files.rs`'s three tests (`ok. 3 passed`) and `--test level_c -- --ignored` (`ok. 14 passed`). Point 5's `level_c` filter matched no test name; the agent's `--test level_c` is right (T271 and T272 do not use the filter). Point 6's result is in `backlog.md` (D8's allowance). CHANGELOG (Changed), README index, done list, plan row 2a; T271 now waits for the owner's ExifTool 13.64 on PATH |
