# T265. ADR 0007 §9.6 Q19: do the reasons for the digest pass and the TIFF restart still hold? (pw-metadata example) — Blocked: waits for T264

- **Why:** ADR 0007 revision 29 (§9.6 point 3, Q19; approved by the owner 2026-11-22). Two steps
  of the safe write rest on behaviour measured with ExifTool 13.59 in T92:
  - **the digest pass** (§5.2, §9.2): in a TIFF or DNG, `-IPTCDigest=new` in the same call as an
    IIM write stores an out-of-date checksum, so a second call
    (`-IPTCDigest=new -overwrite_original <copy>`) stores it on the copy;
  - **the TIFF restart** (§9.3): after a running ExifTool process has written the checksum to a
    TIFF whose Photoshop block holds a checksum followed by an `AgHg` resource, that process reads
    the block of every such TIFF wrongly (two more warnings: "Non-standard Photoshop at
    TIFF-IFD0-Photoshop-IPTCDigest", "Bad Photoshop IRB resource") until it ends, so a correct
    copy would be refused.
  Together they are 15–30 % of a TIFF or DNG write (the analysis
  `docs/analyses/2026-11-21-metadata-write-time.md`, interpretation 5). This task checks whether
  both reasons still hold with the ExifTool in use, and times both steps. It only measures; it
  changes no code that the app runs. The owner decides afterwards (§9.6 point 3).
- **Crate:** `pw-metadata` (an example only; the crate has no `examples/` folder yet, Cargo finds
  it). **Files:**
  - `crates/pw-metadata/examples/digest_and_restart.rs` (new: points 1–4)
  - `docs/analyses/2026-11-21-metadata-write-time.md` (point 5: the results)
  - this task's own file (status and log)
- **What:**
  1. **The program.** `digest_and_restart.rs`, with a doc comment at the top saying what it
     checks, how it is run (point 4), and that it writes only copies in its work folder. Arguments:
     the image files, `--runs <n>` (default 3, for the times) and `--work <folder>` (default a new
     folder under `std::env::temp_dir()`; a given folder must be new or empty, refused otherwise,
     as `crates/pw-imagecheck/examples/write_timing.rs`'s `refuse_used_folder` does; a work folder
     inside a source file's folder is refused, as its `refuse_source_folders` does). ExifTool is
     started and printed as `write_timing.rs`'s `start_exiftool` does (`PW_EXIFTOOL_PL` through
     `perl`, otherwise `ExifTool::start()`; the source as a text, since the program path is not
     public, and `ExifTool::version`), first. Do not copy the tests' panicking helpers (`exiftool`,
     `digest_state`); write `Result`-based ones as `write_timing.rs` does. Read `crates/pw-metadata/src/safe_write.rs`
     (the module comment, `digest_pass`, `needs_restart`) and the T92 tests in
     `crates/pw-metadata/tests/safe_write.rs` (`ag_hg_block`, `digest_state`,
     `a_tiff_with_an_aghg_resource_is_written_and_read_correctly_afterwards`) before writing it;
     do not change them.
  2. **The digest in the same call.** For each file (a JPEG is the control), a copy in the work
     folder is written by one ExifTool call with an IIM and XMP change and the checksum together:
     `-o <copy> -IPTC:CodedCharacterSet=UTF8 -IPTC:City=Lund -XMP-photoshop:City=Lund
     -IPTCDigest=new <source copy>` (through `ExifTool::execute`, as the safe write does, so the
     text is UTF-8; the arguments of the T92 tests). A second copy is written the way the safe
     write does it (`write_copy`: the plan's call, `-IPTCDigest=new` included, then
     `digest_pass`): the same call, then the digest pass `-IPTCDigest=new -overwrite_original
     <copy>`. Each copy's checksum is read by a **new** ExifTool process that has written nothing
     (one per copy), with `ExifTool::snapshot` and its listing keys `Photoshop:IPTCDigest` and
     `File:CurrentIPTCDigest`, as `digest_state` does; the result per copy is "current" (equal),
     "out of date" or "missing". Every write's ExifTool answer that is not "1 image files
     updated" (or "created") is printed. The time of the digest pass is taken per run.
  3. **The misread after a TIFF write.** For each TIFF: two copies A and B of the file in the work
     folder. A new process P writes A the way the safe write does (point 2's second sequence:
     the `-o` call with the IIM change and `-IPTCDigest=new` from A to a new file A2, then the
     digest pass on A2; the T92 trigger, `a_tiff_with_an_aghg_resource_is_written_and_read_correctly_afterwards`),
     its answers printed; then P takes `ExifTool::snapshot(B)`; a new process Q takes
     `ExifTool::snapshot(B)` too. The result is "same" if the two snapshots are equal without the
     `System` group (`System:FileAccessDate` may change with P's read; `verify_copy` allows that
     group too), else the first difference (`validate`, or the first listing key that differs,
     with both values). The misread's two warnings show only in `validate` (`ExifTool:Warning`
     is not in the listing); when `validate` differs, one extra call
     `-validate -warning -a -G1 -j <B>` by each process prints their texts. Whether the TIFF has
     an `AgHg` resource is found by a byte search of the file for the signature `AgHg` followed by
     the big-endian id 0x1f40, as `ag_hg_block` and `photoshop_resource` lay a resource out (the
     listing does not show it), and printed. The
     time of `ExifTool::restart` of P (after its snapshot) is taken per run; it includes ending
     the old process (up to 2 s, `Drop`), say so.
  4. **The output.** Per file: name, kind, size in MB; point 2's two results and the digest
     pass's times and median; for a TIFF, point 3's result and the restart's times and median. At
     the end one Markdown table of all files. Run as
     `cargo run --release -p pw-metadata --example digest_and_restart -- <files>`. Afterwards the
     work folder's files are deleted; one that cannot be deleted is named.
  5. **The measurement and the analysis.** Run in a release build on thanos, work folder on C:
     (default), on `TN006331.JPG` (control), `TN002683.tif` (has the `AgHg` resource, T92),
     `TN004596.tif`, `TN006025.tif`, the five `IMG_20261026_*.dng`, `TN002512.DNG` and
     `TN002513.DNG` from `D:\PhotoWorkflowData\RAW`. Add a section "Measured in T265 (Q19: the
     digest pass and the TIFF restart)" to the analysis, after "Measured in T264" and before the
     appendix (the work folder written `%TEMP%`, never the path the program prints, `AGENTS.md`
     §2): the build (commit, `--release`), the ExifTool version, the table, and per reason whether
     it still holds with this ExifTool (a reason holds if at least one TIFF or DNG shows it). Say
     that point 3 rechecks TIFFs only, not the DNG half of `needs_restart`'s reason. Mark each
     conclusion **inference** or **hypothesis**. Propose nothing as decided.
- **Tests to add:** none (an example; `check.ps1`'s `cargo clippy --workspace --all-targets`
  builds and checks it).
- **Not in this task:**
  - Any change of the safe write, the digest pass, the restart or the T92 tests.
  - Another ExifTool version: none is installed for this (`AGENTS.md` §3, no software without the
    owner's approval). If the version in use is not 13.59, say so in the analysis.
  - The DNG image check (T264).
- **Rules:**
  - The program never writes in the source folder; it copies first and writes only in its work
    folder.
  - No new dependency (`Cargo.toml` and `Cargo.lock` unchanged).
  - `AGENTS.md` applies (no `unwrap`/`expect` outside tests); text in the program is English.
- **Done when:** `.\tools\check.ps1` is all OK, the analysis has its "Measured in T265" section,
  and `git status` shows only the files above (the example as a new file).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written after ADR 0007 revision 29 (§9.6 point 3, Q19), approved by the owner 2026-11-22; waits for T264 (the order of §9.6; both add a section to the same analysis, so they run one after the other) |
| 2026-11-22 | Blocked | Cold read: point 2's safe-write sequence corrected (`write_copy` keeps `-IPTCDigest=new` in the `-o` call, then the digest pass), `-IPTC:CodedCharacterSet=UTF8` added as in the T92 tests, the checksum read with `snapshot` and its listing keys as `digest_state` does, one fresh reader per copy; point 3: P runs the safe write's sequence (the T92 trigger), not a digest pass alone, the snapshots compared without `System`, the warnings from `validate` plus an extra `-validate -warning` call, `AgHg` found by a byte search, the restart timed on P including its end; ExifTool started as `start_exiftool`, `refuse_source_folders` named, no panicking test helpers copied; `%TEMP%` in the analysis, the section before the appendix, the DNG half of the restart not rechecked. No other file needs to change |
