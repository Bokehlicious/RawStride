# T118. English messages in the lower crates — Open

- **Starts after T117 is Done** (both change the expected texts of the same `pw-loupe` tests).
- **Why:** English is the interface's base language (`docs/GUI_REQUIREMENTS.md` GR14, owner
  2026-11-13). Some messages that reach the user are still made in Swedish below `pw-loupe`, and
  the crates' error texts follow no single style.
- **Crates:** `pw-ingest`, `pw-thumbcache`, `pw-viewcore`; the tests in `pw-loupe` that check these
  messages. **Files:** `crates/pw-ingest/src/eject.rs`, `paths.rs`, `request.rs`;
  `crates/pw-thumbcache/src/extract.rs`; `crates/pw-viewcore/src/native.rs`; the tests (only the
  expected texts) in `crates/pw-loupe/src/import.rs`, `import_dialog.rs`, `import_text.rs`,
  `import_worker.rs`; `docs/test-inventory.md`.
- **What:**
  1. **Translate** every Swedish message of these files to English with the terms of
     `docs/glossary.md` (today: the eject results in `eject.rs`, the path checks in `paths.rs`,
     the form checks in `request.rs`, `ThumbError::NoPreview` in `extract.rs`, the TIFF errors in
     `native.rs`). Before you start, search the non-test code of every crate except `pw-loupe`
     for string literals with `å`, `ä`, `ö`, `Å`, `Ä` or `Ö`; if you find one outside these files,
     stop and report it.
  2. **One style for the two kinds of text:**
     - a message the UI shows **as it is**, as its own sentence (the form checks of
       `request.rs`, the eject results of `eject.rs`): a whole English sentence, capital letter,
       full stop ("Choose a source.", "The destination must not be on or above the source.");
     - an **error's `Display` text** that the UI puts into its own sentence (`"…: {error}"`):
       lower case, no full stop (the Rust API guidelines), as `pw-metadata`'s errors already are
       ("the TIFF variant is not supported yet (…)").
     Which kind each message is follows from how `pw-loupe` uses it today; if it is unclear, stop
     and ask.
  3. The words "Källan", "Målet", "Backupmålet" passed as `what` in `request.rs` become "The
     source", "The destination", "The backup destination", and each message stays one sentence
     built from one template.
- **Tests:** changing an expected text from Swedish to the English of this task is allowed;
  nothing else in a test changes. Add a test per crate that its non-test code has no Swedish
  letters in a string literal (as T117's test).
- **Not in this task:** the Tauri app (`src-tauri`, `ui/web`), which keeps its Swedish texts until
  it is retired (GR8, ADR 0007 D7). Its import now shows `pw-ingest`'s messages in English; that
  is accepted. Messages that are already English stay as they are unless point 2 says otherwise
  for a file this task changes.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-13 | Open | Added; starts after T117 |
