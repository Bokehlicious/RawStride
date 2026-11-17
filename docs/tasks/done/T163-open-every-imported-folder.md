# T163. After an import, every folder that got images opens in its own tab (pw-loupe) — Done

- **Why:** the owner's hand check of T158 (2026-11-16): an import with the folder template
  `{year}/{date}` put 39 images in `2026-11-15` and 2 new ones in `2026-11-16`, and only the folder
  with the most files opened, so the new images seemed missing. **Owner, 2026-11-16** (ADR 0010
  "Open the result", ADR 0005 "Dialog"): when an import copies into two or more folders, each of
  them opens in its own tab.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`,
  `crates/pw-loupe/src/import_worker.rs`, `crates/pw-loupe/src/import_dialog.rs`,
  `crates/pw-loupe/src/lib.rs` (the export only), `crates/pw-loupe/src/main.rs`,
  `docs/test-inventory.md`.
- **What:**
  1. **The folders:** `ImportDone` gets `pub new_folders: Vec<PathBuf>`: the destination folders
     (`options.dest.join(item.rel).parent()`) of the plan items with `to_dest` whose file exists
     at the destination after the copy, each folder once, in the order the plan first reaches it
     (the plan is in capture order, so the oldest images' folder comes first). An item with
     `to_dest` was free before the copy, so a file there afterwards was copied by this import. An
     item copied only to the backup, or already present, adds no folder. (An item copied to the
     destination whose backup copy then failed adds its folder too; its file is there.) This is a
     function of its own in `import_worker.rs`,
     `fn new_folders(plan: &pw_ingest::Plan, dest: &Path) -> Vec<PathBuf>` (`Path::parent` handled
     without `unwrap`), which `run_import` calls next to `folders`; `folders` and the summary texts
     are unchanged.
  2. **What opens:** `result_folder` in `import.rs` is replaced by
     `pub fn result_folders(copied: usize, new_folders: &[PathBuf]) -> Vec<PathBuf>`: empty when
     `copied == 0` (ADR 0010 Q2: no tab when nothing was copied), else `new_folders` as they are.
     `ImportDialog::result_folder` becomes `result_folders() -> Vec<PathBuf>` the same way, and the
     export in `lib.rs` follows.
  3. **The tabs** (`main.rs`, where `dialog.result_folder()` is used today): every folder of
     `result_folders()` is opened with `self.open(&folder, true)` in that order, so the newest
     folder's tab is the active one at the end. A folder already shown in a tab gets no second tab:
     `Tabs::open_new` reuses that tab and opens the folder in it afresh, as Ctrl+O does (T119).
     Each `open` also records the folder in Recent and saves the library, as any opened folder
     does; that is accepted. `import_result_opened` keeps doing what it does (the folders open once
     per import).
- **Existing tests that must change** (allowed for this task only, README rule 5; nothing is
  weakened):
  - `import.rs` `result_folder_is_none_without_copied_files_and_uses_first_folder_otherwise`
    becomes a test of `result_folders` (empty with `copied == 0`, the given folders in their order
    otherwise, empty for no folders);
  - `import_dialog.rs` (about line 570) checks `result_folders()` instead of `result_folder()`; the
    `ImportDone` literal there gets `new_folders`;
  - `import_dialog.rs` (about line 902, `result_folder_is_none…`) is renamed for `result_folders`
    and checks that `result_folders()` is empty after a `Done` with `copied == 0` and a non-empty
    `new_folders`.
- **Tests to add** (`import_worker.rs`):
  - with `run_import` called directly as the existing worker tests do, and a helper that writes a
    source image and sets its modification time with `File::set_modified` (the fake images have no
    Exif, so the folder template `{date}` takes the file's date, in UTC):
    - two images on one date and one image on another date: `new_folders` holds the two date
      folders, oldest first, each once;
    - a second import into the same destination with the box off, after one new image on a new
      date was added: `new_folders` holds only the new image's folder, not the folders whose
      images were already present;
  - `new_folders` on a hand-built `Plan` in a temporary destination (this stands for a cancelled
    or failed run): items with `to_dest` whose files were created give their folders in the plan's
    order, each once; an item with `to_dest` whose file is missing, and items with `to_dest` false
    (present or backup only) whose files exist, give none.
- **Not in this task:** the summary's folder lines; the Tauri app; how many tabs may be open (no
  limit today).
- **Rules:** AGENTS.md §3; `docs/tasks/README.md` rule 13 (no new user text is expected; if one is
  needed, stop and ask).
- **Hand check (owner):** import a card whose images span two or more dates with `{year}/{date}`:
  each date folder opens in its own tab and the newest is shown. Import again with one new image:
  only its folder opens.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (besides files of other tasks still uncommitted in the tree).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Open | Added from the owner's hand check of T158; the owner's decision is in ADR 0010 "Open the result" and ADR 0005 "Dialog" |
| 2026-11-16 | Open | Cold read: `new_folders` is its own function, so the cancel case is tested on a hand-built plan (a `run_import` test cannot set the cancel flag from its progress); a destination copy with a failed backup adds its folder; `Path::parent` without `unwrap`; the dialog test belongs in `import_dialog.rs` and replaces the renamed test there; reusing a tab reopens the folder in it, and each `open` records Recent and saves the library (accepted); `{date}` takes the file's date in UTC |
| 2026-11-17 | Ready for review | `new_folders` in `import_worker.rs`, `result_folders` in `import.rs` and the dialog, every folder opened in `main.rs`; three tests added, three changed as the task allows; `check.ps1` all OK (1690 passed, 65 ignored) |
| 2026-11-17 | Done | Reviewed: no bugs (a failed copy leaves no file at the destination, since pw-ingest writes a `.part` file and removes it, so `exists()` is right). Reviewer's `.\tools\check.ps1` on the tree with T160, T161, T163 and T164: all OK, 1694 passed, 65 ignored, 0 failed; clippy without warnings; `import_dialog.rs`, `lib.rs` and `main.rs` split from T161 for this commit, the inventory's header set to its count (1690). The owner's hand check is done after the commit (owner, 2026-11-17). No leftovers |
| 2026-11-17 | Done | The owner's hand check approved |
