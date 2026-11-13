# T68. ADR 0016 C1: new crate `pw-colour` for colour conversion — Done

- **Can start now.** ADR 0016 accepted by the owner (C0, 2026-11-11), including `moxcms` =0.8.1.
- **Crates:** new `pw-colour` only. No other crate uses it yet (that is C2–C4).
- **Files:** new `crates/pw-colour/Cargo.toml` and `crates/pw-colour/src/lib.rs` (more module files
  in `crates/pw-colour/src/` are fine); the root `Cargo.toml` (`[workspace.dependencies]` only);
  `Cargo.lock` (by cargo); `docs/test-inventory.md`. This task may change the two `Cargo.toml` files
  as written below and nothing else in them.
- **Why:** ADR 0016 §4.1, §6 and Q3, Q5, Q6: one place that turns pixels from a source profile into
  the monitor profile, testable without a screen, for `pw-loupe`, `pw-viewcore`, `pw-thumbcache` and
  `pw-tiff` later.
- **What:**
  1. **Crate:** `crates/pw-colour/Cargo.toml` as `crates/pw-tiff/Cargo.toml` (the `*.workspace` keys,
     `[lints] workspace = true`), description "Colour conversion between ICC profiles (ADR 0016)",
     one dependency `moxcms = "=0.8.1"` with **default features** (the same features `image` already
     builds, so nothing new is compiled). Root `Cargo.toml`: add
     `pw-colour = { path = "crates/pw-colour" }` under "Internal crates", next to `pw-tiff`.
  2. **`Profile`** (`Clone`, `Debug`): a parsed RGB profile plus its identity.
     - `Profile::srgb()`, `Profile::adobe_rgb()`, `Profile::pro_photo_rgb()` from moxcms's
       `ColorProfile::new_srgb()`, `new_adobe_rgb()`, `new_pro_photo_rgb()`.
     - `Profile::from_icc(bytes: &[u8]) -> Result<Profile, ColourError>` with
       `ColorProfile::new_from_slice`. A profile whose data colour space is not RGB (gray, CMYK, Lab)
       gives `Unsupported`; bytes moxcms cannot parse give `Profile`. Never a panic, whatever the
       bytes (empty, garbage, a valid profile cut short).
     - The identity is a private `enum ProfileId { Srgb, AdobeRgb, ProPhotoRgb, Icc(Arc<[u8]>) }`
       (`Clone`, `PartialEq`, `Eq`, `Hash`); an ICC profile is identified by its bytes.
  3. **`Transform`** (must be `Send + Sync`): `Transform::new(source: &Profile, destination: &Profile)
     -> Result<Transform, ColourError>` builds an 8-bit and a 16-bit RGB → RGB transform with
     `create_transform_8bit` / `create_transform_16bit`, `Layout::Rgb` on both sides and
     `TransformOptions { rendering_intent: RenderingIntent::RelativeColorimetric,
     ..Default::default() }` (ADR 0016 Q6; moxcms has no black-point compensation).
     - **Identity:** when source and destination have the same `ProfileId`, no moxcms transform is
       built, `is_identity()` is true and the conversions return without touching the data (F3:
       bit-identical to today).
     - `convert_rgb8(&self, data: &mut [u8]) -> Result<(), ColourError>` and
       `convert_rgb16(&self, data: &mut [u16]) -> Result<(), ColourError>`, both **in place**, working
       through the data in chunks of at most 65 536 pixels with one scratch buffer, so a 40 MP image
       does not need a second full copy. A length that is not a multiple of 3 gives `Length` and
       leaves the data unchanged.
  4. **`TransformCache`** (`Default`, `Send + Sync`): `get(&self, source: &Profile, destination:
     &Profile) -> Result<Arc<Transform>, ColourError>`, keyed by the two `ProfileId`s in a
     `Mutex<HashMap<…>>`. The same pair gives the same `Arc` (built once, ADR 0016 §4.1). A failed
     build is returned and not cached. A poisoned lock is taken with `into_inner`, as `pw-tiff` does.
  5. **`ColourError`**: `Profile(String)`, `Unsupported(String)`, `Transform(String)`,
     `Length { len: usize }`, with `Display` and `std::error::Error`. English texts, as `pw-tiff`:
     "ICC profile: {detail}", "ICC profile is not RGB ({colour space:?})", "colour transform:
     {detail}", "pixel data of {len} values is not whole RGB pixels". Swedish texts come in the
     steps that show them.
  6. Module comment: what the crate does, with a reference to ADR 0016, and that it is not used yet.
- **Rules:** rules 3–7 and 10–12. Check with `git diff Cargo.lock`: the only changes are the new
  `pw-colour` package and its dependency list; **no new external package** (`moxcms` 0.8.1,
  `num-traits` and `pxfm` are already there). If cargo wants anything else, stop and report.
- **Tests to add** (in `pw-colour`; no file from the owner's computer, ADR 0016 §8.1). The reference
  values below were computed independently of moxcms (floating point, Bradford adaptation, sRGB
  piecewise curve, Adobe RGB gamma 563/256, ProPhoto gamma 1.8). Each channel must be **within 1**
  of them (F1). If moxcms is further off, stop and report: that is the signal for the `lcms2`
  fallback of ADR 0016 Q5, not a reason to widen the tolerance.
  - **sRGB → Adobe RGB, 8 bit:** (255,0,0) → (219,0,0); (0,255,0) → (144,255,60); (0,0,255) →
    (0,0,250); (255,255,255) → (255,255,255); (0,0,0) → (0,0,0); (128,128,128) → (127,127,127);
    (200,100,50) → (177,100,56); (30,160,220) → (93,159,217).
  - **ProPhoto RGB → sRGB, 8 bit:** (255,255,255) → (255,255,255); (0,0,0) → (0,0,0);
    (128,128,128) → (146,146,146); (150,110,90) → (200,118,105); (100,140,120) → (64,165,136);
    (180,160,140) → (213,171,154); (120,120,160) → (125,139,181).
  - **16 bit:** the ProPhoto → sRGB colours above as 16-bit values (8-bit × 257), converted with
    `convert_rgb16` and reduced as `pw_tiff::u16_to_u8` does (`v >> 8`, written out in the test; no
    dependency on `pw-tiff`), are within 1 of the same references (ADR 0016 §8.1: the 16-bit path
    equals the 8-bit path within one step).
  - **Identity (F3):** sRGB → sRGB is `is_identity()`, and a buffer of all 256 grey values plus the
    colours above is unchanged byte for byte, in 8 and 16 bit. The same for two `Profile`s from the
    same ICC bytes.
  - **ICC bytes:** `Profile::from_icc` of `ColorProfile::new_adobe_rgb().encode()` converts the
    sRGB → Adobe RGB colours above within 1 of the references (the path real files take).
  - **Bad profiles (F4):** empty bytes, 100 garbage bytes, and the first half of the encoded Adobe
    RGB profile give an error, never a panic; `ColorProfile::new_gray_with_gamma(2.2).encode()` gives
    `Unsupported`.
  - **Length:** 4 bytes give `Length { len: 4 }` and leave the data unchanged; an empty buffer is
    fine.
  - **Chunks:** an image of 65 536 × 2 + 7 pixels (a repeating pattern of the colours above) gives the
    same result as converting each pixel on its own.
  - **Cache:** two `get` calls with the same pair return `Arc::ptr_eq` transforms; a different pair
    gives a different one; a failing pair (gray ICC) returns the error.
  - **Threads:** a compile-time check that `Transform` and `TransformCache` are `Send + Sync`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` has a `pw-colour` row and the
  new totals; `git diff --stat` shows only the files above.
