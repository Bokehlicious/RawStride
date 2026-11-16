# T162. A FAT32 card read one hour off after a change of the clocks (pw-ingest) — Open

- **Why:** review of T157 (2026-11-16). FAT32 keeps a file's time as local time, and Windows
  converts it to UTC with **today's** daylight saving time, not the one of the day the file was
  written. After the clocks change, every file on a FAT32 card (SD cards up to 32 GB) reads exactly
  3600 s earlier or later than before. T157's register then recognises nothing, so the whole card
  is imported again, and the destination check (`slot` in `lib.rs`) sees "another file with the same
  name", so in the old destination every shot gets `_1`. **Owner, 2026-11-16** (ADR 0005 choice 6,
  "The same time"): a time exactly one hour earlier or later also counts as the same, for both.
- **Crates:** `pw-ingest`. **Files:** `crates/pw-ingest/src/lib.rs`,
  `crates/pw-ingest/src/register.rs`, `crates/pw-ingest/tests/ingest.rs`, `docs/test-inventory.md`.
- **What:**
  1. **One rule** in `lib.rs`, next to `secs`:
     `pub(crate) fn same_time(a: u64, b: u64) -> bool`, true when `a == b` or the two differ by
     exactly 3600 (use `abs_diff`, no overflow). A doc comment says why (FAT32 local time and
     daylight saving time, ADR 0005 choice 6).
  2. **The destination check** (`slot`): when both times exist (`secs` gives `Some`), they are the
     same when `same_time` holds for them; otherwise the comparison stays exactly as today
     (`Option` equality, so two missing times are the same). The local variable `same_time` in
     `slot` is renamed (for example `time_matches`), so it does not shadow the function. Update `slot`'s
     doc comment ("the same modification time (to the second) or exactly one hour apart").
  3. **The register** (`Register::contains`): with the file's seconds `m`, the file is contained
     when the entry `(path, size, t)` is in the set for `t` equal to `m`, `m + 3600` or `m - 3600`
     (with `checked_add` / `checked_sub`; a result that does not exist is skipped). Update its doc
     comment and the module's doc comment the same way. `RegisterWriter::add` is unchanged: it
     writes the time as it reads now.
  4. Nothing else changes: the size rules, the companions, `new_shots` and the Tauri app keep
     working as today (the Tauri app shares `plan`, so it gets the same rule; that is intended).
- **Tests to add:**
  - Unit (`register.rs`): a file whose time is 3600 s later, and one 3600 s earlier, than its line
    is contained; 3599, 3601 and 7200 s off are not; a file with time 0 (1970-01-01) against a
    line with time 3600 is contained, and against a line with time 100 is not, without a panic.
  - Unit (`lib.rs`, a new `#[cfg(test)] mod tests` at the end of the file if there is none):
    `same_time` for equal values, ±3600, ±3599, ±3601, ±7200, and `0` against `u64::MAX` (no
    overflow).
  - Integration (`tests/ingest.rs`, temporary folders):
    Move a time with a small helper: for every file of every shot of `scan(src)`, open `f.path`
    for writing and `set_modified(f.modified ± n s)` (the helper `write` rewrites the content and is
    not used for this). The options are `opts(dst, "", "{name}")`, an empty folder, because a
    `{date}` folder would follow the moved time. `card()` gives 3 shots and 4 files (`ORPHAN.xmp`
    is not scanned).
    - import the card; move every source file's time 3600 s later; `plan` into the same
      destination has `already_present() == 4`, `files_to_copy() == 0` and no name with `_1`; with
      7200 s instead (from the original times), `rels` gives `A001_1.ARW`, `A001_1.xmp`,
      `A002_1.JPG`, `A003_1.ARW` as today (`EMBEDDED_METADATA` waives only the size, not the time);
    - with a register: import with `import_with_register`, move every source file's time 3600 s
      earlier, then `new_shots` gives no shots and 4 files left out; with 7200 s it gives all 3
      shots and 0.
    Each test uses its own `Tmp::new` names.
- **Not in this task:** time zones that move the clocks by other than one hour; the register's
  lines are not rewritten with the new time; `pw-loupe`.
- **Rules:** AGENTS.md §3 (no panics). No new dependencies. No text the user sees changes.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (besides files of other tasks still uncommitted in the tree, which are left as they are).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Open | Added from the review of T157; the owner's decision is in ADR 0005 choice 6 ("The same time"). Changes only `pw-ingest`, so it may run beside T158 |
| 2026-11-16 | Open | Cold read: the local variable `same_time` in `slot` renamed; the way to move a file's time named (`set_modified` on each scanned file, not `write`); the options' empty folder; the expected counts (4 files, `ORPHAN.xmp` not scanned) and the `_1` names with 7200 s written out; the register test's expected result given. Side effect noted: a JPEG, DNG or TIFF exactly one hour off now counts as the same, which is intended |
