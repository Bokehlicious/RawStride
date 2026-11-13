# T90. ADR 0007 I9d: the real image check — previews with `pw-raw`, the DNG opened with LibRaw — Done

- **Can start now.** The owner approved D4 (the new crate `pw-imagecheck`) on 2026-11-12. It needs
  only the `ImageCheck` trait of T86, so it can run at the same time as T87–T89.
- **Crates:** new `pw-imagecheck`.
- **Files:** `Cargo.toml` (root: the D4 line under `[workspace.dependencies]`); `Cargo.lock` (by
  cargo); new `crates/pw-imagecheck/Cargo.toml`; new `crates/pw-imagecheck/src/lib.rs`; new
  `crates/pw-imagecheck/src/tests.rs`; new `crates/pw-imagecheck/tests/real_files.rs`;
  `docs/test-inventory.md`.
- **Why:** ADR 0007 §9.3 point 3 ("pw-raw finds the same previews (count, sizes, decodable JPEG)")
  and point 4 ("for DNG, LibRaw opens and unpacks the copy without a 'damaged' report"), §9.1
  point 2 (sessions closed first). `pw-metadata` must not depend on `pw-raw` or LibRaw, so T86
  takes the check from the caller; this crate has both. The rest of §9.3 point 3 (`DNGVersion`,
  `NewSubfileType`, the IFD chain) is already covered by T84's listing comparison, where any
  changed or missing key outside the allowed ones is an error. Measured by the reviewer: after a
  metadata write the embedded previews are **byte for byte equal**, only their offsets move.
- **What:**
  1. **`Cargo.toml` of the crate** (as D4): `pw-metadata`, `pw-raw`, `pw-libraw` (all
     `.workspace = true`), `[lints] workspace = true`, the workspace package fields as the other
     crates. Check with `git diff Cargo.lock`: only the `pw-imagecheck` package is added, **no
     other package**; otherwise stop and report.
  2. **`pub struct RawImageCheck`** with `RawImageCheck::new(release: impl Fn(&Path) + Send +
     Sync + 'static)` and `RawImageCheck::without_release()`; it implements
     `pw_metadata::safe_write::ImageCheck` and is `Send`:
     - `release(image)` calls the closure (the loupe passes T89's `Releaser`, T91);
     - **`check_copy(original, copy, kind)`:**
       - **previews** (every kind): `pw_raw::inspect` of both. When the original cannot be
         inspected (for example a BigTIFF), the preview check is skipped. Otherwise the copy
         must be inspected too, with the same number of previews, and each preview (by index)
         the same width, height and length and the **same bytes** (`pw_raw::read_preview`).
         Equal bytes decode exactly as the original's; this is the reviewer's reading of
         "decodable JPEG", and it never refuses a file whose own preview was already broken;
       - **`Dng`:** `pw_libraw::Session::open_with_diagnostics(copy)`; the session is dropped
         at once (the file is closed before the replace). When the copy does not open, the
         original is opened the same way: when it fails too, the check passes (the write did not
         make it worse); otherwise the error is the copy's LibRaw message;
       - an error is a text in English naming the check (`"preview 2 differs"`, `"the copy has 1
         preview, the original 2"`, `"LibRaw: damaged: …"`); the Swedish texts are T91's.
  3. Module comment with the reasons above (§9.3 points 3–4, why the bytes are compared).
- **Rules:** rules 3–7 and 10–12. The D4 lines are the only dependency change. The crate has no
  build script.
- **Tests to add:**
  - `src/tests.rs` (the JPEG fixture of `pw-thumbcache` via `#[path]`, as other crates do; files
    in a temporary folder): a file and an identical copy pass; a copy with one byte changed
    inside its embedded thumbnail (found with `pw_raw::inspect`) → an error naming the preview; a
    copy without the thumbnail → the count error; an original that cannot be inspected (a few
    bytes of text named `.tif`) → passes whatever the copy is; `release` calls the closure with
    the path; `without_release` does nothing;
  - `tests/real_files.rs`, **ignored by default** (`PW_TESTDATA`): every DNG copied and written
    with a City plan through `ExifTool::write_safely` with `RawImageCheck::without_release()` →
    `Written` (report the time of `check_copy` per file in the log row); every DNG with a copy cut
    inside its RAW data (as `pw-libraw/tests/damaged.rs` cuts) → `check_copy` fails with LibRaw's
    `damaged:` text; the cut copy checked against a cut original → passes; a DNG copy whose
    largest preview has one byte changed → the preview error.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
