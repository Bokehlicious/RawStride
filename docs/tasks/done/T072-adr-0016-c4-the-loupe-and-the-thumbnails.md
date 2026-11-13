# T72. ADR 0016 C4: the loupe and the thumbnails convert to the monitor profile — Done

- **Can start now.** T68–T71 are done: `pw-colour` (transforms), the monitor profile in `pw-loupe`
  (T69), `pw_tiff::decode_rgb8_for_display` (T70), `Decoder::icc_profile` and
  `ImageInfo::colour_space` (T71). **This is the step that changes what the owner sees.**
- **Crates:** `pw-colour`, `pw-viewcore` (feature `native`), `pw-thumbcache`, `pw-loupe`.
- **Files:** `crates/pw-colour/src/lib.rs`, `crates/pw-colour/src/tests.rs`;
  `crates/pw-viewcore/Cargo.toml`, `crates/pw-viewcore/src/native.rs`,
  `crates/pw-viewcore/tests/native_files.rs`; `crates/pw-thumbcache/Cargo.toml`,
  `crates/pw-thumbcache/src/extract.rs`, `crates/pw-thumbcache/src/worker.rs`;
  `crates/pw-loupe/src/main.rs`; `Cargo.lock` (by cargo); `docs/test-inventory.md`. The two
  `Cargo.toml` files may change only as written below.
- **Why:** ADR 0016 §2, §3.1, §4.1 ("Applying it in the loupe", "Thumbnails"), §4.2 (caches hold
  display-ready pixels), F3, F5, F6.
- **Not in this task:** the status-line note for an embedded profile that cannot be used (ADR 0016
  §5) needs a new field through the loupe core; it is a later task. Here an unusable profile only
  falls back to sRGB.
- **What:**
  1. **`pw-colour`: the JPEG rule in one place.** `pub fn jpeg_source_profile(icc: Option<&[u8]>,
     exif_adobe_rgb: bool) -> Profile` (ADR 0016 §3.1): the ICC profile through
     `Profile::from_icc` when there is one and it parses as RGB; else `Profile::adobe_rgb()` when
     `exif_adobe_rgb`; else `Profile::srgb()`. An ICC profile that cannot be used falls through to
     the Exif rule, then sRGB. Never an error.
  2. **`pw-viewcore`** (`Cargo.toml`: `pw-colour = { workspace = true, optional = true }` and
     `"dep:pw-colour"` in the `native` feature):
     - `NativeBackend` gets `destination: pw_colour::Profile` and `cache:
       Arc<pw_colour::TransformCache>`. It becomes `Clone` instead of `Copy` (nothing needs `Copy`;
       check with the compiler). `Default` and `with_tiff_limit` use `Profile::srgb()`, so every
       existing test sees identity transforms and unchanged pixels (F3). New
       `pub fn with_monitor_profile(self, profile: pw_colour::Profile) -> NativeBackend`.
     - **TIFF:** `open_tiff` calls `pw_tiff::decode_rgb8_for_display(path, Some(limit),
       &destination, &cache)` instead of `decode_rgb8` (the `SourceColour` is not used yet). The
       session holds the converted image, so `render_half` and `render_region` stay as they are.
     - **JPEG** (embedded preview and plain JPEG file): `open_jpeg` gets the transform once from
       `jpeg_source_profile(decoder.icc_profile(&data)?, info.colour_space ==
       ExifColourSpace::AdobeRgb)` (`icc_profile` before `header`; an `icc_profile` error is a
       `jpeg_error` like `header`'s) and the cache, and keeps it in `JpegSession` as
       `Arc<Transform>`. `render_half` converts its output; the full decode for `render_region` is
       converted **once** when it is made, before it is stored in `full`, so crops need no work.
     - **RAW:** LibRaw renders sRGB (ADR 0006): `render_half` and `render_region` convert their
       output with the sRGB → destination transform. Keep it in `NativeSession::Raw` next to the
       LibRaw session (for example `Raw(Session, Arc<Transform>)` or a small struct).
     - A conversion error (cannot happen for whole RGB pixels) is a `DecodeError::new` with the
       `ColourError` text.
  3. **`pw-thumbcache`** (`Cargo.toml`: `pw-colour.workspace = true` under `[dependencies]`;
     `moxcms = "=0.8.1"` under `[dev-dependencies]` for test profiles):
     - `thumbnail(path, cell, decoder)` gets two more parameters, `destination: &Profile` and
       `cache: &TransformCache`.
     - **JPEG and embedded previews:** the source profile with `jpeg_source_profile` as in the
       loupe (`decoder.icc_profile(&bytes)` and `info.colour_space`); convert **after** the scaled
       decode and before `orient` (the thumbnail is small, so this is cheap).
     - **TIFF:** `pw_tiff::decode_rgb8_for_display(path, Some(TIFF_LIMIT), destination, cache)`
       instead of `decode_rgb8`, then `fit`, `downscale` and `orient` as today.
     - `FileThumbSource` holds the destination and an `Arc<TransformCache>` shared by the worker
       threads. New `pub fn start_with_monitor_profile(profile: Profile) -> io::Result<ThumbWorker>`
       (the default thread count and cache limit); `start()` and `start_with_limits` use sRGB.
  4. **`pw-loupe/src/main.rs`:** the backend becomes
     `NativeBackend::with_tiff_limit(…).with_monitor_profile(monitor_profile.profile.clone())` and
     the thumbnails `ThumbWorker::start_with_monitor_profile(monitor_profile.profile.clone())`.
     Remove the `#[allow(dead_code)] // used by ADR 0016 C4` on `monitor_profile` only if the field
     is now read; if not, keep the field and the allow (the status-line task will read it).
- **Rules:** rules 3–7 and 10–12. `git diff Cargo.lock`: only `pw-colour` (and `moxcms` for
  `pw-thumbcache`'s tests) in the dependency lists of `pw-viewcore` and `pw-thumbcache`; no new
  package. Every existing test keeps passing **unchanged**: with an sRGB destination nothing may
  change (F3). If one has to change, stop and explain.
- **Tests to add** (synthetic files as the existing tests of each crate; ICC profiles from
  `moxcms`; a pixel is "converted" when it equals the plain result run through
  `TransformCache::get(source, destination)` and `convert_rgb8`, byte for byte):
  - `pw-colour`: `jpeg_source_profile` checked by identity (`Transform::new(&result,
    &expected).is_identity()`): an Adobe RGB ICC with or without the marker → the profile from
    `Profile::from_icc` of the same bytes; no ICC and the marker → `Profile::adobe_rgb()`; no ICC and
    no marker → `Profile::srgb()`; a garbage or gray ICC and the marker → `Profile::adobe_rgb()`; a
    garbage ICC and no marker → `Profile::srgb()`.
  - `pw-viewcore` (`native.rs` tests):
    - a TIFF with a ProPhoto ICC and the backend's destination sRGB: the opened image equals
      `decode_rgb8_for_display` for that file; the same TIFF with the default backend and a TIFF
      without ICC: equal to `decode_rgb8` (F3);
    - a plain JPEG file (the `pw-thumbcache` fixture through `#[path]`, as T71 did) with
      destination Adobe RGB: `render_half` and a `render_region` are the plain decodes converted
      sRGB → Adobe RGB; with destination sRGB: the plain decodes byte for byte;
    - the same JPEG with an APP2 ICC holding the Adobe RGB profile and destination Adobe RGB
      parsed from **the same bytes**: unchanged (identity by ICC bytes);
    - the same JPEG with an APP1 Exif ColorSpace 0xFFFF and InteroperabilityIndex "R03" (build it
      as the `pw-raw` tests of T71 do) and destination sRGB: converted Adobe RGB → sRGB.
  - `pw-viewcore` real files (`tests/native_files.rs`, `#[ignore]`, `PW_TESTDATA`): for one
    camera RAW in RAW mode and in JPEG mode, a backend with destination Adobe RGB gives the
    `render_half` of the sRGB backend converted sRGB → Adobe RGB (JPEG mode: from the file's own
    source profile); print the time of each `render_half` with and without conversion.
  - `pw-thumbcache`: the fixture JPEG thumbnail with destination Adobe RGB is the sRGB thumbnail
    converted; a uniform ProPhoto TIFF of (150,110,90) with its ICC and destination sRGB gives
    (200,118,105) within 1 in every pixel (T68's reference); with destination sRGB and no ICC,
    thumbnails equal today's byte for byte.
- **Check by hand (owner, after review; ADR 0016 C5):** the ProPhoto TIFFs are no longer dull, sRGB
  images are no longer oversaturated, the `_TN0633x` Adobe RGB shots look like Lightroom in JPEG
  mode, side by side with Lightroom on the CX240 (F5); `latency.ps1` on the TIFF and RAW sets (F6).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` has the new tests and
  totals; `git diff --stat` shows only the files above.
