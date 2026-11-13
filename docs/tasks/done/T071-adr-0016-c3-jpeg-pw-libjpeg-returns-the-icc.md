# T71. ADR 0016 C3 (JPEG): `pw-libjpeg` returns the ICC profile, `pw-raw` the Exif colour space — Done

- **Can start now.** Independent of T69 and T70. Nothing calls the new code yet (that is C4).
- **Crates:** `pw-libjpeg`, `pw-raw`.
- **Files:** `crates/pw-libjpeg/src/lib.rs`; `crates/pw-raw/src/lib.rs`; `crates/pw-raw/src/tiff.rs`;
  `crates/pw-raw/src/jpeg.rs` (only if the JPEG path needs it); `docs/test-inventory.md`. No
  `Cargo.toml` changes; `Cargo.lock` must not change.
- **Why:** ADR 0016 §3.1 and §4.1: a JPEG's source profile is the ICC profile in APP2, else the
  Exif Adobe RGB marker, else sRGB.
- **What:**
  1. **`pw-libjpeg`:** declare `tj3Set(handle, param, value) -> c_int`,
     `tj3GetICCProfile(handle, *mut *mut u8, *mut usize) -> c_int` and `tj3Free(*mut c_void)`
     (libjpeg-turbo 3.2.0 in vcpkg has them); `TJPARAM_SAVEMARKERS = 25` (its position in the
     `TJPARAM` enum of that `turbojpeg.h`; write that in a comment).
     - `Decoder::new` sets `TJPARAM_SAVEMARKERS` to `2` (keep ICC markers when reading a header); a
       failing `tj3Set` is an error from `new`.
     - New `pub fn icc_profile(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>>`: `header(data)`,
       then `tj3GetICCProfile`; a null buffer or size 0 is `Ok(None)`; otherwise copy the bytes into
       a `Vec` and free the buffer with `tj3Free` on every path. `// SAFETY:` comments as in the
       rest of the file. `decode` is unchanged in behaviour.
  2. **`pw-raw`:** `ImageInfo` gets `pub colour_space: ExifColourSpace` with
     `pub enum ExifColourSpace { #[default] Unknown, Srgb, AdobeRgb, Uncalibrated }`
     (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Default`), filled from the first Exif IFD that
     has it, like the other Exif fields:
     - ColorSpace (0xA001, SHORT) = 1 → `Srgb`;
     - ColorSpace = 0xFFFF and the Interoperability IFD (0xA005 in the Exif IFD) has
       InteroperabilityIndex (0x0001, ASCII) "R03" → `AdobeRgb` (the DCF rule cameras use);
     - ColorSpace = 0xFFFF otherwise → `Uncalibrated`; no ColorSpace → `Unknown`; any other value
       → `Unknown`.
     For a RAW file this is the camera's colour-space setting, which its embedded JPEG previews
     follow; for a JPEG file it comes from its APP1 Exif. Malformed values are ignored, never an
     error of `inspect`.
- **Rules:** rules 3–7 and 10–12.
- **Tests to add:**
  - `pw-libjpeg`: the 16 × 8 JPEG fixture of `pw-thumbcache`
    (`#[path = "../../pw-thumbcache/src/jpeg_fixture.rs"] mod jpeg_fixture;` in the test module)
    gives `Ok(None)`; the same JPEG with one APP2 segment `ICC_PROFILE\0` + sequence 1 + count 1 +
    100 test bytes inserted after SOI gives exactly those bytes; with the bytes split over **two**
    APP2 segments (sequence 1 and 2 of 2) gives them joined; `decode` of the JPEG with APP2 gives the
    same pixels as without; garbage data gives an error.
  - `pw-raw`: generated files as the existing tests (`lib.rs` builds TIFF and JPEG structures by
    hand): ColorSpace 1 → `Srgb`; 0xFFFF with "R03" → `AdobeRgb`; 0xFFFF with "R98" → `Uncalibrated`;
    0xFFFF without an Interop IFD → `Uncalibrated`; no ColorSpace → `Unknown`; a JPEG file with
    APP1 Exif ColorSpace 1 → `Srgb`; an Interop offset pointing outside the file → no panic,
    `Uncalibrated`.
  - `pw-raw` real files (`#[ignore]`, `PW_TESTDATA`): print each file's `colour_space`, assert
    nothing (the reviewer reads the output).
- **Done when:** `.\tools\check.ps1` is all OK; `git diff Cargo.lock` is empty;
  `docs/test-inventory.md` has the new tests and totals; `git diff --stat` shows only the files above.
