# T269. ADR 0007 §9.6 Q19: the digest pass and the TIFF restart with ExifTool 13.64 (measurement) — Blocked: waits for T266

- **Why:** ADR 0007 revision 29 (§9.6 point 3, Q19). T265 found that both reasons for two extra
  steps of the safe write still hold with ExifTool 13.59 (the analysis
  `docs/analyses/2026-11-21-metadata-write-time.md`, "Measured in T265"): the digest pass (§5.2,
  §9.2; 39–438 ms per TIFF or DNG) and the TIFF restart (§9.3; about 190 ms per TIFF). ExifTool
  13.64 (2026-11-16, a development release; the newest production release is 13.55) lists three
  fixes that match them (https://exiftool.org/history.html):
  - "Fixed bug where reading back a file containing IPTCDigest using the -stay_open feature after
    writing a file with IPTCDigest would result in an erroneous 'Bad Photoshop IRB resource'
    warning" (the reason for the TIFF restart; the owner reported it, the repro is in
    `D:\PhotoWorkflowData\exiftool-report`);
  - "Fixed bug where -IPTCDigest=new could generate an incorrect digest when writing new IPTC to
    TIFF-format files (due to the padding which may be required due to the improper TIFF format
    used by Adobe for IPTC-NAA)" (the reason for the digest pass; DNG is a TIFF format too);
  - "Fixed bug where new PhotoshopSettings were created in TIFF-format files with an appropriate
    format code instead of the improper code used by Adobe" (changes what is written in a TIFF,
    so it may change the warnings the safe write compares, §9.3).

  This task measures whether the two reasons are gone with 13.64, and whether the safe write and
  its tests still pass with it. It only measures; it changes no code, no test and no tool. The
  owner decides afterwards (§9.6 point 3) whether the digest pass or the restart changes, and
  whether the lowest ExifTool version (`MIN_EXIFTOOL_VERSION`, 12.67) and CI's pinned version
  (13.59) change; that is a later revision of ADR 0007 and its own tasks.
- **The owner's approval (2026-11-22):** ExifTool 13.64 may be downloaded for this measurement
  (`AGENTS.md` §3), into a folder of its own; the installed 13.59 is not replaced, and PATH is not
  changed outside the shell that runs the measurement.
- **Blocked because:** the example builds against `pw-metadata`, which T266 is changing in the
  same worktree, and `.\tools\check.ps1` and the `target` folder are shared. Set to Open when T266
  is Done and committed.
- **Crate:** none changed. **Files:**
  - `docs/analyses/2026-11-21-metadata-write-time.md` (point 5: the results)
  - this task's own file (status and log)
- **What:**
  1. **ExifTool 13.64.** Download `exiftool-13.64_64.zip` from exiftool.org (or, if it is gone
     there, from the SourceForge file archive as CI does, `.github/workflows/ci.yml`). Check its
     SHA-256 against exiftool.org's `checksums.txt`, or, when 13.64 is no longer listed there
     (it lists only current versions, `ci.yml`'s note), SourceForge's `checksums-13.64.txt`; stop
     if it differs or no checksum is found. Unpack it to `D:\PhotoWorkflowData\exiftool-13.64\` (outside the
     repository), so that folder holds `exiftool.exe` (renamed from the package's
     `exiftool(-k).exe`, as the "Install ExifTool" step of `ci.yml` does) and its `exiftool_files`
     folder. Delete the zip afterwards. For points 2–4 only, put that folder first on PATH and
     leave `PW_EXIFTOOL_PL` unset. The agent's shell keeps no environment variable from one
     command to the next, so set PATH (and `PW_TESTDATA` in point 4) in every command that needs
     it (PowerShell: `$env:PATH = "D:\PhotoWorkflowData\exiftool-13.64;$env:PATH"; …`); check that `exiftool -ver` answers `13.64`
     and that the program prints 13.64 at start (`ExifTool::start` finds `exiftool` on PATH, since
     `target\release\examples\` holds no `exiftool.exe`). Every result below names the version it
     was made with.
  2. **The owner's repro.** Once per version, each in a new empty folder under `%TEMP%`: copy
     `D:\PhotoWorkflowData\exiftool-report\sample.tif` and `repro-args.txt` there, and run
     `exiftool -stay_open True -@ repro-args.txt` in it (the file ends with `-stay_open False`,
     so the process ends by itself). First with 13.64 (first on PATH), then with 13.59, the
     installed one, called by its full path, `%LOCALAPPDATA%\Programs\ExifTool\exiftool.exe`,
     since 13.64 is first on PATH (`-o out.tif` refuses an existing file, hence the folder per
     version). Compare with `repro-output.txt` (made with 13.59): does the second `-validate`
     still show "[minor] Non-standard Photoshop at TIFF-IFD0-Photoshop-IPTCDigest" and "Bad
     Photoshop IRB resource"? Also read `out.tif`'s `Photoshop:IPTCDigest` and
     `File:CurrentIPTCDigest` with a new process of the same version
     (`exiftool -a -G1 -IPTCDigest -CurrentIPTCDigest out.tif`): current or out of date. Delete
     both folders afterwards. Nothing is written to `D:\PhotoWorkflowData\exiftool-report`.
  3. **T265's example with 13.64.** Run
     `cargo run --release -p pw-metadata --example digest_and_restart -- <files>` (unchanged, the
     default three runs, the default work folder on C:) with 13.64 on the same eleven files as
     T265: `TN006331.JPG`, `TN002683.tif`, `TN004596.tif`, `TN006025.tif`, the five
     `IMG_20261026_*.dng`, `TN002512.DNG` and `TN002513.DNG` from `D:\PhotoWorkflowData\RAW`.
     Keep its whole output in a file outside the repository (under `%TEMP%`); it prints the work
     folder's full path, which is never copied into the repository (`AGENTS.md` §2). Then read,
     with a new 13.64 process and without writing, whether each of the eleven originals holds IIM
     data before any write: `exiftool -a -G1 -IPTC:all <file>` (any `[IPTC]` line other than
     `CodedCharacterSet` and `ApplicationRecordVersion` counts as IIM data).
  4. **The tests with 13.64.** With 13.64 first on PATH and `PW_TESTDATA=D:\PhotoWorkflowData\RAW`:
     `cargo test --workspace` (rule 6 of the README allows it while working), then the ignored
     real-file tests of `pw-metadata`: `cargo test --workspace -- --ignored every_real_`. The
     filter selects five tests, all of which copy to `%TEMP%` or only read: T87's
     `every_real_dng_jpeg_and_tiff_is_rated_safely` (`tests/edit.rs`), the two in
     `tests/safe_write.rs` (among them `every_real_tiff_and_dng_gets_a_current_digest`, the
     digest pass's own test) and the two in `tests/verify.rs`. Check that the run lists exactly
     these five and no other ignored test (the recycle-bin tests must not run); it reads every
     TIFF, DNG and JPEG in `PW_TESTDATA` (about 3.8 GB) and takes a while. Record the summaries, and for every failing test
     its name and the message, without changing the test or the code (rule 5). A failure here is
     a result, not a reason to stop the task.
  5. **The analysis.** Add a section "Measured in T269 (Q19 with ExifTool 13.64)" to the
     analysis, after "Measured in T265" and before the appendix (folders written `%TEMP%` and
     `D:\PhotoWorkflowData\…`, never a user folder's path, `AGENTS.md` §2): the build (commit,
     `--release`), the versions, the SHA-256 checked, point 2's result for both versions, point
     3's table next to T265's columns for the checksum states and the P/Q comparison (the times
     from point 3 too), and point 4's summaries and failures. Then, per reason, whether it still
     holds with 13.64:
     - **the digest pass:** gone only if the one call gave a current checksum in every TIFF and
       DNG in every run. Say which of the eleven sources already held IIM data before the write
       (13.64's note says "writing new IPTC"), since the fix may cover only some of them;
     - **the TIFF restart:** gone only if P and Q read every untouched TIFF copy the same in every
       run;
     - **the safe write as it is:** whether it still works with 13.64 (point 4), and whether the
       third fix changed any warning the safe write compares.
     Mark each conclusion **inference** or **hypothesis**. Say what was not measured (at least:
     TIFFs without an `AgHg` resource, a TIFF or DNG with no IIM data before the write, a cold
     cache, another 13.6x version). Leave "Measured in T265" as it is (its point 6, "another
     ExifTool version", is answered here). Propose nothing as decided.
- **Tests to add:** none (a measurement; no code changes).
- **Not in this task:**
  - Any change of the safe write, the digest pass, the restart, `MIN_EXIFTOOL_VERSION`, the
    example, the tests, `docs/building.md` or CI's pinned ExifTool (`ci.yml`). They wait for the
    owner's decision on Q19.
  - Replacing the installed ExifTool 13.59 or changing PATH for the user or the system.
  - The DNG image check (T264).
- **Rules:**
  - Nothing is written in `D:\PhotoWorkflowData\RAW` or `D:\PhotoWorkflowData\exiftool-report`;
    the example copies first, as in T265.
  - No new dependency (`Cargo.toml` and `Cargo.lock` unchanged).
  - **An exception to the README's rules 6 and 9:** `.\tools\check.ps1` is not run (no code
    changes, and it would run with 13.64 or 13.59 depending on PATH); the report gives point 4's
    test summaries instead.
- **Done when:** the analysis has its "Measured in T269" section, `D:\PhotoWorkflowData\exiftool-13.64\`
  holds ExifTool 13.64 (kept for the owner's decision), and `git status` shows only the two files
  above changed.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written at the owner's request after T265 (the owner: a newer ExifTool may have fixed the TIFF bug the owner reported); ExifTool 13.64's change notes list fixes for both reasons. Waits for T266 (same worktree, `pw-metadata` and `target` shared) |
| 2026-11-22 | Blocked | Cold read: the checksum's fallback to SourceForge when 13.64 leaves `checksums.txt`; PATH set in every command (the shell keeps no variables); the repro in a folder per version, 13.59 called by its full path; the example's output kept outside the repository (it prints the work folder's path); a read-only command for whether each original holds IIM data; the five tests `every_real_` selects named, the recycle-bin tests must not run; check.ps1's exception to rules 6 and 9 stated; T265's section left as it is. No other file needs to change |
