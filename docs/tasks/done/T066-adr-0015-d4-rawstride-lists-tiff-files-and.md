# T66. ADR 0015 D4: RawStride lists TIFF files and names them in the status line — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Can start now.** T64 and T65 are done: the core opens TIFF files and the thumbnail worker
  makes their thumbnails, so listing them is the last step before the owner's acceptance (D5).
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/lib.rs` (functions and tests);
  `docs/test-inventory.md`. No change of `main.rs` is expected; if one is needed, stop and explain.
- **Why:** ADR 0015 §3.1 and §5: TIFF files are listed in the grid and the loupe, and the status line
  names the real source (GR9). The owner accepted replacing the test that pins the old rule (ADR
  0015 D0, §8.1).
- **What:**
  1. **Listing:** `is_listed` lists every extension of `pw_raw::SUPPORTED_EXTENSIONS`, TIFF
     included (remove `&& !pw_raw::is_tiff_file(path)`). Update its doc comment: "the file types of
     the current app (TIFF since ADR 0015)".
  2. **Status texts for a TIFF** (`frame.path` is a TIFF by `pw_raw::is_tiff_file`; the core keys it
     `Source::Jpeg`, ADR 0015 §4.2). Add a private helper, for example `fn is_tiff(frame: &Frame) ->
     bool`, and use it in:
     - `source_text`: "TIFF-fil{size}" (before the JPEG cases), for example "TIFF-fil 7741 × 5161";
     - `status_line` while preparing without a size: "förbereder TIFF" instead of "förbereder JPEG";
     - `indicator_text`: "Förbereder TIFF …" instead of "Förbereder JPEG …".
     RAW files, embedded JPEGs and JPEG files keep their texts exactly.
  3. **Usage text** (`USAGE`): `--core-mb` "memory for RAW sessions and decoded JPEGs and TIFFs
     (default 4096)". Nothing else in `USAGE`.
- **Rules:** rules 4–7 and 10–12. No change of `error_text`: the TIFF error texts come from
  `pw-viewcore` (T64) after "Filen kan inte visas: ". No `Cargo.toml` change.
- **Test to replace (owner's approval, ADR 0015 D0):** `the_folder_lists_supported_files_by_name_without_tiff`
  becomes `the_folder_lists_supported_files_by_name`, with the same files and steps; only the
  expected list changes to `["a.jpg", "b.ARW", "C.dng", "d.tif", "f.rw2"]` (`e.xmp`, `notes.txt` and
  the folder `sub.arw` stay out; `C.dng` stays at position 2). Every other test keeps its assertions.
- **Tests to add:** `is_listed` for `a.tif`, `b.TIFF`, `c.tiff`; `source_text` of a ready TIFF frame
  (path `D:/Scans/Scan 01.tif`, `Source::Jpeg`, a size with `flip = 6`) is "TIFF-fil" with the oriented
  size, in both `mode` values; `status_line` of a preparing TIFF frame without a size contains
  "förbereder TIFF"; `indicator_text` of a TIFF frame is "Förbereder TIFF …"; a `.JPG` frame still
  gives "JPEG-fil" and "Förbereder JPEG …".
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the files above; and
  the owner checks by hand that `D:\PhotoWorkflowData\RAW` shows its TIFFs in the grid (thumbnails)
  and the loupe, with "TIFF-fil W × H" in the status line in both modes. The full acceptance with
  ratings, zoom, errors and timing is step D5 (ADR 0015 §8.2).
