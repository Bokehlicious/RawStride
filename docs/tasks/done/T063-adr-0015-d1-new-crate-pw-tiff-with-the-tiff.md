# T63. ADR 0015 D1: new crate `pw-tiff` with the TIFF decoder from the Tauri app — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's commit.

- **Can start now.** ADR 0015 accepted by the owner (D0, 2026-11-10), including `tiff` =0.9.1.
- **Crates:** new `pw-tiff`, `photoworkflow` (`src-tauri`).
- **Files:** new `crates/pw-tiff/Cargo.toml` and `crates/pw-tiff/src/lib.rs` (more module files in
  `crates/pw-tiff/src/` are fine); the root `Cargo.toml` (`[workspace.dependencies]` only);
  `src-tauri/Cargo.toml`; `src-tauri/src/tiff_view.rs`; `src-tauri/src/imaging.rs`; `Cargo.lock`
  (by cargo); `docs/test-inventory.md`. This task may change the three `Cargo.toml` files as written
  below and nothing else in them.
- **Why:** ADR 0015 §4.1, §4.4 and §6: RawStride and the Tauri app share one TIFF decoder; RawStride
  needs a memory limit and one decode at a time.
- **What:**
  1. **Crate:** `crates/pw-tiff/Cargo.toml` as the other internal crates (`version.workspace = true`
     and the other `*.workspace` keys, `[lints] workspace = true`), description "TIFF decoding to 8-bit
     RGB (ADR 0015)", one dependency `tiff = "=0.9.1"` with default features. Root `Cargo.toml`: add
     `pw-tiff = { path = "crates/pw-tiff" }` under "Internal crates" (the members glob
     `crates/*` already covers the folder).
  2. **Move from `src-tauri/src/imaging.rs`** to `pw-tiff`, unchanged in behaviour and messages:
     `Layout` (with `channels`), `Rgb8`, `to_rgb8`, `u16_to_u8`, `f32_to_u8`, and the test
     `conversions` with its assertions. In `to_rgb8`, replace the slice indexing `samples[..n * k]`
     with `samples.get(..n * k)` and the same "image data is short: …" error (rule 4); the result is
     the same. `imaging.rs` keeps `fit` and `downscale` and their tests (the RAW view uses them) and
     gets `pub use pw_tiff::Rgb8;` so `rawview.rs` and `main.rs` do not change.
  3. **Move from `src-tauri/src/tiff_view.rs`** to `pw-tiff`: `open`, `dimensions` and `decode_rgb8`.
     New signatures: `pub fn dimensions(path: &Path) -> Result<(u32, u32), TiffError>` and
     `pub fn decode_rgb8(path: &Path, limit: Option<usize>) -> Result<Rgb8, TiffError>`.
     - `limit = None` keeps today's `Limits::unlimited()` (the Tauri app). `Some(bytes)` starts from
       `Limits::default()` and sets `decoding_buffer_size` and `intermediate_buffer_size` to `bytes`
       (the core budget in RawStride, ADR 0015 Q4).
     - `pub enum TiffError { Io(String), Decode(String), Unsupported(String), TooLarge { limit: usize } }`
       with `Display` and `std::error::Error`. `Display` gives **exactly today's texts** for the first
       three ("{path}: {io error}", "TIFF: {error}", "TIFF colour type {other:?} is not supported
       yet", "TIFF sample format is not supported yet", "image data is short: …"). `tiff`'s
       `LimitsExceeded` becomes `TooLarge`, shown as "TIFF: the file needs more memory than the
       limit of {limit / 1 MiB} MB". The Swedish texts of ADR 0015 §5 come in D4, not here.
  4. **One decode at a time (ADR 0015 §4.4):** a private `static` `Mutex<()>` in `pw-tiff`, taken by
     `decode_rgb8` for the whole decode (a poisoned lock is taken with `into_inner`, as `tiff_view`'s
     `lock` does). `dimensions` does not take it.
  5. **Tauri app:** `src-tauri/Cargo.toml` replaces `tiff = "0.9"` with `pw-tiff.workspace = true`.
     `tiff_view.rs` keeps `TiffView`, the cache, `max_edge`, `encode_jpeg` and its own decode lock,
     and keeps the functions `dimensions(path) -> Result<_, String>` and `decode_rgb8(path) ->
     Result<Rgb8, String>` as one-line wrappers over `pw_tiff` (`limit = None`, `.map_err(|e|
     e.to_string())`), so `main.rs` does not change. Update the module comments of both files to say
     where the decoder now lives.
- **Rules:** a move with additions; no change in what the Tauri app shows or reports; rules 3–7 and
  10–12. Check with `git diff Cargo.lock`: the only changes are the new `pw-tiff` package and the
  dependency lists of `photoworkflow` and `pw-tiff`; **no new external package** (`tiff` 0.9.1 is
  already there). If cargo wants anything else, stop and report.
- **Tests to add** (in `pw-tiff`; build the files in the test with `tiff::encoder::TiffEncoder`, or
  by hand as `crates/pw-metadata/tests/sidecar.rs` does):
  - 8-bit RGB, 16-bit RGB, 8-bit Gray, 8-bit RGBA, 8-bit CMYK and 32-bit float RGB, each 2×2 or
    larger, decode to the expected RGB bytes; one LZW-compressed RGB file decodes to the same bytes
    as its uncompressed twin;
  - `dimensions` gives the encoded size;
  - a palette TIFF gives `Unsupported` (build it by hand: PhotometricInterpretation 3 with a
    ColorMap);
  - a truncated file (a valid file cut in half) gives an error, not a panic;
  - `Some(limit)` smaller than the image gives `TooLarge`; `None` and a large enough limit decode;
  - the lock: while a test holds the decode lock (a `#[cfg(test)]` accessor is fine), a
    `decode_rgb8` on another thread does not finish (`recv_timeout` of 200 ms gives a timeout); after
    the lock is released it finishes;
  - a file that does not exist gives `Io`.
- **Done when:** `.\tools\check.ps1` is all OK; `photoworkflow` has the same tests as before minus
  `conversions`, which `pw-tiff` now has, all passing; `docs/test-inventory.md` has a `pw-tiff` row
  and the new totals; `git diff --stat` shows only the files above.
