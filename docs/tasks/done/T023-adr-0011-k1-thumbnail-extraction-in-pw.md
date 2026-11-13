# T23. ADR 0011 K1: thumbnail extraction in pw-thumbcache — Done

- **Crate:** `pw-thumbcache`. **Files:** `crates/pw-thumbcache/Cargo.toml` (add the internal
  dependency `pw-libjpeg.workspace = true`; `pw-raw` is already there), `crates/pw-thumbcache/src/`
  (e.g. a new `extract.rs`, declared in `lib.rs`).
- **What (ADR 0011 §3.1, §4.1):**
  1. `pub fn choose_preview(info: &pw_raw::ImageInfo, cell: u32) -> Option<&pw_raw::PreviewRef>`:
     the smallest embedded preview whose **long edge** is at least `cell`; if none is that large, the
     largest one; `None` if there is no preview.
  2. `pub fn choose_scale(width: u32, height: u32, cell: u32) -> pw_libjpeg::Scale`: the smallest
     decode (Eighth, then Quarter, then Half, then Full) whose long edge is still at least `cell`
     (use `Scale::size`).
  3. `pub fn orient(image: pw_libjpeg::Rgb8, orientation: u16) -> pw_libjpeg::Rgb8`: applies the EXIF
     orientation in pixels, all eight values (1 = as is, 2 = mirror, 3 = 180°, 4 = flip, 5 =
     transpose, 6 = 90° clockwise, 7 = transverse, 8 = 270° clockwise); unknown values act as 1.
  4. `pub fn thumbnail(path: &Path, cell: u32, decoder: &mut pw_libjpeg::Decoder) -> Result<Rgb8, ThumbError>`:
     `pw_raw::inspect`, then for a plain JPEG file (`Format::Jpeg`) read and decode the file itself,
     otherwise `choose_preview` + `pw_raw::read_preview`; decode with `choose_scale`; apply `orient`
     with the file's orientation.
     `ThumbError` is an enum with at least `NoPreview` (the "ingen förhandsvisning" case) and
     `Unreadable(String)` (I/O, format or decode errors), and implements `Display`.
- **Rules:** never decode RAW data (G1); no `unwrap`/`expect`/`panic!`/unchecked indexing in non-test
  code; no new external crates. Update the crate description in `Cargo.toml` to "Thumbnail
  extraction, background generation and an in-memory cache (disk cache later)".
- **Tests to add:**
  - `choose_preview`: several previews (picks the smallest large enough), none large enough (picks the
    largest), no previews (`None`), a portrait preview (long edge is the height);
  - `choose_scale`: e.g. 1616×1080 with cell 200 → Eighth (202 px), with cell 300 → Quarter;
    160×120 with cell 200 → Full; exact boundaries (corrected in review: the first example said
    Quarter by mistake);
  - `orient`: a small 2×3 image with distinct pixels, checked for all eight values (size and pixel
    positions);
  - `thumbnail`: a non-existent file → `Unreadable`; a file that is not an image → `Unreadable`, not a
    panic; a small JPEG written to a temporary file (you may reuse the small JPEG bytes of
    `crates/pw-libjpeg/tests/decode.rs`) → a thumbnail of the expected size;
  - real files (ignored by default, the T6 pattern): one thumbnail per camera format of the test set,
    with the oriented width/height checked for an A7R IVA vertical image (`TN100423.ARW`, orientation 8).
- **Done when:** `.\tools\check.ps1` and `.\tools\check.ps1 -Ignored` are all OK.
