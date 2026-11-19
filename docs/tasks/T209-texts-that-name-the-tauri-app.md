# T209. Comments and documents that still name the Tauri app as if it existed — Blocked

- **Why:** after T207 and T208 the Tauri app and its scripts are gone, but code comments and
  developer documents still describe it as present ("beside the Tauri app", "the Tauri app still
  uses them", the UI tests with a mocked Tauri backend, `tools\package-dist.ps1`). Last task of
  the chain T207 → T208 → T209 (owner's decision 2026-11-19 to retire the Tauri app; see T207).
  Only texts change; no code behaviour.
- **Crates:** `pw-loupe`, `pw-metadata`, `pw-tiff`, `pw-libraw` (comments only). **Files:**
  - `crates/pw-loupe/src/main.rs`
  - `crates/pw-metadata/src/edit.rs`
  - `crates/pw-tiff/src/lib.rs`
  - `crates/pw-tiff/src/resize.rs`
  - `crates/pw-libraw/src/lib.rs`
  - `README.md`
  - `docs/building.md`
  - `docs/TESTING.md`
  - `docs/libraw-build.md`
- **What:**
  1. `pw-loupe/src/main.rs`, line 1: "the egui program beside the Tauri app" → "the egui program".
  2. `pw-metadata/src/edit.rs` (about lines 71–72): "The old `write_rating`, `write_iptc` and
     `write_tags` are unchanged; the Tauri app still uses them (owner decision D7)." → they are
     unchanged (owner decision D7); since the Tauri app was retired (T207) only `tests/sidecar.rs`
     calls them, and whether they stay is an open owner question.
  3. `pw-tiff/src/lib.rs`:
     - line 1: "shared by RawStride and the Tauri app" → for RawStride;
     - about line 7: "`None` keeps the Tauri app's unlimited decoding" → "`None` decodes without a
       limit";
     - line 3, "moved here from `src-tauri` (…)", is history and stays.
  4. `pw-tiff/src/resize.rs`, lines 1–3: keep "Moved from `src-tauri/src/imaging.rs` (ADR 0015
     D3)"; replace "so the TIFF thumbnails of RawStride and the Tauri app's TIFF and RAW views share
     one box filter" with words that do not say the Tauri app uses it today (for example "where it
     was shared with the retired Tauri app's views").
  5. `pw-libraw/src/lib.rs` (about line 11): "(ADR 0006, the Tauri app)" → "(ADR 0006; used by
     `pw-rawview`)". Check with a search that `render_full` / `render_region` are still called
     from `pw-rawview`; if not, say so in the report and write only "(ADR 0006)".
  6. `README.md`:
     - the status note (lines 8–9): remove the sentence "An older Tauri app is still in the
       repository until the switch ([GUI_REQUIREMENTS](docs/GUI_REQUIREMENTS.md) GR8).";
     - **Layout**: remove the `src-tauri/` and `ui/` lines.
  7. `docs/building.md` (lines 21–22): remove the paragraph about the old Tauri app.
  8. `docs/TESTING.md`:
     - §4.3 "UI Regression Tests" (about lines 82–88): replace the section's text with one
       sentence: the program's interface has no automated UI tests; its behaviour is tested in the
       crates (text modules, state and workers) and checked by hand by the owner (ADR 0017 §7);
     - §5, the "UI Regression" profile (about lines 144–148, Playwright and the mocked backend):
       remove it.
  9. `docs/libraw-build.md` (about line 82): the paragraph on `tools\package-dist.ps1` describes
     a script T208 removed. Keep what it says about which DLLs the program needs and where they
     come from; replace the sentences about the script with one saying that a distribution folder
     for RawStride is still to be made (ADR 0002 §4).
- **Tests to add:** none; comments and documents only.
- **Not in this task:** the ADRs, `docs/GUI_REQUIREMENTS.md`, `docs/architecture.md`,
  `docs/distribution.md` and `docs/tasks/` (the reviewer's); done tasks and analyses (history);
  `prototypes/` (history, T207); the test name `done_speed_is_rounded_like_the_tauri_dialog`.
- **Rules:** no code changes, only comments and text. No other file changes; if one must, stop and
  explain.
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the files above; and
  `git grep -nI -e src-tauri -e "ui/web" -e "ui/tests" -e "Tauri app" -- crates tools README.md
  Cargo.toml .gitignore docs/building.md docs/TESTING.md docs/libraw-build.md` shows only history
  ("moved from `src-tauri`", "the retired Tauri app"), the kept test name, and these lines that
  stay as notes of where a behaviour came from: `pw-loupe/src/import_text.rs` (about lines 242 and
  355, "as in the Tauri app", "as `toFixed(0)` in the Tauri dialog"), `pw-tiff/src/convert.rs`
  line 2 and `resize.rs` line 121 ("from `src-tauri/src/imaging.rs`"), and
  `pw-rawview/src/lib.rs` line 2 ("Independent of Tauri").

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added: split from T207 by its cold read, which also found the comments in `main.rs`, `edit.rs`, `pw-tiff`, `pw-libraw`, `TESTING.md` §5 and `libraw-build.md`. Nine files, all text. Waits for T208 |
