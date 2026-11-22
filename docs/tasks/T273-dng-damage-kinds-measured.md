# T273. ADR 0007 §9.7 Q23: which checks refuse other kinds of DNG damage? (pw-imagecheck example) — Blocked: waits for T267

- **Why:** ADR 0007 revision 30 (§9.7 point 4, Q23; approved by the owner 2026-11-22). The DNG
  image check (§9.3 point 4) is kept for now. LibRaw's unpack is 96–99 % of it (249–553 ms per
  DNG; T264, the analysis `docs/analyses/2026-11-21-metadata-write-time.md`, "Measured in T264").
  T264 measured two kinds of damage: a cut copy, which the `ImageDataHash` comparison refused, and
  a changed preview byte, which only the preview comparison refused. T264's interpretation 7
  leaves open whether damage of other kinds, which neither the hash nor the snapshot listing
  shows, is refused only by the unpack. This task makes such damaged copies and records which
  check refuses each. It only measures. The owner decides afterwards whether the unpack is dropped
  (keeping LibRaw's open, the preview comparison and the snapshot comparison).
- **Blocked because:** T267 adds a section to the same analysis; one at a time (the README's
  "Parallel tasks"). Set to Open when T267 is Done.
- **Crate:** `pw-imagecheck` (an example only). **Files:**
  - `crates/pw-imagecheck/examples/dng_damage.rs` (new: points 1–4)
  - `docs/analyses/2026-11-21-metadata-write-time.md` (point 5: the results)
  - this task's own file (status and log)
- **What:**
  1. **The program.** `dng_damage.rs`, with a doc comment at the top saying what it checks, how it
     is run, and that it writes only copies in its work folder. Arguments, the work folder's rules
     (new or empty, not inside a source file's folder) and the start of ExifTool as
     `examples/check_timing.rs` (T264) does them; read that example first and copy what you need
     from it (examples cannot share code), including its LibRaw C API declarations and data-error
     handler (Windows only). `Result`-based helpers, no panicking ones. A file whose extension is
     not `.dng` is skipped.
  2. **The damaged copies.** For each DNG, find the main RAW image's IFD with a small TIFF reader
     in the example (byte order from the header; no new dependency): walk IFD0's chain, and each
     IFD's `SubIFDs` (330) list, nested ones too, depth first in file order; the first IFD whose
     `NewSubfileType` (254) is 0, or missing (the TIFF default is 0), and that has strip or tile
     offsets, is the RAW IFD. Print which IFD it was (IFD0, or the path of SubIFDs). Print its
     `Compression`, `ImageWidth`/`ImageLength`, and whether it uses strips (273/279) or tiles
     (324/325) and how many. Then make one copy per damage kind below, each from a fresh copy of
     the original, changing only the bytes named (in place; the file's length unchanged):
     - **a. data moved:** the first strip or tile offset + 4096 (within the file);
     - **b. two pieces swapped:** the first two strip or tile offsets exchanged (skipped, and said,
       when there is only one);
     - **c. a short piece:** the first strip or tile byte count halved;
     - **d. data zeroed:** 4096 bytes (or half the piece, if it is shorter) set to 0 in the middle
       of the first strip or tile (offsets and counts unchanged);
     - **e. a wrong width:** the RAW IFD's `ImageWidth` − 16;
     - **f. a wrong compression:** the RAW IFD's `Compression` changed to 1 if it is not 1, else 7;
     - **g. a broken link:** the pointer that leads to the RAW IFD (its `SubIFDs` entry, or the
       next-IFD offset before it) set to point past the end of the file. When the RAW IFD is IFD0,
       g is skipped and said (breaking the header's pointer makes the whole file unreadable, which
       any check finds).
     A value that does not fit where the tag keeps it (an offset list outside the entry) is
     changed where it lies; say in the output where each change was made.
  3. **The checks.** Each damaged copy is checked against the unchanged original by:
     - **the snapshot comparison** of the safe write (§9.3 points 1–3): `ExifTool::snapshot` of the
       original and of the copy, each by a new ExifTool process, then
       `pw_metadata::verify::verify_copy(original, copy, &WritePlan::default())` (a plan that
       writes nothing, so any change in the listing, the hash or the validation is refused); the
       result is "refused" with the error's text, "passed", or "snapshot failed" with ExifTool's
       error (that counts as refused, since the safe write would fail). Note that `verify_copy`
       always allows keys ending in `Offset`, `Offsets` or `Start` (`verify.rs`), so for a and b
       only the hash or the validation can refuse; say so in the analysis;
     - **the image data hash alone** (as T264): equal or not;
     - **the preview comparison alone** (as T264: `check_copy` with `FileKind::Tiff`);
     - **LibRaw's open alone** and **LibRaw's unpack** (C API, return codes and data errors, as T264);
     - **the shim's verdict** (`pw_libraw::Session::open_with_diagnostics`, as T264);
     - **the whole check** (`RawImageCheck::without_release().check_copy(…, FileKind::Dng)`).
     The time of each check is printed too.
  4. **The output.** Per file and damage kind: what was changed (tag, where, old and new value) and
     the result of each check. At the end one Markdown table (rows: file × damage kind; columns:
     the checks), and a list of every copy that **the whole check refuses but the snapshot
     comparison, the preview comparison and LibRaw's open all pass**: the cases where only the
     unpack (or the shim's stream) finds the damage. Afterwards the work folder's files are
     deleted; one that cannot be deleted is named.
  5. **The measurement and the analysis.** Run in a release build
     (`cargo run --release -p pw-imagecheck --example dng_damage -- <files>`), work folder on C:
     (default), on the seven DNGs of T264 (the five `IMG_20261026_*.dng`, `TN002512.DNG`,
     `TN002513.DNG` in `D:\PhotoWorkflowData\RAW`), with the ExifTool on PATH (say its version).
     Add a section "Measured in T273 (Q23: other kinds of DNG damage)" to the analysis, after the
     last "Measured in" section and before the appendix (the work folder written `%TEMP%`, never the
     path the program prints, `AGENTS.md` §2): the build (commit, `--release`), each file's RAW IFD
     (compression, strips or tiles), the table, and the list of point 4. Then: whether any damage
     kind measured is refused only by the unpack; and what was not measured. Mark each conclusion
     **inference** or **hypothesis**. Propose nothing as decided.
- **Tests to add:** none (an example; `check.ps1`'s `cargo clippy --workspace --all-targets`
  builds and checks it).
- **Not in this task:** any change of the image check, the safe write or `pw-libraw`; JPEG and
  TIFF.
- **Rules:**
  - The program never writes in the source folder; it copies first and writes only in its work
    folder.
  - No new dependency (`Cargo.toml` and `Cargo.lock` unchanged).
  - `AGENTS.md` applies (no `unwrap`/`expect` outside tests); text in the program is English.
  - A damaged copy may make LibRaw or the shim crash, hang or allocate without end. So each
    (file, damage kind) is checked in a **child process**: the program runs itself with a hidden
    `--one <original> <damaged copy>` option that runs point 3's checks on that pair and prints
    one result line per check as it is ready; the parent gives each child 120 s, kills it after
    that, and records "crashed (exit code …)" or "timed out" for the checks the child did not
    report. The parent makes the copies, prints the table and the list, and cleans up the work
    folder whatever the children did.
  - ExifTool is the one `ExifTool::start` finds (the program folder first, then PATH); print its
    version and where it came from, as `check_timing.rs` does.
- **Done when:** `.\tools\check.ps1` is all OK, the analysis has its "Measured in T273" section, and
  `git status` shows only the files above (the example as a new file).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written after ADR 0007 revision 30 (§9.7 point 4, Q23; the owner, 2026-11-22: the measurement as a task). Not part of the chain T270 → T272. Waits for T267 (the same analysis) |
| 2026-11-22 | Blocked | Cold read: how the RAW IFD is found (nested SubIFDs, a missing `NewSubfileType` counts as 0, the first that has offsets); g skipped when the RAW is in IFD0; d for a piece under 4096 bytes; "snapshot failed" as a third result; `verify_copy` allows offset keys, so a and b show only in the hash or validation; each (file, damage) in a child process with a 120 s limit, so a crash or hang neither ends the run nor leaves copies; ExifTool's source printed. No other file needs to change |
