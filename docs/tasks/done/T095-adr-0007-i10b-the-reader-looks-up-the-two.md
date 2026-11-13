# T95. ADR 0007 I10b: the reader looks up the two sidecar names instead of listing the folder — Done

- **Can start now.** No dependency change. Can run at the same time as T94, T96, T97 and T99 (no
  shared files).
- **Crates:** `pw-metadata` only.
- **Files:** `crates/pw-metadata/src/read.rs` (`find_sidecars` and the module comment only);
  `crates/pw-metadata/tests/read.rs` (new tests only); `docs/test-inventory.md`.
- **Why:** found in T82: `find_sidecars` reads the whole folder (`read_dir`) for every image it
  reads, to find `<basename>.xmp` and `<basename>.<ext>.xmp` without regard to case (§4.1, §4.2,
  Q5, Q8). The info panel (T103) reads one image each time the user moves to the next, so in a
  folder of thousands of files every step lists them all (slow on a memory card or a network
  share).
- **What:**
  1. **`find_sidecars` on Windows:** the two names are looked up directly
     (`folder.join(name)`, `is_file()`); NTFS and FAT/exFAT compare names without regard to case,
     so `IMG_1.XMP` is found for `img_1.dng`. The path returned is the name **as it is in the
     folder** (the reader reports it to the user): `folder.join(<file name of
     std::fs::canonicalize(found)>)`, since Windows gives the stored case there; check it in the
     test below. No `read_dir`. A folder that cannot be read is the error it is today.
  2. **Other systems** (`#[cfg(not(windows))]`): the listing stays as it is (their file systems
     may compare case).
  3. The module comment says why.
- **Rules:** rules 3–7 and 10–12. No other change in `read.rs`; what `read_metadata` returns must
  be the same for every existing test.
- **Tests to add** (`tests/read.rs`, ExifTool required, files made as the existing tests make them):
  - a DNG with `IMG_1.XMP` (other case) next to it → `ForeignSidecarIgnored` with the name
    `IMG_1.XMP` as in the folder; a fake RAW `img_2.arw` with `IMG_2.xmp` → the sidecar is read;
    `x.arw.XMP` → `ExtSidecarIgnored`;
  - one test **ignored by default**: a temporary folder with 20 000 empty files and one JPEG;
    `read_metadata` of the JPEG ten times; report the time per read in the log row (before and
    after the change if you can).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
