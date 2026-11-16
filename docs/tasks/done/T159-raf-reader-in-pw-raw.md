# T159. Fujifilm RAF in pw-raw: previews, size and Exif — Done

- **Why:** the owner's job test (2026-11-16): Fujifilm RAW files (`.RAF`) are not shown, and an
  import skips them without a word. **Owner, 2026-11-16** (ADR 0003, point 5): RAF is to be
  supported (CR3 later, when there are files). This is the first of two tasks. This one teaches
  `pw-raw` to read a RAF file. T160 then lists `raf` in the viewer, the ingest and the sidecar rules.
  LibRaw already opens and renders RAF, X-Trans included (`crates/pw-libraw/tests/statistics.rs`
  uses the five RAF files), so no rendering task is needed.
- **The format** (public, as LibRaw and ExifTool read it; checked by the reviewer on `_DSF4243.RAF`
  and `_DSF4271.RAF`, X-T30). The RAF header and its metadata directory are big-endian; the Exif
  inside the embedded JPEG is a normal TIFF (here little-endian, `II`), which `tiff.rs` already
  reads:
  - bytes 0–15 are `FUJIFILMCCD-RAW ` (with the space); bytes 28–59 hold the camera model in ASCII,
    padded with `\0` (`X-T30`);
  - at byte 84: `u32` offset and `u32` length of the embedded JPEG (148 and 2 995 344 here). It is
    a full JPEG (`FF D8 FF E1`, 4416 × 2944) whose APP1 Exif holds Make, Model, DateTimeOriginal,
    exposure and Orientation. Its IFD1 has a thumbnail (160 × 120): `JPEGInterchangeFormat` 3936,
    relative to the Exif TIFF base (absolute byte 160 here, so the thumbnail is at byte 4096), and
    `JPEGInterchangeFormatLength` 6 608;
  - at byte 92: `u32` offset and `u32` length of the metadata directory: a `u32` count, then
    `count` entries of `u16` tag, `u16` size and `size` bytes of data. Tag `0x0111` (size 4) is the
    cropped image size, height then width as two `u16` (4160, 6240 here).
- **Crates:** `pw-raw`. **Files:** `crates/pw-raw/src/raf.rs` (new), `crates/pw-raw/src/lib.rs`,
  `crates/pw-raw/src/tiff.rs`, `crates/pw-raw/tests/real_files.rs`, `docs/test-inventory.md`.
- **What:**
  1. `Format` gets `Raf` ("Fujifilm RAF: an own header with an embedded JPEG"). The crate's doc
     comment (`lib.rs`, "Supported containers") names RAF too.
  2. `inspect_source`: when the source is at least 16 bytes long, it reads 16 bytes and checks
     them against the RAF magic before the JPEG and TIFF checks; on a match it calls
     `raf::inspect_raf(src)`. Shorter files go on to the JPEG and TIFF checks as today (no new `Io`
     error for a file of 4–15 bytes).
  3. `inspect_raf` is never a panic: checked arithmetic, `Source::read_at` only, every offset and
     length from the file checked against the file's length. Its errors are `Error::Format` with a
     text starting `"RAF: "`:
     - a RAF shorter than 100 bytes, or an embedded JPEG whose offset + length lies past the end of
       the file, or a JPEG `jpeg::dimensions` cannot read, is an error (there is nothing to show);
     - a metadata directory that is missing, lies outside the file, or whose entries run past the
       directory's own length (the loop is bounded by the directory length) is **not** an error:
       `width`/`height` are then `None`.
  4. `inspect_raf` fills `ImageInfo`, starting from `orientation: 1` as `jpeg.rs` does
     (`fill_from_exif_tiff` reads the Exif orientation only when it is still 1):
     - `format: Some(Format::Raf)`;
     - from the embedded JPEG's Exif: everything `fill_from_exif_tiff` already fills.
       `jpeg::exif_tiff(src, jpeg_offset, jpeg_length)` already works on a part of the file and gives
       the absolute Exif TIFF base;
     - the model from the header if the Exif has none;
     - `width`/`height` from tag `0x0111` (width 6240, height 4160 here); without it, `None`;
     - `previews`, sorted by `(width, length)` as `inspect_tiff` sorts them:
       - the embedded JPEG: its offset and length, its size from `jpeg::dimensions`, source
         `"RAF/JpegImage"`;
       - the Exif thumbnail, only if it lies inside the embedded JPEG's range: from a new
         `pub(crate) fn exif_thumbnail<S: Source>(src: &mut S, base: u64) -> Option<(u64, u64)>`
         in `tiff.rs` (absolute offset and length: `Parser::new(src, base)`, IFD0, its next IFD,
         then `JPEG_IF_OFFSET` and `JPEG_IF_LENGTH`, offset + `base`); size from `jpeg::dimensions`;
         source `"RAF/IFD1/JPEGInterchangeFormat"`. `fill_from_exif_tiff` itself is unchanged.
  5. `read_preview` needs no change (it reads by offset and length).
  6. `SUPPORTED_EXTENSIONS` is **not** changed here (T160).
  7. The doc comment at the top of `tests/real_files.rs` recommends `cargo test -p pw-raw`, which
     AGENTS.md §5 forbids. Change it to `$env:PW_TESTDATA = "<folder>"; .\tools\check.ps1 -Ignored`.
- **Tests to add:**
  - Unit (`raf.rs`), on a RAF built in the test. The JPEG part comes from `lib.rs`'s test helper
    `tiny_jpeg`, made `pub(crate)` inside `lib.rs`'s `mod tests` and used as
    `crate::tests::tiny_jpeg`. The RAF has the 16-byte magic, the model, offsets to that JPEG (with
    an Exif Orientation of 6), and a metadata directory with tag `0x0111`:
    - format, model, size, orientation 6 and one preview with the right offset;
    - no `0x0111` gives `None` size but a good result; a directory count larger than the directory
      gives the same;
    - a JPEG whose offset + length lies past the end of the file, and a RAF of 50 bytes, each give
      `Error::Format` with the `"RAF: "` start, not a panic;
    - a file that starts with `FUJIFILM` but not the full magic is an error whose text does not
      start with `"RAF: "` (it went to the TIFF check);
    - a file of 10 bytes gives the same error as before this task (not an `Io` error).
  - `tiff.rs`: `exif_thumbnail` on a small Exif TIFF with an IFD1 gives the absolute offset and
    length; without IFD1 it gives `None`.
  - Real files (`tests/real_files.rs`, ignored like the others, with `PW_TESTDATA`):
    - `_DSF4243.RAF`: `Format::Raf`; make `FUJIFILM`; model `X-T30`; date `2020:05:30 20:57:18`;
      `exposure_time.to_string()` `"1/350 s"` (stored as 10/3500); f-number 7.1 (within 0.01);
      ISO 800; orientation 1; size 6240 × 4160; `widths` `[160, 4416]`; `read_preview` of the large
      one starts with `FF D8`;
    - `_DSF4271.RAF`: date `2020:05:31 06:16:09`, `"1/75 s"`, f/6.4.
- **Not in this task:** listing `raf` anywhere (T160); CR3; any change in `pw-libraw`, `pw-rawview`
  or `pw-thumbcache`.
- **Rules:** AGENTS.md §3 (no panic paths: the header's numbers come from the file and are not
  trusted). No new dependencies. The format is read from the facts above and public descriptions,
  not from any other program's code (AGENTS.md §4).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (besides files of other tasks still uncommitted in the tree). The ignored real-file tests are run
  once **before** the last plain `check.ps1`
  (`$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; .\tools\check.ps1 -Ignored`; it runs only the
  ignored tests and overwrites `test-output.txt`). The report gives the `pw-raw` lines of that run.
  Ignored tests of other crates that need files outside that folder (for example `pw-metadata`'s
  level-C samples) may fail there; they are named in the report, not fixed. The kept
  `test-output.txt` is the last plain run (README rule 6).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Open | Added after the owner's job test (bug 4 of 5); the owner approved RAF support (ADR 0003 point 5). Chain T159 → T160 |
| 2026-11-16 | Open | Cold read: the thumbnail offset is 3936 from the Exif base (4096 absolute); the Exif is little-endian; exposure compared as text; orientation starts at 1; `exif_thumbnail` named; a broken metadata directory gives no size instead of an error; the 16-byte magic read only for files of 16 bytes or more; the impossible `u64` overflow test replaced; `tiny_jpeg` shared; the `-Ignored` run order stated; `jpeg.rs` dropped from the list (no change needed). Left for T160 and the backlog: `PreviewSize::Medium` picks the 160 px thumbnail for RAF, and the contact sheet decodes the 4416 px JPEG per thumbnail |
| 2026-11-16 | Ready for review | `raf.rs` added (`inspect_raf`), `Format::Raf`, the 16-byte magic checked in `inspect_source`, `tiff::exif_thumbnail`; 7 unit tests in `raf.rs`, 2 in `tiff.rs`, 2 real-file tests. `-Ignored` run: `pw-raw` 1 + 6 passed; `pw-metadata` `every_real_dng_jpeg_and_tiff_is_rated_safely` failed on `_TN06333.JPG` (read-only in the data folder; not fixed). Plain `check.ps1`: all OK, 1679 passed, 65 ignored |
| 2026-11-16 | Done | Reviewed: no bugs. The reviewer's own runs: plain `.\tools\check.ps1` on the tree with T158 all OK (1679 passed, 65 ignored, 0 failed; 1665 with T159 and the committed tasks only), clippy without warnings; `-Ignored` with `PW_TESTDATA`: `pw-raw` 1 + 6 passed. The `pw-metadata` failure was `_TN06333.JPG` being read-only in the owner's data folder; the owner cleared the flag. The header of `docs/test-inventory.md` was set to this commit's count. Leftovers are T160's and the backlog's (named in the cold-read row) |
