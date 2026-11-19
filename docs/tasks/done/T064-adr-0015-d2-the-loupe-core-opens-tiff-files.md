# T64. ADR 0015 D2: the loupe core opens TIFF files — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's commit.

- **Can start now.** Independent of T65 (they share only `docs/test-inventory.md` and this file).
- **Crates:** `pw-viewcore`, `pw-loupe` (one line).
- **Files:** `crates/pw-viewcore/Cargo.toml`, `crates/pw-viewcore/src/native.rs`,
  `crates/pw-viewcore/src/state.rs`, `crates/pw-viewcore/src/lib.rs` (re-export),
  `crates/pw-viewcore/tests/native_files.rs`, `crates/pw-loupe/src/main.rs` (the `NativeBackend`
  construction in `run` only), `Cargo.lock` (by cargo), `docs/test-inventory.md`.
- **Why:** ADR 0015 §4.1–§4.4 and §5: a TIFF is shown as itself in both display modes, decoded once
  with `pw-tiff` under the core budget.
- **What:**
  1. **Cargo:** `pw-tiff = { workspace = true, optional = true }` under `[dependencies]` and
     `"dep:pw-tiff"` added to the `native` feature (update its comment); `tiff = "=0.9.1"` under
     `[dev-dependencies]` (to write test files; already in `Cargo.lock`).
  2. **`state.rs`:** a new `pub fn is_shown_as_itself(path: &Path) -> bool`: true for the extensions
     jpg, jpeg, tif, tiff (any case). `source_in` uses it instead of `is_plain_jpeg`, so a TIFF is
     `Source::Jpeg` in both modes (ADR 0015 Q3). `is_plain_jpeg` stays unchanged (pw-loupe uses it for
     the "JPEG-fil" text until T66). Re-export the new function from `lib.rs` next to it.
  3. **`native.rs`, the limit:** `NativeBackend` gets a private field `tiff_limit: usize` (keeps
     `Clone, Copy, Debug`); `impl Default` = `4 << 30` (the default core budget);
     `pub fn with_tiff_limit(bytes: usize) -> NativeBackend`. `pw-loupe` `run`:
     `NativeBackend::with_tiff_limit(usize::try_from(config.core_budget).unwrap_or(usize::MAX))`
     instead of `NativeBackend` (ADR 0015 Q4). `native_files.rs`: `NativeBackend` →
     `NativeBackend::default()`, nothing else in the existing tests.
  4. **`native.rs`, the session:** `NativeSession::Tiff(Rgb)` holding the full decoded image.
     `open` checks the file type **first**, for either `Source`: a TIFF (`pw_raw::is_tiff_file`) goes
     to a new `open_tiff(path, limit)`:
     - `pw_tiff::decode_rgb8(path, Some(limit))`, converted to `Rgb` (same three fields);
     - the orientation from `pw_raw::inspect(path)`; if inspect fails (BigTIFF), orientation 1
       (ADR 0015 §4.3); `ImageSize { w, h, flip: flip_of_orientation(orientation) }` with the
       decoded size.
     - `render_half`: `scale::downscale` of the full image to `scale::scaled_size(w, h, 0.5)`;
       `render_region`: `scale::crop` as for the JPEG's full decode, with the same error texts
       ("the TIFF was not decoded" is not needed: the image is always there).
     - `session_bytes` needs no change: a TIFF is keyed `Source::Jpeg`, which already counts
       `w × h × 3`, the decoded RGB.
  5. **Error texts (Swedish, ADR 0015 §5; the loupe shows them after "Filen kan inte visas: "):**
     `TiffError::Unsupported(m)` → "TIFF-varianten stöds inte än ({m})";
     `TiffError::TooLarge { limit }` → "TIFF-filen är för stor för minnesbudgeten ({limit / 1 MiB}
     MB)"; `Io` and `Decode` → their own text. Never `DecodeError::damaged` (that is for RAW data,
     G3). Put the mapping in a small function `tiff_error(e: pw_tiff::TiffError) -> DecodeError` so
     it can be tested.
- **Rules:** no change of `Source`, of the cache keys or of RAW/JPEG behaviour; rules 3–7 and 10–12.
  `git diff Cargo.lock`: only the new dependency lines of `pw-viewcore`; no new package.
- **Tests to add:**
  - `state.rs`: `is_shown_as_itself` for `a.tif`, `b.TIFF`, `c.jpg`, not for `d.arw`, `e.dng`, `f`;
    a TIFF is keyed `Source::Jpeg` in RAW mode and in JPEG mode, and has no "other mode" entry
    (follow `a_plain_jpeg_file_is_shown_as_itself_in_both_modes`).
  - `native.rs` (write the files with `tiff::encoder::TiffEncoder` into a test folder of its own):
    a 6×4 8-bit RGB TIFF opens with `w = 6, h = 4`, `flip = 0`, and `render_half` gives 3×2 and
    `render_region` gives exactly the requested pixels; opening it with `Source::Raw` gives the same
    TIFF session (the file type wins); a TIFF with Orientation 6 (write the tag with the encoder's
    directory API, or by hand) gives `flip = 6`; a palette TIFF (copy `palette_tiff` from
    `crates/pw-tiff/src/tests.rs`) gives the "TIFF-varianten stöds inte än" text and `damaged =
    false`; a limit smaller than the image gives the "för stor för minnesbudgeten" text;
    `tiff_error` for all four kinds.
  - `native_files.rs`, ignored like the others (needs `PW_TESTDATA`): every `.tif` / `.tiff` in
    `PW_TESTDATA` (a new helper; at least one must exist) is shown in fit view with
    `Source::Jpeg` in **both** modes, at the size of `pw_tiff::dimensions`, without an error.
- **Done when:** `.\tools\check.ps1` is all OK; the owner runs `.\tools\check.ps1 -Ignored` with
  `PW_TESTDATA` set and the new real-file test passes on the owner's 9 TIFFs; `git diff --stat` shows only
  the files above.
