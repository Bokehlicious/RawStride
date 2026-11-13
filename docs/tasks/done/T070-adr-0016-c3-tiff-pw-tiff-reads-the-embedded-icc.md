# T70. ADR 0016 C3 (TIFF): `pw-tiff` reads the embedded ICC profile and converts 16-bit data before the reduction — Done

- **Can start now.** T68 accepted. Independent of T69 and T71.
- **Crates:** `pw-tiff` only. Nothing calls the new function yet (that is C4); the Tauri app and
  `decode_rgb8` are unchanged.
- **Files:** `crates/pw-tiff/Cargo.toml`; `crates/pw-tiff/src/lib.rs`; `crates/pw-tiff/src/convert.rs`;
  `crates/pw-tiff/src/tests.rs`; `Cargo.lock` (by cargo); `docs/test-inventory.md`. This task may
  change `crates/pw-tiff/Cargo.toml` as written below and nothing else in it.
- **Why:** ADR 0016 §3.1 and §4.1: the owner's TIFFs are 16-bit ProPhoto RGB; the profile is in
  tag 34675 and must be applied **before** `u16_to_u8`, or the 8-bit result bands.
- **What:**
  1. `crates/pw-tiff/Cargo.toml`: `pw-colour.workspace = true` under `[dependencies]`; under
     `[dev-dependencies]` `moxcms = "=0.8.1"` (to encode ICC profiles in tests).
  2. **`pub enum SourceColour { Embedded, AssumedSrgb, Unusable(String) }`** (`Debug`, `Clone`,
     `PartialEq`, `Eq`): which source profile was used. `Embedded`: the file's ICC profile.
     `AssumedSrgb`: no profile, or a layout whose profile is not applied (below). `Unusable(detail)`:
     the file has a profile that could not be used (the `ColourError` text); the pixels are treated
     as sRGB.
  3. **`pub fn decode_rgb8_for_display(path: &Path, limit: Option<usize>, destination:
     &pw_colour::Profile, cache: &pw_colour::TransformCache) -> Result<(Rgb8, SourceColour),
     TiffError>`**, under the same decode lock as `decode_rgb8`, for the whole decode and
     conversion:
     - Read the ICC profile with `find_tag(Tag::Unknown(34675))` and `into_u8_vec` (the `tiff` crate
       reads type UNDEFINED, which Lightroom writes, as bytes). A tag that cannot be read counts as
       `Unusable`, never as an error of the decode.
     - **Source profile:** for `Layout::Rgb` and `Layout::Rgba`, the ICC profile through
       `Profile::from_icc` (`Embedded`), sRGB when there is none (`AssumedSrgb`), sRGB when it fails
       (`Unusable`). For Gray, GrayAlpha and CMYK the profile is **not** applied (a gray or CMYK
       profile does not describe the RGB that `to_rgb8` makes; ADR 0016 §3.2): sRGB,
       `AssumedSrgb`.
     - Get the transform with `cache.get(source, destination)`; a failure there is `Unusable` with
       sRGB as the source.
     - **16-bit RGB or RGBA:** make 16-bit RGB first (a new helper `to_rgb16` in `convert.rs`, the
       16-bit twin of `to_rgb8` for `Rgb` and `Rgba` only, with the same "image data is short" error;
       for `Rgb` it uses the samples without a copy when their length is exact), then
       `convert_rgb16`, then `u16_to_u8`. **8-bit and float:** today's path to `Rgb8`, then
       `convert_rgb8`. Gray, GrayAlpha and CMYK at 16 bit keep today's path (reduce, then
       `to_rgb8`, then `convert_rgb8` from sRGB).
     - A conversion error (cannot happen for whole pixels, but handled) returns the image as it is
       with `Unusable`.
  4. `decode_rgb8` and everything the Tauri app uses stay unchanged; their tests stay unchanged.
- **Rules:** rules 3–7 and 10–12. `git diff Cargo.lock`: only `pw-colour` and `moxcms` in
  `pw-tiff`'s dependency list; no new package.
- **Tests to add** (synthetic files as the existing `pw-tiff` tests; the ICC tag written with
  `encoder().write_tag(Tag::Unknown(34675), &bytes[..])`, plus **one hand-built file** with the tag
  as type UNDEFINED (7), as Lightroom writes it). The references are T68's tables
  (`PRO_PHOTO_TO_SRGB`, `SRGB_TO_ADOBE`); copy the values into `tests.rs`. Each channel within 1:
  - 8-bit RGB with a ProPhoto ICC (`ColorProfile::new_pro_photo_rgb().encode()`), destination sRGB:
    the ProPhoto → sRGB references, `Embedded`; the same with the hand-built UNDEFINED tag;
  - **16-bit** RGB with the ProPhoto ICC (values × 257), destination sRGB: the same references,
    `Embedded`;
  - 16-bit RGBA with the ProPhoto ICC: the same, alpha dropped;
  - 8-bit RGB without ICC, destination Adobe RGB: the sRGB → Adobe RGB references, `AssumedSrgb`;
  - **identity (F3):** 8-bit RGB, 16-bit RGB, Gray and CMYK without ICC, destination sRGB: pixels
    equal to `decode_rgb8` byte for byte, `AssumedSrgb`;
  - a 100-byte garbage ICC tag and a gray ICC on an RGB file: `Unusable`, and with destination sRGB
    the pixels equal `decode_rgb8`;
  - a Gray file **with** a gray ICC: `AssumedSrgb`, pixels equal `decode_rgb8` (destination sRGB);
  - the limit, a truncated file and a missing file give the same errors as `decode_rgb8`;
  - **16 bit is better than 8 bit:** a 16-bit ProPhoto ramp of 4 096 dark values (0 to 4 095,
    grey) converted to sRGB gives at least as many distinct 8-bit output values as the same ramp
    reduced to 8 bit first and then converted (shows that the conversion runs before the
    reduction);
  - **real files** (`#[ignore]`, `PW_TESTDATA`, as the existing real-file tests): every TIFF there
    decodes with `Embedded` against destination sRGB.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` has the new tests in the
  `pw-tiff` row and the new totals; `git diff --stat` shows only the files above.
