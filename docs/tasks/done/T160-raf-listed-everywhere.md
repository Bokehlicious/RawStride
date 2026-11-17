# T160. Fujifilm RAF listed: viewer, import and sidecar — Done

- **Why:** the second and last task for RAF (owner, 2026-11-16, after the job test). T159 lets
  `pw-raw` read a RAF file. This task adds `raf` to the two lists that decide what RawStride
  shows, imports and rates:
  - `pw_raw::SUPPORTED_EXTENSIONS` (`crates/pw-raw/src/lib.rs`). The loupe and the contact sheet
    (`pw-loupe` `is_listed`), the inspector (`exif_supported`) and the ingest's scan (`pw-ingest`
    `is_image`) all use it. Today an import leaves RAF files on the card without a word.
  - `SIDECAR_EXTENSIONS` (`crates/pw-metadata/src/lib.rs`): ratings, labels and IPTC for a RAF go
    to an XMP sidecar next to it, as for the other manufacturer RAW files (ADR 0007, table in §4).

  LibRaw already opens and renders RAF, X-Trans included (`crates/pw-libraw/tests/statistics.rs`).
  This task touches two crates, but only one constant in each, plus their tests.
- **Crates:** `pw-raw`, `pw-metadata` (and tests in `pw-ingest`, `pw-loupe`). **Files:**
  `crates/pw-raw/src/lib.rs`, `crates/pw-metadata/src/lib.rs`, `crates/pw-metadata/tests/sidecar.rs`,
  `crates/pw-ingest/tests/ingest.rs`, `crates/pw-loupe/src/lib.rs`, `crates/pw-loupe/src/inspector.rs`,
  `docs/test-inventory.md`.
- **What:**
  1. Add `"raf"` to `SUPPORTED_EXTENSIONS`, after `"srw"`.
  2. Add `"raf"` to `SIDECAR_EXTENSIONS`, after `"srw"`.
  3. Nothing else changes. If a test or a match elsewhere fails because of the new extension or
     T159's new `Format::Raf` (for example in `src-tauri` or `pw-thumbcache`), stop and report it.
     Do not change that file.
- **Tests to add:**
  - `pw-loupe` `lib.rs`: a **new** small test (not in `tiff_files_are_listed`): `is_listed` is true
    for `a.RAF` and `a.raf`.
  - `pw-loupe` `inspector.rs`: `a.RAF` added to the first name list of the test that checks
    "Every listed kind is read" (about line 876).
  - `pw-metadata` `tests/sidecar.rs`, in `which_file_holds_the_rating`: `supports_sidecar` is true
    for `a.RAF`, and `sidecar_path` of `x/_DSF4243.RAF` is `x/_DSF4243.xmp` (as for ARW).
  - `pw-ingest` `tests/ingest.rs`: a **new** test with its own card (not the shared `card()`, whose
    exact counts other tests assert): `DCIM/100_FUJI/_DSF0001.RAF` with bytes made in the test (its
    inspect fails, so the time falls back to the file date, as for the other fake files) is
    scanned as one shot and planned for copying.
- **Not in this task:** CR3; thumbnails and rendering (they follow from the lists). The Tauri app
  (`src-tauri`) is not developed further (owner, 2026-11-16): it only has to keep compiling, and
  nothing is checked or changed there for RAF.
- **Rules:** AGENTS.md §3; no new dependencies.
- **Hand check (owner):**
  - open `D:\PhotoWorkflowData\RAW`: the five X-T30 files are in the contact sheet with
    thumbnails;
  - the loupe shows the embedded JPEG, and `R` (RAW ↔ JPEG) shows the RAW rendering with the right
    colours and orientation;
  - the histogram and the clipping overlays work;
  - a rating and a label written to a RAF end up in `_DSF….xmp` next to it;
  - an import of a folder with RAF files copies them;
  - how fast the contact sheet fills with RAF thumbnails. A RAF has no preview between 160 and
    4416 px, so each thumbnail decodes the large JPEG; if it is slow, the backlog item "RAF
    previews" gets a task.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (besides files of other tasks still uncommitted in the tree).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Blocked | Added with T159 (bug 4 of 5). Waits for T159 |
| 2026-11-16 | Blocked | Cold read: the two constants are the only production change, and every other extension check treats RAF right; the ingest test gets its own card; a new `is_listed` test; the thumbnail speed is in the hand check and the backlog ("RAF previews") |
| 2026-11-16 | Open | T159 is Done; the text is unchanged |
| 2026-11-17 | Ready for review | `raf` added to `SUPPORTED_EXTENSIONS` and `SIDECAR_EXTENSIONS`; the four tests added; nothing else failed; `check.ps1` all OK (1681 passed, 65 ignored, 0 failed) |
| 2026-11-17 | Done | Reviewed: no bugs. Reviewer's `.\tools\check.ps1` on the tree with T160, T161, T163 and T164: all OK, 1694 passed, 65 ignored, 0 failed; clippy without warnings; committed one task at a time, the inventory's header set to this commit's count (1681). The owner's hand check is done after the commit (owner, 2026-11-17); its result, and whether the contact sheet is slow, decides the backlog item "RAF previews" |
