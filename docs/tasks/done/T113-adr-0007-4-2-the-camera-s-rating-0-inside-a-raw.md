# T113. ADR 0007 §4.2: the camera's rating 0 inside a RAW file counts as no rating — Done

- **Can start now** (owner decision 2026-11-13, after the hand check of T110). It does not touch
  `pw-loupe` and can run at the same time as a `pw-loupe` task (T111 or T112).
- **Crates:** `pw-metadata` only.
- **Files:** `crates/pw-metadata/src/reconcile.rs` (the rule, the module comment and the tests);
  `docs/adr/0007-metadata-iptc.md` (§4.2 and one row in "Revision history"); `docs/test-inventory.md`.
- **Why:** Sony cameras (seen on the A1 and the A7R IVA) write `xmp:Rating` = `0` into the XMP
  inside the ARW when the image has no rating in the camera. When the owner rates the image in
  RawStride, the rating goes to the sidecar (§4.2), and `reconcile_camera_xmp` then reports a
  conflict between the sidecar's value (for example 1) and the camera's 0, with rule `Xmp`. So
  every rated Sony RAW shows "Konflikter · Betyg · 1 (XMP-sidofil) · 0 (XMP) · XMP används" in the
  inspector and the `M` window, although nothing differs that the user cares about. In RawStride
  a rating of 0 already means "no stars": `RatingKey::Stars(0)` removes the stars (ADR 0013).
- **Owner's decision (2026-11-13):** in the XMP **inside a RAW file** (camera XMP, step 3 of
  `reconcile`), an `xmp:Rating` whose value is zero is **no rating**: it neither fills the
  property nor gives a conflict. Every other camera rating (1–5, and −1 for rejected) is unchanged:
  it fills when the sidecar has no rating, and a different sidecar rating is a conflict with rule
  `Xmp` as today (a rating set in the camera and changed in RawStride is a real difference).
- **Not in this task:** the rating path of `pw-loupe` (`read_ratings`, the rating store, the
  filter), which reads the ratings apart from `reconcile` and is unchanged; a rating of 0 in the
  **primary** XMP (a JPEG's, TIFF's or DNG's embedded XMP, or a RAW file's sidecar), which stays a
  value as today; the notice "Förslag till Skapad från kameran"; how the inspector or the `M`
  window shows conflicts.
- **What:**
  1. **`reconcile.rs`:** in step 3 (camera XMP, `Raw` only) a value of `xmp:Rating` that is a
     number equal to zero (`0`, also written `0.0` or with surrounding spaces) is skipped before
     `reconcile_camera_xmp`, the same way a blank value is skipped today. It is therefore also not
     reported as `Notice::HiddenByUser` when the user cleared the rating in the sidecar (§6.4):
     there is nothing to hide. A value that is not a number is not affected by this rule.
  2. **The module comment** of `reconcile.rs` (point 3 of its list) says that the camera's
     `xmp:Rating` 0 counts as no rating (§4.2).
  3. **ADR 0007 §4.2**, the bullet "Embedded metadata in the RAW": one added sentence saying that
     a camera `xmp:Rating` of 0 counts as no rating (it neither fills nor gives a conflict), with
     "(owner, 2026-11-13; Sony writes 0 for an unrated image)". One new row in "Revision history"
     (number 10, 2026-11-13) naming the change. Nothing else in the ADR changes.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. No other rule of `reconcile` changes;
  the existing tests stay as they are.
- **Tests to add** (in `reconcile.rs`, with the existing `readings`/`values` helpers):
  - a RAW with sidecar rating 1 and camera rating 0: the sidecar's 1 with origin `XmpSidecar`, and
    no conflict;
  - a RAW with no sidecar rating and camera rating 0: no `xmp:Rating` value at all, and no conflict;
  - `0.0` and ` 0 ` in the camera XMP: the same as `0`;
  - a RAW with sidecar rating 1 and camera rating 3: the conflict with rule `Xmp`, as today;
  - a RAW with no sidecar rating and camera rating −1: filled with origin `XmpEmbedded`;
  - a RAW whose sidecar cleared the rating (`cleared`) and camera rating 0: no
    `Notice::HiddenByUser`;
  - a JPEG (`FileKind::Jpeg`) with rating 0 in its primary XMP: still the value 0.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: rating an
  unrated Sony ARW (A1 and A7R IVA) shows no conflict in the inspector or the `M` window; an ARW
  rated in the camera and given another rating in RawStride still shows the conflict.
