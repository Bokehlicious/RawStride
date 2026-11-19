# T207. Retire the Tauri app: remove `src-tauri` and `ui/` from the workspace — Done

- **Why:** the egui program (`pw-loupe`, "RawStride") has every feature of the old Tauri app
  (`docs/GUI_REQUIREMENTS.md` GR8: "No feature of the Tauri app is left"), and the Tauri app has
  not been used for a long time. Owner's decision (2026-11-19): the Tauri app is retired now,
  without waiting for the rest of the S6 acceptance measurement (ADR 0009); S6 measures the egui
  program only and is not affected. Removing it shortens the build (about 285 packages that only
  the Tauri app uses leave `Cargo.lock`), removes code nobody runs, and prepares the repository for
  being made public. First task of the chain T207 → T208 (the scripts in `tools/`) → T209 (texts
  that still name the Tauri app).
- **Crates:** the workspace (`photoworkflow`, the Tauri app's package, is removed), `pw-loupe`,
  `pw-viewcore`. **Files:**
  - removed whole: `src-tauri/` (16 files), `ui/` (14 files)
  - changed: `Cargo.toml`, `Cargo.lock`, `.gitignore`, `crates/pw-loupe/build.rs`,
    `crates/pw-loupe/src/lib.rs`, `crates/pw-viewcore/src/state.rs`, `docs/test-inventory.md`
- **What:**
  1. Remove `src-tauri/` and `ui/` with `git rm -r`. This deletes the Tauri app's two unit tests
     in `src-tauri/src/ingest.rs` and the Node.js tests in `ui/tests/`; that is part of this task
     and is not a weakened test (`docs/tasks/README.md` rule 5), because the code they test is
     removed with them. Nothing else in the workspace depends on the `photoworkflow` package or
     reads a file under `ui/` (checked by the cold read: `cargo tree -i photoworkflow` shows only
     itself, and no `include_*!` points into `ui/`).
  2. `Cargo.toml`: remove `"src-tauri"` from `[workspace] members`. Nothing else in the file
     changes. Let Cargo update `Cargo.lock` (`cargo build --workspace`); do not edit it by hand and
     do not run `cargo update`. Expected diff of the lock file: whole package entries removed,
     dependency lines of kept packages removed where an optional dependency is no longer turned on
     (for example `rfd` loses `glib-sys`, `gobject-sys`, `gtk-sys`), and version suffixes such as
     `"block2 0.6.2"` shortened where only one version is left. **No kept package changes its
     version or checksum**; if one does, stop and report it.
  3. `.gitignore`: remove the `# Tauri` block (`/src-tauri/target/`, `/src-tauri/gen/`) and the
     `# Frontend` block (`node_modules/`, `/ui/dist/`, `/ui/.vite/`). The `/dist/` line stays
     until T208.
  4. `crates/pw-loupe/build.rs`, `rfd`'s features. Today `common-controls-v6` (and `gtk3`) are
     turned on only by `tauri-plugin-dialog` → `photoworkflow`; `pw-loupe` takes `rfd` with
     `default-features = false` (`crates/pw-loupe/Cargo.toml`, line 30). After the removal the
     feature is therefore expected to be **off**. Check with
     `cargo tree --offline -e features -i rfd` and paste the result in the report.
     - Keep the manifest and the code of `build.rs` as they are (asking for Common Controls
       version 6 is harmless without the feature). Rewrite only the module comment: the manifest
       asks Windows for Common Controls version 6; it was needed while the Tauri app's
       `tauri-plugin-dialog` built `rfd` with `common-controls-v6` (T47), and is kept so the
       program's dialogs look the same.
     - If the feature is still on, name in the comment the package that turns it on instead, and
       say so in the report.
  5. `crates/pw-viewcore/src/state.rs` (about line 55): the doc comment of the zoom limits refers
     to "the current viewer (`ui/web/app.js`, `zoomTo`)". Change it to say the limits were taken
     from the retired Tauri viewer's `zoomTo`, without a path. The constants do not change.
  6. `crates/pw-loupe/src/lib.rs` (about line 766): the doc comment of a test names "The `app.js`
     case … (`ui/tests/filter-select.test.js`)". Change it to say the case was taken from the
     retired Tauri app's tests, without a path. The test itself does not change.
  7. `docs/test-inventory.md` (change it by script, `CLAUDE.md` §4):
     - remove the `photoworkflow` row (line 13, `src-tauri/src/ingest.rs`, 2 tests);
     - update the totals in the text of line 4 (today "1984 passed, 76 ignored") from the new
       `test-output.txt`;
     - line 21 (`pw-loupe`, "the `app.js` case") and line 25 (`pw-tiff`, "moved from
       `photoworkflow`") are history and stay.
- **Tests to add:** none; this task removes code. `.\tools\check.ps1` must pass with the new
  totals.
- **Not in this task:**
  - The scripts in `tools/` that build or run the Tauri app (T208), and the comments and documents
    that still name it (T209).
  - `prototypes/` stays as history: `prototypes/phase1/README.md` names `src-tauri`, and five
    prototype `tauri.conf.json` files point at `../../../src-tauri/icons/icon.ico`. The prototypes
    are not workspace members and are not built; their icon path breaks, which is accepted.
  - `crates/pw-rawview` stays in the workspace untouched, and so do `pw-metadata`'s direct
    `ExifTool::write_rating` and `ExifTool::write_iptc`; both are decided separately (backlog item
    "`pw-rawview` and the direct ExifTool write calls after T207").
  - The test name `done_speed_is_rounded_like_the_tauri_dialog` in `pw-loupe/src/import_text.rs`
    stays (rule 5).
  - The ADRs, `docs/GUI_REQUIREMENTS.md`, `docs/architecture.md`, `docs/distribution.md` and
    `docs/tasks/`: the reviewer updates them (GR8 and ADR 0009 §7 get the owner's decision before
    this task is Done; `AGENTS.md` §6). Done tasks and analyses are history and are not changed.
- **Rules:** remove with `git rm`, never by editing files to empty. No change to any
  `[dependencies]` section. No other file changes; if one must, stop and explain (rule 2).
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the files above; and
  the owner has, with a release build from `cargo run --release -p pw-loupe`:
  - started RawStride with ExifTool neither on `PATH` nor as `exiftool.exe` in `target\release`
    (it is looked for there first, `pw-metadata/src/exiftool.rs`), and seen the "ExifTool is
    missing" warning box (`check_exiftool` in `main.rs`, an `rfd::MessageDialog`);
  - opened a folder with **Open Folder…**, so `rfd::FileDialog` is seen to work too (point 4).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added after the owner's decision to retire the Tauri app. Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README and this file only): the first version also removed the scripts and changed the documents (9 changed files, 2 folders and 16 scripts), too big; split into T207 → T208 → T209. Also changed after the cold read: `rfd`'s feature is expected off after the removal (only `tauri-plugin-dialog` turns it on), and the hand check now covers the file dialog and `exiftool.exe` in `target\release`; the `Cargo.lock` diff described as it will be (about 285 packages, not 17; dependency lines and version suffixes of kept packages); `pw-loupe/src/lib.rs` (a comment names `ui/tests/`) added; `prototypes/` named as history; the totals are on line 4 of `docs/test-inventory.md`. Ready for Open once the owner agrees to the chain |
| 2026-11-19 | Open | The owner agreed to the chain T207 → T208 → T209. No chain review: the three tasks remove and reword in order, with nothing running across them in time |
| 2026-11-19 | Ready for review | `src-tauri/` and `ui/` removed with `git rm -r` (30 files); `"src-tauri"` out of `members`; `Cargo.lock` updated by `cargo build --workspace`: 635 → 364 packages, no kept package changed its version, checksum or source, none got a new dependency (checked by script); `.gitignore` blocks removed; `rfd`'s `common-controls-v6` is off (`cargo tree` shows only `pw-loupe`), `build.rs` comment rewritten; comments in `state.rs` and `lib.rs` reworded; `docs/test-inventory.md`: `photoworkflow` row removed, totals 1982 passed, 73 ignored. Not foreseen: the Tauri app also turned on `pw-rawview`'s `libraw` feature, so `pw-rawview/src/libraw.rs` and the 3 ignored tests in `tests/libraw_engine.rs` are no longer built, tested or clippy-checked (76 → 73 ignored); owner: row 45 of the inventory stays, the gap is for the backlog item on `pw-rawview` after T207. The leftover generated `src-tauri/gen/` (untracked once its `.gitignore` line went) deleted from disk with the owner's yes. `.\tools\check.ps1` all OK |
| 2026-11-19 | Done | Review: the diff holds only the brief's files; `Cargo.lock` checked by script (635 → 364 packages, no `tauri*` or `photoworkflow` left, no kept package with a changed version, checksum or source or a new dependency); `cargo tree -e features -i rfd` shows only `pw-loupe`, so `common-controls-v6` is off and the `build.rs` comment is right; no `include_*!` or path into `ui/` or `src-tauri/` left in code (the `pw-tiff` "moved from `src-tauri`" comments are history, T209). Inventory row of `pw-rawview/tests/libraw_engine.rs` (3 ignored) kept as the owner said, though those tests are no longer built; the backlog item on `pw-rawview` after T207 covers it. Owner's decision written into GR8 and ADR 0009 (§7 and the status lines). `.\tools\check.ps1` run by the reviewer: all OK, 1982 passed, 73 ignored, 0 failed, no clippy warnings. The owner's hand check (ExifTool missing box, Open Folder…) follows |
| 2026-11-19 | Done | Hand-checked by the owner with a release build (`cargo build --release -p pw-loupe`, ExifTool removed from `PATH` in that shell, none in `target\release`): the "ExifTool is missing" warning box (`rfd::MessageDialog`) showed at start, and **Open Folder…** (`rfd::FileDialog`) opened a folder. Both dialogs work without `common-controls-v6`. Approved |
