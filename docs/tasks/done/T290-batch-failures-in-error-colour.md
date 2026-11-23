# T290. The failed images of "Apply to {n} Marked" shown in the error colour — Done

- **Why:** found in the owner's hand check of ADR 0017 U13b (2026-11-23). After "Apply to {n}
  Marked" the metadata window shows the batch's summary under the form: the written count, one
  line per image that failed ("Could not save metadata for <file>: …"), the notices of the other
  images and the window's image's notices. Every line is drawn in the normal text colour (white in
  the dark theme), so a failure is easy to miss, while the failure of a single Save is drawn in the
  theme's error colour. The owner asked for the failed images in the error colour (2026-11-23).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/metadata.rs`,
  `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`.
- **Background (read before starting):** `metadata.rs`: `BatchSummary` (`written`, `total`,
  `failed`, `kept`, `notices`) and `BatchSummary::lines` (the written count, then `failed`, then
  `kept`, then the window's notices through `notice_text`), and its tests (search for
  `summary.lines()`). `main.rs`: `EditorStatus` (`Batch(Vec<String>)`), `MetadataWindow::apply`
  (`EditorEvent::BatchDone` sets `EditorStatus::Batch(summary.lines())`; `EditorEvent::Reloaded`
  with an error after a batch pushes the read error as one more line), and the drawing of the
  window's status in the bottom panel of the metadata window (`EditorStatus::Batch(lines)` drawn
  with `ui.label`; `EditorStatus::Failed` with `egui::RichText::new(message).color(error_color)`,
  where `error_color` is `ui.visuals().error_fg_color`).
- **What:**
  1. **A line and whether it is an error** (`metadata.rs`): `pub(crate) struct StatusLine { pub
     text: String, pub error: bool }` (`Debug, Clone, PartialEq, Eq`) and `pub fn
     BatchSummary::status_lines(&self) -> Vec<StatusLine>`: the same lines in the same order as
     `lines`, with `error: true` for each line of `failed` and `false` for the written count, the
     lines of `kept` and the notices. `lines` stays and gives the texts of `status_lines` (its
     tests are not changed). After point 2 `lines` has no caller outside tests, and `metadata`
     is a module of the binary only, so it gets `#[cfg_attr(not(test), allow(dead_code))]` as
     other items of `metadata.rs` have, so the build has no warning. (`import_text.rs` has the
     same shape for the import's done lines, `DoneLine { text, error }`, drawn with
     `ui.colored_label(error_fg_color, …)`; `StatusLine` is a name of its own, not used yet.)
  2. **The status** (`main.rs`): `EditorStatus::Batch` holds `Vec<StatusLine>` (`StatusLine`
     added to `main.rs`'s `use metadata::{…}`). `BatchDone` sets it from `status_lines`; the read
     error after a batch is pushed as a line with `error: true` (it is a failure too). The comment
     of `EditorStatus::Batch`, which names `BatchSummary::lines`, is changed to say this.
     `EditorStatus::Batch` is matched only in `apply` and in the drawing.
  3. **Drawing:** a batch line with `error: true` is drawn as the single failure is
     (`egui::RichText::new(text).color(error_color)`); the other lines as today. No other
     colour changes (the notices and the "kept" lines stay in the normal colour, as after a single
     Save).
- **Tests to add:**
  - `metadata.rs`: `status_lines` of a summary with a written count, one failure, two `kept`
    lines and a window notice gives the texts of `lines` in the same order, with `error` true only
    for the failure;
  - `main.rs`: a `MetadataWindow` (`MetadataWindow::new`) given `EditorEvent::BatchDone` with one
    failure has `EditorStatus::Batch` with that line marked as an error; a following
    `EditorEvent::Reloaded` for the window's image with an error (after the batch, so
    `expect_reload` is set) adds the read error as a line marked as an error. No `main.rs` test
    builds a `MetadataWindow` yet: `MetadataWindow::new(path, marked)` needs only a `PathBuf` and
    a `Vec<PathBuf>`; the test module is a child of `main.rs`, so it reads the private `status`;
    `BatchSummary`'s `notices` can be empty; `Reloaded` takes `Err(String)`, so no read is needed;
    the expected read-error line is `panel_text(&PanelText::ReadError(error))`. The test module's
    `use` lines get what it needs.
  - `docs/test-inventory.md`: both the `pw-loupe` unit row (the `metadata.rs` test) and the
    binary target's row (the `main.rs` test), and the totals.
- **Not in this task:** colours of other texts (the notices, the field errors, the status line of
  the main window); a change of the summary's texts.
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`. The existing tests
  of `BatchSummary::lines` are not changed.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  The owner's hand check: after "Apply to {n} Marked" with a JPEG without DateTimeOriginal and
  Headline `{date}`, the line of the failed image is red (the theme's error colour) and the other
  lines are as before, in the dark and the light theme.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added from the owner's hand check of ADR 0017 U13b (the failed images of a batch were white in the dark theme); waits for the cold read |
| 2026-11-23 | Open | Cold read: `lines` would have no caller outside tests in the binary-only `metadata` module, so it gets `#[cfg_attr(not(test), allow(dead_code))]` (else a warning); `StatusLine` added to `main.rs`'s `use`, and `EditorStatus::Batch`'s comment named; how the `main.rs` test builds the window spelled out (no existing example); `DoneLine` in `import_text.rs` named as the same shape; both test-inventory rows named. No other file needed |
| 2026-11-23 | Ready for review | `metadata.rs`: `StatusLine { text, error }` and `BatchSummary::status_lines` (failed lines `error: true`); `lines` gives its texts, `#[cfg_attr(not(test), allow(dead_code))]`. `main.rs`: `EditorStatus::Batch(Vec<StatusLine>)`, `BatchDone` uses `status_lines`, the read error after a batch pushed with `error: true`, error lines drawn with `error_fg_color`. Tests: 1 in `metadata.rs`, 1 in `main.rs` (both in the binary target). The test inventory has one combined `pw-loupe` row (library and binary), so that row's count went 1323 -> 1325 and its text names both tests; totals 2464 passed, 78 ignored. check.ps1 all OK, no clippy warnings |
| 2026-11-23 | Done | Reviewed: matches the task; the test inventory has one combined `pw-loupe` row for the library and the binary (the task named two), so the agent's single row is right. check.ps1 run again by the reviewer: all OK, 2464 passed, 78 ignored, no clippy warnings. No CHANGELOG line (the variables are not released yet). The owner's hand check follows |
