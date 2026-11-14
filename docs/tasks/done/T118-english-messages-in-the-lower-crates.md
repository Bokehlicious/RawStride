# T118. English messages in the lower crates — Done

- **Starts after T117 is Done** (both change the expected texts of the same `pw-loupe` tests).
- **Why:** English is the interface's base language (`docs/GUI_REQUIREMENTS.md` GR14, owner
  2026-11-13). Some messages that reach the user are still made in Swedish below `pw-loupe`, and
  the crates' error texts follow no single style.
- **Crates:** `pw-ingest`, `pw-thumbcache`, `pw-viewcore`, `pw-metadata`; `pw-loupe` for point 4 and
  the tests that check these messages. **Files:** `crates/pw-ingest/src/eject.rs`, `paths.rs`,
  `request.rs`; `crates/pw-thumbcache/src/extract.rs`; `crates/pw-viewcore/src/native.rs` (and the
  file that defines `DecodeError`, if it is another); `crates/pw-metadata/src/worker.rs`;
  `crates/pw-loupe/src/lib.rs`, `loupe_text.rs`, `metadata.rs`, `editor.rs`; the tests (only the
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
  4. **No logic on an error text** (found in T116): two places in `pw-loupe` decide what to show
     by comparing another crate's English error text. Replace each with a typed kind in that
     crate, and match on the kind in `pw-loupe`:
     - `pw-viewcore`'s `DecodeError` for a file without an embedded JPEG (`"no embedded JPEG"`,
       compared in `pw-loupe` `lib.rs` `error_text`), for example a `no_embedded_jpeg` flag like
       today's `damaged`;
     - `pw-metadata`'s `WriteError::Other` with `NO_JOURNAL` and `EXIFTOOL_TOO_OLD` (`worker.rs`,
       compared in `pw-loupe` `metadata.rs` `write_error_text`), for example two new
       `WriteError` variants.
     The texts the user sees do not change. If a change outside these files is needed (another
     caller matches on `WriteError`), stop and ask.
- **Tests:** changing an expected text from Swedish to the English of this task is allowed;
  nothing else in a test changes. Add tests for point 4: the new kinds are set where the old texts
  were made, and `pw-loupe` gives the same texts as before for them. Add a test per crate that its
  non-test code has no Swedish letters in a string literal (as T117's test).
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
| 2026-11-13 | Open | Reviewer: point 4 added (the two comparisons of error texts that T116 found) |
| 2026-11-14 | Ready for review | Agent: done. Owner approved three files outside the list: `pw-metadata/src/lib.rs` (the two `WriteError` variants), `pw-loupe/src/preview.rs` (`no_embedded_jpeg: false` in a test's struct literal), `docs/glossary.md` (five terms). check.ps1 OK: 1218 passed, 0 failed, 43 ignored |
| 2026-11-14 | Ready for review | Reviewer: accepted, not committed. Every Swedish message of the listed files is English with the glossary's terms; shown-as-is messages are whole sentences, `Display` texts lower case without a full stop; `DecodeError::no_embedded_jpeg` and `WriteError::NoJournal` / `ExifToolTooOld` replace the two text comparisons, the user's texts unchanged; the three files outside the list were approved by the owner. Notes, not blocking: the Swedish-letter check is copied into four crates (about 200 lines each); two `pw-loupe` test names still say "swedish_text". The reviewer's `check.ps1`: fmt, test and clippy OK, 1218 passed, 0 failed, 43 ignored. Waits for the owner's approval |
| 2026-11-14 | Done | Owner: hand check done and approved; committed by the reviewer |
