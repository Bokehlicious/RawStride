# T91. ADR 0007 I9e: RawStride writes ratings and the ingest template safely — Done

- **Can start after T87, T88, T89 and T90 are accepted.** D4, D5 and D6 were approved on 2026-11-12.
- **Crates:** `pw-metadata` (the worker), `pw-loupe`.
- **Files:** `crates/pw-metadata/src/worker.rs`; `crates/pw-metadata/src/lib.rs` (`WriteError` only);
  `crates/pw-loupe/Cargo.toml` (the D4 line only); `Cargo.lock` (by cargo);
  `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/metadata.rs`;
  `crates/pw-loupe/src/import_worker.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §9 for every embedded write RawStride makes, §9.5 (the journal and recovery at
  start; ingest writes only to the verified copy, with this procedure), §15.
- **What:**
  1. **The worker** (`worker.rs`): `pub struct WriteSetup { pub journal: Journal, pub checks:
     Box<dyn ImageCheck + Send>, pub options: EditOptions }`; `MetadataWorker::start(setup)` and
     `start_with(program, pre_args, setup)` take it; a rating write runs
     `write_rating_safely(&path, &rating, &setup.options, &*setup.checks, &setup.journal)`.
     Reading (`read_ratings`) is unchanged. The existing calls of `start_with` in tests
     (`worker.rs`, `pw-loupe/src/metadata.rs`) get a setup with `NoImageCheck` and a temporary
     journal; nothing else in those tests changes.
  2. **`WriteError`** (`lib.rs`) gains `NotSafe { stage: Stage, original: OriginalState, reason:
     String }`; `From<EditError>`: `Unsupported`, `Read`, `Value`, `Plan`, `Template` → `Other`
     with the error text (`Read` through `From<Error>` as today, so `ReadOnly`/`NotWritten` keep
     their kind); `Write` with `Stage::Precheck` while the target exists and is read-only (as
     `refused_write_error` checks) → `ReadOnly`; any other `Write` → `NotSafe`. `Display` in
     English.
  3. **`pw-loupe` start** (`main.rs`): the journal folder is `%LOCALAPPDATA%\RawStride\journal`
     (`Journal::open`). **Before** the workers start, `Journal::recover` runs once; when it
     returns `NeedsUser`, `Failed` or `Unreadable` entries, a message box (as the exit message,
     T44) says "RawStride avslutades mitt i en metadataskrivning. Kontrollera de här filerna:" and
     one line per entry (`<original>` and, when there is one, `(säkerhetskopia: <backup>)`);
     `Cleaned` and `Restored` are not shown. When the folder cannot be opened (no
     `LOCALAPPDATA`, an error), RawStride still starts; every rating write and ingest template
     then fails with "skrivloggen kunde inte öppnas" and writes nothing.
  4. **The image check:** one `pw_imagecheck::RawImageCheck::new` whose closure calls
     `releaser.release(path, 5 s)` (T89's `Releaser` of the loupe's core) for the metadata worker,
     and one for the import worker.
  5. **Texts** (`metadata.rs`, `write_error_text`, after "Kunde inte spara betyget för <fil>: "):
     `NotSafe` with `Intact` → "metadata kunde inte skrivas säkert, filen är oförändrad";
     `AtBackup(backup)` → "metadata kunde inte skrivas säkert, originalet ligger nu som <filnamn
     på backup>"; `Replaced` → "filen skrevs men slutkontrollen misslyckades, den tidigare
     versionen finns kvar som säkerhetskopia". `ReadOnly`, `NotWritten` and `Other` as today.
  6. **Ingest** (`import_worker.rs`): `ImportWorker::start` takes a journal and a
     `Box<dyn ImageCheck + Send>`; `apply_iptc` uses `apply_template_safely(current,
     &iptc.to_template(), &EditOptions::default(), …)` instead of `write_iptc`; a failure is listed
     as today (`"<file>: <error>"`). The existing tests that start the import worker get
     `NoImageCheck` and a temporary journal.
  7. **`Cargo.toml`** of `pw-loupe`: `pw-imagecheck.workspace = true` (D4); `git diff Cargo.lock`
     shows only that line in `pw-loupe`'s list.
- **Rules:** rules 3–7 and 10–12. Nothing in `src-tauri` changes (D7). `write_rating` and
  `write_iptc` stay for the Tauri app.
- **Tests to add:**
  - `worker.rs`: a rating written through the worker with a JPEG and a temporary journal arrives
    as `RatingWritten` with `Ok`, and the journal folder is empty afterwards; a read-only JPEG
    gives `WriteError::ReadOnly`; an `ImageCheck` that fails gives `NotSafe` with `Intact`;
  - `lib.rs`: `From<EditError>` for each case of point 2;
  - `pw-loupe` `metadata.rs`: the three `NotSafe` texts (`write_error_text`);
  - `pw-loupe` `main.rs` (or where the start code is testable): the recovery message for a
    `NeedsUser` entry with and without a backup, and none for `Cleaned`/`Restored`;
  - `pw-loupe` `import_worker.rs`: an import of the JPEG fixture with a creator in the template
    writes it (read back with `read_metadata`) and leaves no `pwtmp` file.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: a rating on
  a DNG shown in RAW mode is saved and the image is still shown; the same on a JPEG and an ARW
  (sidecar).
