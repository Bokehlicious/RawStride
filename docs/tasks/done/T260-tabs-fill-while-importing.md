# T260. ADR 0026 F4: the destination tabs open on the first image and fill while importing (pw-loupe shell) — Done

- **Why:** ADR 0026 §3.4 (F-Q10, F-Q11 and what was settled on 2026-11-21), GR12 as changed:
  each destination folder opens in its own tab when its first image is in place with its
  template, and the tab fills as the import goes on. Today the folders open only at `Done`
  (`poll_import_worker` opens `ImportDialog::result_folders`, T163). This task wires the import's
  events (`Planned`, `Placed`, T258) to the tabs with `ImportTabs` and `Tabs::open_behind` (T259),
  keeps the thumbnails while a tab fills (T257), and ends with the owner's hand check of F4. Last
  task of the chain T256 → T257 → T258 → T259 → T260.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/main.rs` (points 1–6)
  - `crates/pw-loupe/src/import_dialog.rs` (point 7)
  - `crates/pw-loupe/src/import.rs` (point 7)
  - `crates/pw-loupe/src/lib.rs` (point 7, the re-export)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests, when tests are
    added or removed)
  - this task's own file (status and log)
- **What:**
  1. **The import's bookkeeping.** `Loupe` gains `import_tabs: Option<ImportTabs>`, with a doc
     comment (`None` at start and in the test constructor). In `poll_import_worker`, besides what
     each event does today: `Planned` sets it to `ImportTabs::new(images)`; `Placed` is handled
     by point 2; `Done` applies point 5 after the existing lines that clear `ejecting_drive` and
     `import_destination` and after `dialog.apply`, then sets it to `None`; a disconnected worker sets
     it to `None`.
  2. **A `Placed` event** (`Loupe::import_placed`, a new method with a doc comment): nothing when
     `import_tabs` is `None`. Otherwise `ImportTabs::place` gives the images by folder; for each
     group, in order:
     - **The folder has one or more tabs** (`Tabs::tabs_of_folder_mut`, as `relist_tabs` loops
       over them): each such tab's files become `with_placed(that tab's folder, its files, the
       group's images)` through `relist_tab` (no file made current; the chosen order,
       `tab_pending`, `tab_rating` and `modified_time` as `relist_tabs` passes them; the active
       tab with `self.ratings_ready()`, every other tab with `false`, as `relist_tabs` and
       `replace_tab_files` do). When one of them is the active tab, its new files are then
       followed as `follow_disk_changes` follows a listing's new files: the contact sheet's note
       becomes the anchor (before the relist), the new files' ratings are read on their own
       (`add_files`, after `resolve_paths`) and those sent marked `unread`, the tab is
       refiltered, `active_changed(false)` (which keeps the made thumbnails, T257), and the sort
       is followed; an error of those steps is shown in the status line as `follow_disk_changes`
       shows it, but there is **no status text** of what came, and the files are **not** added
       to `added_files` (the import put them in place whole). Factor the shared steps out of
       `follow_disk_changes` into one method both use, without changing what
       `follow_disk_changes` does. A tab that is not active is only relisted; it is read in full
       when it becomes active (`load_tab`), as today.
     - **The folder has no tab, and this is its first group** (`PlacedInFolder::first`): its
       listing is `list_folder(folder)` passed through `ImportTabs::leave_out_waiting` (files of
       earlier imports stay; images this import has not shown yet are left out), then
       `with_placed` with the group's images; when the listing fails, the group's images alone.
       The folder (in the import's form, the parent of the images) is recorded as the first
       recent folder and the library saved, as `open` does. For the import's first shown folder
       (point 3) the tab is opened with `Tabs::open_new` (the folder's files, the first current),
       set to the contact sheet (GR13) and loaded with `load_active_tab`; for a later folder with
       `Tabs::open_behind` (the active tab unchanged, nothing loaded).
     - **The folder has no tab, and it is not its first group:** the user closed its tab during
       the import; nothing is opened again.
  3. **The first shown folder becomes active** (F-Q10): when `ImportTabs::first_kept()` (T259)
     is false, the first group's folder is the import's first shown folder. When it already had
     a tab: if the active tab is one of its tabs, that one stays; otherwise its first tab
     (`Tabs::index_of_folder`) is activated after the relist of point 2 and loaded as a click on
     it does (`load_activated_tab`, which reads it in full); its view is kept as it was. Then
     `ImportTabs::first_shown` keeps the position: the active tab's folder, its current file
     (`files[current]`, `None` when empty) and its view, taken after the load. (A tab with a
     filter whose current image a later refilter moves counts as moved at the end; accepted.)
     The import dialog is not hidden (the owner, 2026-11-21: the tab opens behind it).
  4. **Listings during an import** leave out what the import has not shown: while `import_tabs`
     is `Some`, the listing in `open` (a folder opened from the library, Recent, a drop), in
     `reload_active_tab` (F5) and in `relist_tabs` (after a move or an undo) goes through
     `leave_out_waiting` before it is used. In `open`, when a file was opened, the position is
     found again in the filtered list (the same path; the first file when that file was left
     out). The watch's listing stays held for the destination as today (`disk_listing_held`,
     T251); do not change that.
  5. **At `Done`** (F-Q10's end rule): `ImportTabs::end_target` with the active tab's position
     now (as in point 3; `None` when there is no active tab with a folder); when it names a
     folder that has a tab, its first tab is activated and loaded as a click does
     (`load_activated_tab`). Once nothing holds the listing any more (the import has ended and
     the import dialog is hidden or closed, `disk_listing_held`), a tab whose watch noted a
     change is listed by `follow_disk_changes` as today; that is how the files of a shot the
     import did not show (a cancel, the owner 2026-11-21) appear, without the template. Nothing
     else happens to the tabs at `Done`.
  6. **What goes in `main.rs`:** the opening of `dialog.result_folders()` at `Done`, the field
     `import_result_opened` (and every place that sets it), and what only served them (the
     `was_running` check around `dialog.apply`; `dialog.apply(&event)` itself stays).
  7. **What goes elsewhere:** `ImportDialog::result_folders` (`import_dialog.rs`, and
     `result_folders` in its `use` line) and `import::result_folders` (`import.rs`, and its
     `pub use` in `lib.rs`), now unused. Their tests go with them
     (`result_folders_are_empty_without_copied_files` in `import_dialog.rs`,
     `result_folders_are_empty_without_copied_files_and_every_new_folder_otherwise` in
     `import.rs`), and the `assert_eq!(dialog.result_folders(), …)` in the dialog test that
     applies `Done` (the rest of that test stays). `ImportDone::new_folders` stays (the
     summary's data).
- **Tests to add** (`main.rs`, as pure functions where the logic allows; the shell's wiring is
  checked by hand below):
  - if the agent factors out a pure helper for point 2's choice (tabs exist, the first folder, a
    later folder, a closed tab) or point 4's position in `open`, its cases; otherwise name in
    the report why the wiring has no test of its own (`ImportTabs` and `open_behind` are tested
    in T259, the thumbnails in T257).
- **Hand check (the owner, a release build, with a card or a folder of camera files):**
  1. An import with the IPTC template into an empty destination: after the first shot, the
     destination folder's tab opens behind the dialog as a contact sheet; hide the dialog: the
     images come in capture order, and the thumbnails already shown do not flicker when new ones
     come.
  2. During the import (the dialog hidden), rate an image already shown: the rating stays; after
     the import, the image has both the rating and the template (the metadata window).
  3. An import over two dates (two folders): the first folder's tab is active, the second opens
     behind it; touching nothing, the second folder's tab is active at the end. Again, with the
     dialog hidden and a click on another image during the import: the active tab stays at the
     end.
  4. A destination folder already open in a tab (with a filter or marks): the new images are
     added to that tab, the filter and marks stay.
  5. Cancel during the import: the tabs keep the images shown; once the dialog is hidden or
     closed, images of a shot not finished may appear (the status line says new images),
     without the template.
  6. The Tasks row and the dialog show only the copy's progress, no "Writing IPTC".
  7. F-Q11, measured: the time of an import of the same files with the template on, in 0.2.0
     and in this build (the summary's time); written in the log.
  8. F4a: files copied into an open folder with File Explorer, a sort chosen, a move to
     `_rejected` and its undo: the thumbnails already shown do not flicker.
  9. Sort by capture time during an import: the new images are placed in the order.
- **Not in this task:** the import worker (T258), `ImportTabs` (T259), the thumbnails (T256,
  T257); no new user texts. The CHANGELOG line is the reviewer's, at Done. A tab open on the
  *backup* folder is not held while the import runs, so its watch may list a backup copy before
  its template is written (not new; the reviewer notes it in `known-limits.md` at Done).
- **Rules:** `docs/tasks/README.md` rules 1–13 (no new texts); `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added when F4 was written (ADR 0026 §3.4); waits for T259. At Done: CHANGELOG line, an independent code review |
| 2026-11-21 | Blocked | Cold read: path forms consistent; fixed: every tab of a folder relisted (the active one followed as active, others with `ratings_ready` false), `ImportTabs::first_kept` (added to T259), the first folder's existing tab activated after its relist with its view kept, errors shown but no change text, a tab closed during the import not opened again, `relist_tabs` and `open`'s position in point 4, the hold after `Done` lasting while the dialog is shown (point 5, hand check 5), the dialog hidden for clicks (hand check 2, 3), the `was_running` leftovers |
| 2026-11-21 | Blocked | Chain review: the end rule's place at `Done` named; `test-inventory.md` only when tests change; the backup folder's tab not held (not new, to `known-limits.md` at Done); the other findings (`relist_tabs`, `open`'s position, the hold while the dialog is shown, `first`, the activation order) were already fixed after the cold read |
| 2026-11-21 | Open | T259 done; ready for its agent |
| 2026-11-21 | Ready for review | Points 1–7 done: `import_tabs` on `Loupe`; `import_placed`, `fill_import_tabs`, `open_import_folder`, `import_ended`, `import_position`; `follow_relisted_active` factored out of `follow_disk_changes`; listings in `open`, `reload_active_tab` and `relist_folder_tabs` through `leave_out_waiting`; `import_result_opened` and the `was_running` check removed; `result_folders` and its two tests removed. Pure helpers `placed_tabs`, `leave_out_waiting`, `opened_during_import` with 3 tests; the event wiring has no unit test of its own (`ImportTabs`, `open_behind` tested in T259, thumbnails in T257). check.ps1: fmt, audit, tests (2249 passed, 76 ignored, 0 failed) and clippy OK; personal data FAILED on a user folder in `docs/analyses/2026-11-21-metadata-write-time.md:131` (committed with the analysis, outside this task). Hand check not done |
| 2026-11-21 | Done | Reviewer: points 1–7 as the brief says; the active tab's second relist in `fill_import_tabs`' loop is a no-op (`Tab::relist` returns at once with nothing new or gone). Extra code review (one fresh agent): no confirmed bug; two plausible edge cases accepted into `known-limits.md` ("Import: the template per shot"): a folder opened again during an import can keep a copied image hidden after a cancel or a failed backup copy, and a `subst`/`\\?\` spelling of the destination is not recognised; an anchor set when nothing changed, harmless. The reviewer also fixed `ImportDone::new_folders`' doc comment (`import_worker.rs`, it still said the folders open after the import) and added the backup-folder item to `known-limits.md`, the CHANGELOG line (Changed) and the plan's F4 row. check.ps1 all OK after that. Awaiting the owner's hand check (nine points above) |
| 2026-11-21 | Done | The owner's hand check (release build of 34d2dcd): points 1, 2, 3.1, 4, 5, 6, 8 and 9 passed. Point 3.2 behaved as the brief says (the active tab stayed), but the owner wants the rule changed: a later folder's tab becomes active when it opens, if the user has not moved since the import last made a tab active (F-Q10 changed, ADR 0026, GR12 and ADR 0010 updated 2026-11-21): T262. Point 7: the first measurement was not valid (0.2.0 was run without the template); measured again the same day with the template on in both, 55 files: 0.2.0 about 1 s faster than this build (the owner, not measured closely). F-Q11's template per shot costs next to nothing |
