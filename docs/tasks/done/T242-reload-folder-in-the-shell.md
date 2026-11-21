# T242. ADR 0026 F1c: "Reload Folder" (F5) in the shell, and tabs listed again after a move or an undo with the placement (pw-loupe) — Done

- **Why:** ADR 0026 step F1 (accepted by the owner 2026-11-18): "Reload Folder" (F5) lists the
  active tab's folder again and applies §2 (the current image, the marks, the filter and the sort
  kept; new files placed in the order; the contact sheet does not jump), then reads every image's
  rating and metadata again, makes the thumbnails anew and drops the views the view core keeps
  (§3.1). T240 made the plain-Rust part (`Tab::relist`, `Tab::place_waiting`,
  `SortFollow::Placed`, `grid::anchored_offset`, `grid::top_position`, temporary files not
  listed) and T241 the command, the key test, the texts and the readers'
  `begin_reload`/`folder_reloaded`. This task wires them into `main.rs`. By the owner's decision
  of 2026-11-21 (ADR 0026 §2 point 4), the tabs RawStride lists again after a move or an undo
  (`relist_tabs`, T183, and the tab an undo brings files back to) use the same placement instead
  of applying the whole order again. Last task of the chain T240 → T241 → T242 (plan, ADR 0026
  F1 row); the owner's hand check of F1 follows it.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/main.rs` (points 1–8 and their tests)
  - `crates/pw-loupe/src/metadata.rs` and `crates/pw-loupe/src/inspector.rs` (point 1's
    `cfg_attr` only; added at the review)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **The command:** T241's two placeholder arms are replaced. `Command::ReloadFolder` in
     `run_command` calls a new `reload_active_tab(&mut self, ctx: &egui::Context)`.
     `command_enabled` gives true when the active tab has a folder (also a folder with no
     images: F5 is how it shows new files) and neither a batch of moves or an undo
     (`self.moves.busy()`) nor a delete (`self.delete.busy()`, every step but idle, its
     preparing step included) is running, as `main.rs` already combines them; a reload while
     files are being moved could list a file on its way. T241's
     `#[cfg_attr(not(test), allow(dead_code))]` on `MetadataCache::begin_reload` and the Exif
     summary's `folder_reloaded` are removed now that they are called.
  2. **The key:** F5 (`reload_key_action`) in the central area's key loop, beside the Delete
     and Ctrl+E keys (asked after Delete and before Ctrl+E, the order `lib.rs`'s test helper
     `command_of_key` models since T241; if the order differs, that helper and its doc comment
     change with it) and with their conditions (not while a text field has the keyboard or a
     dialog holds the program, `self.dialog_open()`; not on a repeat, so a held F5 reloads
     once), through `run_command(&ctx, Command::ReloadFolder)` so a greyed command does nothing.
     F5 with the pointer over the left column stays the library's "Refresh" only (U4-Q1, the
     check near `LibraryAction::Refresh`): the left column is drawn before the central area in
     the same frame, so it notes in a field that it took F5 this frame, and the key loop skips
     the reload then. The field is reset at the start of every frame (in the preview, or with the
     column collapsed, the library's block does not run).
  3. **`reload_active_tab`,** in this order:
     1. An active tab without a folder: nothing.
     2. `list_folder(folder)` fails (the folder is gone, renamed or unreadable): the tab keeps
        its files, nothing is read again, and `self.message` becomes `folder_gone_text(folder)`
        (F-Q4). A second F5 says it again.
     3. The listing is converted to the tab's own form if the tab's paths differ from it (as
        `relist_tabs` and the undo do today).
     4. `tab.relist(listed, None, &pending, order, self.ratings_ready(), &rating_of,
        &modified_time)` on the active tab, with `pending` from `MetadataCache::is_pending` (a
        file RawStride is writing counts as listed, T240 point 5) and `tab_rating` as elsewhere.
        When the tab shows the contact sheet, the anchor of point 5 is taken from the last frame's
        note before the relist.
     5. `exif.folder_reloaded()` and `histograms.folder_reloaded()`.
     6. The reads, as `load_active_tab` does them, with `metadata.begin_reload` instead of
        `begin_load` and without calling the two `folder_loaded` of step 5. Make
        `load_active_tab` take this as a parameter (or share a private function), not a copy of
        its body. Its new thumbnail generation empties the thumbnail cache, and in the preview
        `open_loupe_core` opens the current image (or the one that followed it) again from the
        file (`open_folder` empties the preview's caches).
     7. The status line: an error from the reads (`begin_reload` fails) is shown, as
        `load_active_tab` shows it; otherwise `removed_by_another_program_text(file)` when
        `Relisted::current_removed` is `Some` (F-Q5, §2 point 1); otherwise what
        `load_active_tab` sets (`no_images_text` for an empty folder, else nothing).
     8. `self.follow_sort(ctx)`.
  4. **The sort's waiting files:** `request_capture_times` today returns when the active tab is
     already in the chosen order; then it asks instead for the capture times of the tab's
     `unplaced` files not read or asked for yet (only those), so they are placed when they come
     (F-Q6). `sort_tabs` answers which of three things happened to the active tab: nothing, a
     new order (`SortFollow::Reordered { moved: true }`), or a placement
     (`SortFollow::Placed { moved: true }`, T240). After a new order `active_reordered` does
     what it does today; after a placement it does the same except that it does not scroll to
     the current image: the contact sheet keeps its top image (point 5). The preview gets the
     new list through `set_files` in both cases.
  5. **The contact sheet does not jump** (§2 point 5): every frame `draw_grid` notes, for the
     tab it drew, the scroll offset (`viewport.min.y`), the view's height, the column count it
     used, the top position (`visible_range` with no margin, its start) and the paths of
     `tab.visible` from that position on (or enough of them: at least two screens' worth). A
     relist (point 3, point 6 for the active tab) or a placement (point 4) turns that note into
     a pending anchor. The next time `draw_grid` draws that tab, when `top_position(old_from_top,
     new visible paths)` is `Some`, the offset `anchored_offset(offset, stride, columns,
     old_top, new_top)` is set once with `ScrollArea::vertical_scroll_offset`, and the anchor is
     dropped. While nothing is shown (an active filter while the ratings are read, ADR 0014 Q7)
     the anchor waits; it is dropped when the tab changes, when the frame scrolls by itself
     (the local `scroll_to` in `draw` is `Some`: it wins over the anchor in that frame), or when
     the ratings are ready and none of its files is shown.
  6. **Tabs listed again after a move or an undo** (ADR 0026 §2 point 4, the owner
     2026-11-21):
     - `relist_tabs` (T183): each tab open on one of the folders gets `tab.relist(listed, None,
       &pending, order, ready, …)` (the active tab with the real `ratings_ready`, the others
       `false`, as `replace_tab_files` does) instead of `replace_files` with the new listing,
       and its `self.tabs.forget_sort()` is removed. The rest (the active tab loaded again when
       it was one of them, the sort followed) stays.
     - `undo_done`: the tabs of the folders the files come back to get `tab.relist(listed,
       first_restored, &pending, …)` (the image it makes current today) instead of
       `replace_files` with a full listing, and its `self.tabs.forget_sort()` (with its comment
       "Every tab's sort is forgotten (accepted)") is removed. The rest stays. Note (review of
       T240): `relist` returns early, without using `current`, when nothing is new and nothing
       gone (a file the undo brings back that the tab already lists, for example as `pending`);
       the undo then makes `first_restored` current itself in that case, as it does today.
     - Doc comments say so. A tab without a folder keeps its files as today. `batch_done`'s and
       `deletes_done`'s own `replace_tab_files` (files moved away or deleted, nothing added)
       are unchanged: they never applied the order again.
  7. **Testable parts:** `Loupe` cannot be built in a test (only inside the eframe closure, with
     the view cores, ExifTool and the workers). So the decisions of points 1, 3, 4 and 6 are
     made by free functions (or methods of `Tab`/`Tabs` called from `main.rs`) that take the
     tab(s), the listing or its error, and the values they need, and return what to do, and
     `Loupe`'s methods only call them and apply the answer; for example:
     - `reload_enabled(tab: Option<&Tab>, busy: bool) -> bool`;
     - a function that, given the active tab, `list_folder`'s result, `pending` and the sort,
       relists the tab and returns the status text to show (or the folder-gone text, the tab
       unchanged);
     - `capture_requests(tab: &Tab, order: SortOrder, in_flight: &HashSet<PathBuf>) ->
       Vec<PathBuf>` for point 4;
     - a function that turns the last frame's note and the tab after a change into the offset
       to set (point 5).
     Name in the report which functions were made.
  8. **Tests** (in `main.rs`'s test module, of the functions of point 7; a temporary folder with
     small files where a listing is needed, as the existing folder tests in `lib.rs` make them):
     - a reload of a folder where a file was added and one removed: the new file in its place by
       name, the removed one gone, the current image and the marks of the files still there
       unchanged;
     - a reload whose current file was removed: the following file current, the text is
       `removed_by_another_program_text` of the removed file;
     - a reload of a folder that was deleted: the tab's files unchanged, the text is
       `folder_gone_text`;
     - a reload where a file being written is missing from the listing (`pending`): it stays;
     - `reload_enabled`: false for a tab without a folder and while busy, true for a folder tab
       with images and for one with none;
     - the relist of a `_picked` tab in the order `Modified` with a file added (as `relist_tabs`
       does it): the files already shown keep their order, the new one is placed;
     - the relist of an undo with the order `Modified`: the file brought back is placed and
       current, nothing else moves;
     - `capture_requests` with the tab in the capture order and one `unplaced` file asks for
       that file only; with the order not applied it asks as today;
     - the anchor: after a relist that adds files above the top image, the offset is
       `anchored_offset`'s; with the top image gone, the next file decides; with nothing shown,
       no offset.
  9. **Fixes after the review** (the reviewer's and an independent review, the owner 2026-11-21,
     ADR 0026 §2 point 5). The work done so far stays; only these change:
     1. **A preview with nothing to show goes back to the contact sheet** (ADR 0014 §2.3). Today
        `load_tab` opens the view core with the visible list (`open_loupe_core`) and never calls
        `loupe_follows_visible`, the only call that leaves an empty preview. Two cases leave the
        preview open on nothing: (a) a reload in the preview of a folder whose every image was
        removed (the tab has no files); (b) a reload in the preview under an active rating or
        label filter where no file matches once the ratings are read: `visible` goes from empty
        (while they are read) to empty, `refilter` answers "unchanged", and
        `refilter_when_ratings_change` therefore never calls `loupe_follows_visible`. Fix: after
        a reload's reads begin (`reload_active_tab`, after `load_tab(true)`), a tab in the
        preview with nothing visible and the ratings ready goes back to the contact sheet
        through `loupe_follows_visible`; and in `refilter_when_ratings_change`, when the ratings
        became ready, `loupe_follows_visible` is called also when `refilter_active` answers
        "unchanged" but nothing is visible in the preview (it already does nothing outside the
        preview). Its answer (the position to scroll to) is used as the other callers use it.
        If the decision can be a free function (for example "the preview must leave: mode,
        visible empty, ratings ready"), test it in `main.rs`'s test module.
     2. **An undo always scrolls to the image brought back** (the owner 2026-11-21). Today
        `undo_done` calls `active_reordered` (which scrolls to the current image) when every
        restored image is still in the metadata cache, and `load_active_tab` (no scroll; the
        anchor from `relist_folder_tabs` keeps the top image) otherwise. Fix: after an undo whose
        active tab was one of the folders, the pending anchor is dropped (`grid_anchor = None`)
        and the contact sheet scrolls to the current image (`scroll_request` from
        `Tab::position`) in both branches; in the preview nothing changes. The relist after a
        move (`relist_tabs`, also called at the end of `undo_done` for the folders the files
        left) and a reload keep the top image as now; when `relist_tabs` at the end of
        `undo_done` takes a new anchor for the active tab, the undo's scroll still wins (a
        scroll of that frame drops the anchor, point 5). Doc comments say so.
     Not changed (accepted at the review): the anchor uses the previous frame's column count and
     the current thumbnail size (`backlog.md`, "Leftovers of writing ADR 0026 F1"); the note
     matched by tab index and folder; the paths copied by `grid_note` each frame; an anchor that
     waits while a read failed under an active filter.
- **The owner's hand check of F1** (after the review), in the contact sheet and the preview:
  1. Copy two images into an open folder with Explorer; F5: they appear in their place by name,
     the current image and the marks stay, the contact sheet does not jump.
  2. The same under "Capture Time" and under "Rating": the new images come at the end and move
     to their place once their time or rating is read; nothing else moves, the contact sheet
     does not jump. Watch whether the thumbnails flash at the placement (`backlog.md`,
     "Leftovers of writing ADR 0026 F1").
  3. Delete the current image in Explorer while the preview shows it (a JPEG: a RAW shown is
     held open by its decoder); F5: the following image is shown and the status line says
     "{file} was removed by another program".
  4. Change a rating with another program (or edit a JPEG's pixels); F5: the new rating and
     thumbnail are shown. (A rotation made by another program is not followed: `backlog.md`.)
  5. Rename a folder whose tab is open but not active (or close the program's view of it first:
     Windows refuses to rename a folder with an open file in it), make the tab active, F5: the
     tab keeps its images and the status line says the folder is gone.
  6. A rating filter with marked images; F5: the marks of the files still there are kept once
     the ratings are read; switching to another tab and back keeps them too (ADR 0014
     amendment 3).
  7. F5 with the pointer over the library reads the drives again and does not reload the tab;
     F5 elsewhere reloads; the File menu shows "Reload Folder F5", greyed for an empty tab.
  8. Move images to `_picked` (P) with the `_picked` tab open and sorted by Modified: the images
     already in that tab do not move; Ctrl+Z: the images come back to the first tab in their
     place, nothing else moves in either tab, and the contact sheet scrolls to the image brought
     back (point 9.2).
  9. In the preview, remove every image of the folder in Explorer (JPEGs), F5: back to the contact
     sheet, which shows the empty folder's text. A rating filter that no image matches, in the
     preview, F5: back to the contact sheet once the ratings are read (point 9.1).
  (That no `.pwtmp-`/`.pwbak-` file is listed during a write depends on timing and is shown by
  T240's test, not by hand.)
- **Not in this task:** following the disk by itself (ADR 0026 F2, F3) and the import (F4); other
  tabs open on the same folder are not reloaded by F5 (§3.1 says the active tab; F3 marks them);
  the compare's held image and "Only the marked" after a reload, `list_folder`'s skipped entries
  and a read of only the new files (`backlog.md`, "Leftovers of writing ADR 0026 F1"); F5 is not
  greyed while an import runs into the folder (F4 changes the import).
- **Rules:** `AGENTS.md` §3 and §7, `docs/tasks/README.md` rules 1–13. No new dependency. An
  independent review of the diff follows the reviewer's own (the shell's state while the user
  keeps working, `docs/tasks/README.md`, "An extra code review").
- **Wait for:** T240 and T241 Done.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added; waits for T240 and T241. The owner decided 2026-11-21 that the tabs listed again after a move or an undo use the placement (ADR 0026 §2 point 4) |
| 2026-11-21 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README and this file, T240 and T241 for their APIs): `Loupe` cannot be built in a test, so the decisions are free functions tested on their own (point 7); the left column is drawn before the key loop, and its F5 note is reset every frame; `self.delete.busy()` named; `vertical_scroll_offset` exists in egui 0.37; the anchor needs the view height and the columns too, and the local `scroll_to` (not the field) wins in its frame; `undo_done` has its own `forget_sort`, now removed with the owner's decision (point 6), and a delete never applied the order; hand check: a JPEG for the deleted current image, a renamed folder with nothing open in it, the temporary files left to T240's test, an undo added to item 8 |
| 2026-11-21 | Blocked | Chain review of T240–T242 (fresh agent, the three task files): T241's placeholder arms (one makes the item greyed) replaced here; `Placed` tells a placement from a new order; the note made every frame so a placement inside `sort_tabs` has an anchor; `pending` passed to `relist`; the order of the reload's steps and which text wins; `reload_enabled` true for a folder with no images; the compare's held image, "Only the marked", `list_folder`'s skipped entries and a read of only the new files to `backlog.md` |
| 2026-11-21 | Blocked | T240 Done; waits for T241. Review of T240: point 6 notes `relist`'s early return without `current` for the undo |
| 2026-11-21 | Open | T241 Done. Review of T241: point 2 names the order of F5 in the key loop (after Delete, before Ctrl+E, as `command_of_key` models); a read in flight at a reload is still taken (`backlog.md`), to be watched in the hand check |
| 2026-11-21 | Ready for review | Done by the task agent. `main.rs`: `Command::ReloadFolder` runs `reload_active_tab`, enabled by `reload_enabled`; F5 in the key loop after Delete and before Ctrl+E (`command_of_key` unchanged), skipped when the left column took it (`library_took_f5`, reset each frame); `load_active_tab` shares `load_tab(reload)`; `sort_tabs` returns `SortChange` (`Placed` → anchor, `active_changed(false)` without the scroll); `relist_folder_tabs` (`relist_tabs` and `undo_done`, both `forget_sort` removed); `GridNote`/`grid_anchor` with the offset set by `vertical_scroll_offset`. Free functions: `reload_enabled`, `reload_tab`, `relist_tab`, `capture_requests`, `grid_note`, `anchor_step`, `tab_pending`. Also `metadata.rs` and `inspector.rs` (point 1's `cfg_attr` removed; not in the Files list). Point 3.3: `list_folder` of the tab's folder already gives the tab's form, so nothing is converted, as before. The note keeps no height: it picks the paths from it. 9 tests added; test-inventory 2097 passed, 76 ignored. check.ps1 all OK, no clippy warnings |
| 2026-11-21 | Open | Sent back after review. The reviewer's review (points 1–8 as written; `metadata.rs`, `inspector.rs` added to Files) and an independent review (fresh agent, read only): finding 1, an empty preview after a reload (every image removed, or a filter matching nothing once the ratings are read): point 9.1; finding 2, an undo scrolled or kept the top image depending on the metadata cache: the owner decided 2026-11-21 that an undo always scrolls to the image brought back (ADR 0026 §2 point 5): point 9.2. Accepted, not changed: the anchor's previous-frame layout (`backlog.md`), the note matched by tab index and folder, `grid_note`'s per-frame copies, an anchor waiting after a failed read; the thumbnails remade at each placement were already in `backlog.md` (hand check item 2). Hand check items 8 and 9 for the fixes. The reviewer's clean run of check.ps1 before: all OK, 2097 passed, 76 ignored, no clippy warnings |
| 2026-11-21 | Ready for review | Point 9 done by the task agent (`main.rs` only). 9.1: free function `preview_must_leave(tab, ratings_ready)`; `reload_active_tab` calls `loupe_follows_visible` after `load_tab(true)` when it is true (its position to `scroll_request`), and `refilter_when_ratings_change` calls it also when `refilter_active` answers unchanged but `preview_must_leave` is true. 9.2: `undo_done`, when its active tab was one of the folders, drops `grid_anchor` and sets `scroll_request` from `Tab::position` in the contact sheet, in both branches; doc comments of both. 1 test added (`the_preview_leaves_with_nothing_visible_once_the_ratings_are_ready`); 9.2 has no unit test (`Loupe` cannot be built in a test), hand check item 8. test-inventory 2098 passed, 76 ignored. check.ps1 all OK, no clippy warnings |
| 2026-11-21 | Done | Point 9 reviewed (reviewer): as written; no second independent review (a small fix written exactly from the first). `refilter_when_ratings_change` ignores `loupe_follows_visible`'s position, as before T242 (accepted). The reviewer's clean run of check.ps1: all OK, 2098 passed, 76 ignored, no clippy warnings. CHANGELOG Added line for Reload Folder. Moved to `done/`; the owner's hand check of F1 (items 1–9) follows in a new session |
| 2026-11-21 | Done | Hand-checked by the owner (F1, items 1–9): all as written. Item 2: the thumbnails flash briefly at the placement, not disturbing (`backlog.md`, "A placement remakes every thumbnail", kept for F3). Item 4: a rotation made in DxO PhotoLab of a JPEG was shown after F5 (likely an image whose orientation RawStride had not read yet; the `backlog.md` gap for an image already shown stays). Item 5: the tab keeps its images and the status line says the folder is gone; its thumbnails show "Unreadable", as the files cannot be read under the old name (accepted; `backlog.md`, "A gone folder's tab shows … thumbnails"). Item 9: a rating filter cannot be set in the preview, so the second half was checked with the filter set in the contact sheet and the matching ratings lowered by another program before F5 |
