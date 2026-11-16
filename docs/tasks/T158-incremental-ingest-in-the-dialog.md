# T158. Incremental ingest in the import dialog (pw-loupe) — Open

- **Why:** the user part of "copy new images only". **Owner, 2026-11-16** (ADR 0005 choice 6): a
  check box in the import dialog, on by default and remembered in the settings. Files an earlier
  import took from the card (T157's register) are left out before the plan, and the preview and the
  summary count them. A card that cannot be marked is said in the summary, and the import runs anyway.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`,
  `crates/pw-loupe/src/import_worker.rs`, `crates/pw-loupe/src/import_text.rs`,
  `crates/pw-loupe/src/import_dialog.rs` (a test helper only), `crates/pw-loupe/src/settings.rs`,
  `crates/pw-loupe/src/main.rs`, `docs/glossary.md`, `docs/test-inventory.md`.
- **What:**
  1. **The form:** `ImportForm` gets `pub incremental: bool`. `ImportForm::default()` has `true`.
  2. **The setting:** `ShellSettings` gets `import_incremental: bool` (default `true`), read and
     written as `import_incremental` in `settings.txt` like `compare_locked`; a missing key gives
     `true`. In `main.rs`:
     - the first form is `ImportForm { incremental: settings.shell.import_incremental, ..ImportForm::default() }`
       where `last_form` is made today (`Settings` is `Copy`);
     - `close_import` writes the form's value to `self.settings.shell.import_incremental`;
     - `on_exit`, before it saves the settings, does the same from an open import dialog's form,
       so that a change made just before quitting is not lost.

     The value is then saved whenever the settings are saved today (at exit, and on the other
     saves that keep `..self.settings.shell`).
  3. **The check box** in the dialog, right after the "Eject the card …" check box and drawn the
     same way (`ui.checkbox(&mut form.incremental, import_text(ImportText::Incremental))`). A change
     already starts a new preview, because the dialog compares the whole form.
  4. **One planning step** in `import_worker.rs`, used by both `preview` and `run_import`:
     `fn plan_new(shots: &[pw_ingest::Shot], options: &pw_ingest::Options, incremental: bool, root: Option<PathBuf>) -> Planned`
     with `struct Planned { shots: Vec<pw_ingest::Shot>, plan: pw_ingest::Plan, earlier: usize, note: Option<String>, register: Option<(PathBuf, pw_ingest::Register)> }`:
     - box off: all shots, `earlier` 0, no note, no register (today's behaviour);
     - box on, no root: all shots, the note `ImportText::NoRegisterRoot`, no register;
     - box on with a root: `read_register(root)`, `new_shots`, the plan of the new shots, and the
       note `register.problem()` (T157: the foreign or unreadable text, read without opening the
       file for writing).
  5. **Where the root comes from:** `preview` and `run_import` take a
     `register_root: &dyn Fn(&Path) -> Option<PathBuf>` (for `run_import` as a field of
     `ImportRun`). The worker's loop passes `&pw_ingest::register_root`. Tests pass a closure that
     gives their temporary source folder, so no test reads or writes a file in a drive's root.
     The source given to it is the validated source, not a resolved `\\?\` path.
  6. **The preview:** `Preview::new(shots, plan, earlier, note)` with the new shots. `Preview` gets
     `pub earlier: usize` and `pub register_note: Option<String>`. `preview_summary` checks
     `shots == 0 && earlier > 0` first and then gives `ImportText::AllImportedEarlier` instead of
     "No images were found in the source.".
  7. **The import** (`run_import`), on the fresh scan it already makes: `plan_new`; with a register,
     `RegisterWriter::open(root, &register)`. Its `Err` goes to `ImportDone::register_error`, and
     the copy goes on without a writer. Then `pw_ingest::execute_recording` with the writer, or with
     `None`. `ImportDone` gets `pub earlier: usize` and `pub register_error: Option<String>`;
     `report.register_error` goes there too when set. With the box on and no root, `ImportDone` says
     nothing more (the preview already showed the note). With the box off, it is exactly today's
     path (`execute` result and fields unchanged).
  8. **Texts** (rule 13, in `import_text.rs`; the terms of the glossary, "image" and "file", not
     "photo"):
     - `ImportText::Incremental`: "Copy new images only (skip what was imported from this card before)";
     - `ImportText::NoRegisterRoot`: "The source is not a drive, so earlier imports cannot be recognised; every image is copied.";
     - `ImportText::AllImportedEarlier`: "Every image in the source was imported earlier; there is nothing new to copy.";
     - in `preview_summary`, one more part after "already exist(s)", in the same way as the parts
       there today: "1 file imported earlier" / "{n} files imported earlier";
     - in `done_lines`, one more line right after the summary line when `earlier > 0`, made by a new
       `pub fn earlier_done_text(n: usize) -> String`: "1 file imported earlier was left out." /
       "{n} files imported earlier were left out.";
     - `register_error` (if any) as its own line with `error: true`, right before the eject line;
     - the preview's `register_note` drawn as a plain label right under `preview_summary`'s label.

     `preview_summary` and the summary line join parts today, which rule 13 does not want for new
     code. The new part follows the existing pattern on purpose (reviewer, 2026-11-16); moving those
     two texts to whole-sentence templates is not part of this task.
  9. **Glossary:** add "Kopiera bara nya bilder / importerade tidigare" → "Copy new images only /
     imported earlier", and name it in the report.
- **Existing tests that must change** (allowed for this task only, README rule 5; nothing is
  weakened):
  - `import.rs` `form_defaults_match_the_ingest_templates`: the full `ImportForm` literal gets
    `incremental: true`;
  - `import_dialog.rs` test helper `preview()` (about line 336) and `import_text.rs`
    `empty_preview`: the full `Preview` literals get `earlier: 0, register_note: None`;
  - `settings.rs` `saved_file_has_the_comment_line_and_every_key` gets the new key line, and
    `changed_shell()` gets the new field (with `false`, so the "changed" test covers it);
  - `import_worker.rs`: `TempDirs::form` gets `incremental: false`, so every existing worker test
    runs exactly as today; the two direct `run_import` calls get the new `ImportRun` field.
- **Tests to add:**
  - `settings.rs`: `import_incremental` read and written, and `true` when the key is missing.
  - `import.rs`: `ImportForm::default().incremental` is `true`; `Preview::new` passes `earlier` and
    `register_note` through.
  - `import_text.rs`: every new text exactly as above, both counts of the plural texts,
    `preview_summary` with `shots == 0 && earlier > 0`, and `done_lines` with `earlier` and with
    `register_error` (its place and `error: true`).
  - `import_worker.rs`, calling `preview` and `run_import` directly with a root closure that gives
    the temporary source folder:
    - a first import with the box on copies everything and writes the register; a second one copies
      nothing and reports `earlier` equal to the number of files;
    - with the box off, the second import behaves as today (`present` counts, nothing new copied,
      `earlier` 0);
    - a preview with the box on and a root closure that gives `None` has the `NoRegisterRoot` note;
    - with a foreign register file in the root, the preview has the foreign note and the import
      copies everything with `register_error` set.
- **Not in this task:** the Tauri app; changing T157's engine; a register on the computer (ADR 0005
  choice 6 says none).
- **Rules:** `docs/tasks/README.md` rule 13; AGENTS.md §3.
- **Hand check (owner):** import a card. Import it again with the box on: nothing is copied, and the
  summary says how many were left out. Take a few new images and import again: only they are
  copied. Turn the box off: today's destination check is used. Restart the program, also once
  straight from an open import dialog: the box keeps its last state.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (besides files of other tasks still uncommitted in the tree).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Blocked | Added with T157 (bug 3 of 5); the owner's decision is ADR 0005 choice 6. Waits for T157 |
| 2026-11-16 | Blocked | Cold read: `plan_new` now returns the new shots and the register; the root comes in as a closure, so the worker tests never touch `C:\`; existing worker tests run with the box off; the foreign note uses T157's `Register::problem` instead of opening a writer; `on_exit` keeps the value; the existing tests that must change are named; `import_dialog.rs` added for its `Preview` literal; "image"/"file" instead of "photo"; the joined summary parts follow the existing pattern on purpose |
| 2026-11-16 | Open | T157 is Done. From its review: a source without a drive letter, also a `\\?\` path, has no register; point 5 (the validated source, not a resolved path) and the note `NoRegisterRoot` with its test already cover it, so the text is unchanged. T162 (the daylight saving time shift on FAT32) changes only `pw-ingest` and may run beside this task |
