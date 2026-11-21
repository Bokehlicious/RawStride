# T251. ADR 0026 F3c: the tabs follow their folders on disk (pw-loupe) — Open

- **Why:** ADR 0026 step F3 (accepted by the owner 2026-11-18), §3.2 and §3.3: the active tab
  follows its folder at once, another tab when it becomes active (F-Q8); a tab whose watch could
  not start or has ended is listed when it becomes active and on F5 (F-Q3, no timer); a folder
  that is gone leaves the tab as it was and says so (F-Q4); the current image removed shows the
  following one and says so (F-Q5); the status line says briefly what another program added or
  removed (F-Q9). T248 notes the watcher's events on the tabs (`changed_on_disk`, `unwatched`);
  T249 made the listing strict, the `Started` event and the texts; T250 the read of only the new
  files and the files waiting for their rating. This task acts on the notes. It also takes up the
  leftovers in `plans/adr-0018-0025-steps.md` ("Taken up when a step is written") that are the
  shell's: RawStride's own changes are never reported as another program's, a folder renamed
  while open, the gap between a tab's listing and its watch, and the thumbnails made anew when
  files come (watched in the hand check). Last task of the chain T249 → T250 → T251; the owner's
  hand check of F3 follows it.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/main.rs` (points 1–10, and their tests)
  - `crates/pw-loupe/src/lib.rs` (only `inside_lexically` added to `pub use tabs::{…}`, for
    point 2)
  - `crates/pw-loupe/src/metadata.rs` and `crates/pw-loupe/src/tabs.rs` (only the
    `#[cfg_attr(not(test), allow(dead_code))]` lines and their "T251 calls it" comments on
    `add_files`, `rating_known` and `inside_lexically` removed, now that the shell calls them;
    review of T250)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **The events:** in `poll_folder_watcher`, `WatchEvent::Started(folder)` notes the tabs as
     `Changed` does (`note_changed_on_disk`): the folder is listed once more now that its watch
     runs, so a file that came between the tab's listing and the watch's start is shown (T249
     may have added this arm already; keep it). The doc comment no longer says acting is F3's.
  2. **RawStride's own work holds a listing back:** a free function
     `disk_listing_held(moves_busy: bool, delete_busy: bool, dialog_open: bool,
     import_destination: Option<&Path>, folder: &Path) -> bool`, true while a batch of moves or
     an undo runs or waits (`self.moves.busy()`), a delete runs (`self.delete.busy()`), a dialog
     that holds the program is open (`self.dialog_open()`), or `folder` is inside the destination
     of a running import (`tabs::inside_lexically`, T250: no file access). While held, the note
     `changed_on_disk` is **kept**, and the folder is listed once the hold ends (point 3 runs
     every frame). Reason, in the doc comment: these change the tabs themselves when they end
     (`replace_tab_files`, `relist_tabs`, an import's result opened), and while they run a file
     can be missing for a moment (a move, the import's `.pwpart` renamed and its template written
     with ExifTool's `-overwrite_original`), which a listing would report as removed by another
     program. A field `import_destination: Option<PathBuf>` is set in `start_import`'s `Ok` arm
     to the form's destination as written (trimmed), and cleared where `ejecting_drive` is
     cleared when the import ends (`ImportEvent::Done` and `TryRecvError::Disconnected`). The
     files RawStride is writing count as listed (T240's `pending`): **`tab_pending` uses
     `MetadataCache::write_pending`** instead of `is_pending`, so an editor save and a write of
     "Apply to All Marked" count too (this also applies to Reload Folder and `relist_tabs`, which
     use it; say so in its doc comment). Accepted edge, also in the doc comment: a move waits only
     for `is_pending`, so a file moved while "Apply to All Marked" still has it pending counts as
     listed and stays in the source tab until the next listing.
  3. **The active tab follows at once:** a method `follow_disk_changes(&mut self, ctx)`, called
     in `ui` right after `poll_folder_watcher` and before the first `follow_tab_folders`. When
     the active tab has a folder, `changed_on_disk` is set and the listing is not held (point 2),
     the note is cleared and the folder is listed (`list_folder`) on the UI thread, as Reload
     Folder does:
     - the listing fails: the tab keeps its files and the status line says
       `folder_gone_text` (F-Q4); also a folder renamed or moved while open, whose watch gives
       no event of its own, is found this way on its next note or activation;
     - otherwise `relist_tab` with `current` `None`, `tab_pending` (point 2), the chosen order,
       the real `ratings_ready()`, `tab_rating` and `modified_time` (through point 9's
       function). When nothing came or went, nothing more happens (no anchor, no text, no new
       thumbnails). Otherwise, in this order: `anchor_from_note` (only now, so an unchanged
       listing leaves no stale anchor); when files were removed,
       `MetadataCache::forget_deleted` with `Relisted::gone` resolved as the cache keys them (they
       leave `images` and the waiting reads, their ratings and turns are dropped, a pending write
       kept, the inspector cleared when its image went), so a file that comes back under the same
       name, for example from the Recycle Bin, is read anew; `MetadataCache::add_files` with
       `Relisted::new` resolved likewise (T250), and **only when it succeeds**
       `Tab::mark_unread` with those of `Relisted::new` whose resolved path it returned (an error
       goes to the status line and marks nothing, so no file waits for a read never sent); then
       `refilter_active(None)`, so under a filter the new files
       wait for their rating; `active_changed(false)` (the thumbnails asked for again for the new
       list, the contact sheet keeping its top image, a preview following its visible images and
       going back to the contact sheet when none is left); then `follow_sort`; the status text
       (point 6) last, so nothing above overwrites it.
     The Exif summary and the histograms are not emptied, and no file already shown is read
     again (F-Q7: a file that only changed is F5's).
  4. **New files are shown when their rating is read:** a method `follow_unread(&mut self)`,
     called in `draw` right before `refilter_when_ratings_change`: when the active tab's
     `unread` is not empty, `Tab::clear_read` (T250) with `MetadataCache::rating_known` on each
     path resolved as `tab_rating` resolves it; when it removed one, `refilter_active(None)` and,
     when `visible` changed, `loupe_follows_visible`. The placement under a rating order follows
     in `follow_sort`, which runs after it in `draw`.
  5. **Another tab when it becomes active** (F-Q8, F-Q3): where a tab becomes active through
     `Tabs::activate` (a click on its tab) or `tab_action` (Next, Previous, and Close for the tab
     that becomes active), before `load_active_tab`: a tab marked `unwatched` also gets
     `changed_on_disk` (the fallback lists it on each activation); when `changed_on_disk` is set
     and the listing is not held, the note is cleared and the folder is listed and relisted as in
     point 3 (point 9's function), but with `ratings_ready` false (the value of `ratings_ready()` there is still the
     previous tab's), with no anchor, no `mark_unread`, no `add_files`, no `active_changed` and no
     `follow_sort` of its own, since `load_active_tab` reads the whole tab next. A listing that fails keeps the tab as it was. The
     status text (point 6, or `folder_gone_text`) is set **after** `load_active_tab`, which
     otherwise clears the message; when the tab has no files left, `load_tab`'s "no images" text
     stays instead. When held, the note stays and point 3 lists the tab once the
     hold ends. One method shared with point 3 is welcome; its doc comment names both uses.
  6. **The status texts** (F-Q5, F-Q9): a free function `disk_change_text(relisted: &Relisted)
     -> Option<String>`: `removed_by_another_program_text` of the current image when it was
     removed (F-Q5, as Reload Folder), otherwise `folder_changed_text(added, removed)` (T249);
     `None` when nothing changed. Shown in the status line (`self.message`) by points 3 and 5.
     No sound, no dialog (§3.3).
  7. **Reload Folder (F5)** on the active tab clears its `changed_on_disk` (it lists the folder)
     and its `unwatched` (so the watch is tried again through `follow_tab_folders`; a watch that
     still cannot start sends `Ended` and the tab is marked again). Nothing else in F5 changes.
  8. **An undo reads what it does not know** (`undo_done`): today it reads the active tab again
     (`load_active_tab`) only when an image brought back is not in the metadata cache, else
     `active_reordered`. The relist there lists the folder from disk, so it may also bring in
     files another program added while the listing was held (point 2); those would never be read.
     The condition becomes: any file of the active tab (resolved as the cache keys them) is not
     in the cache (`has_image`). Nothing else in the undo changes.
  9. **One listing function:** a free function, for example `list_into_tab(tab: &mut Tab,
     listing: Result<Vec<PathBuf>, String>, pending, order, ratings_ready, rating_of,
     modified_of) -> Result<Relisted, String>`, used by points 3 and 5: `Err(folder_gone_text)`
     when the listing failed (the tab unchanged), else `relist_tab`'s `Relisted`. Reload Folder's
     `reload_tab` may use it too, unchanged in what it does.
  10. **Doc comments** of the fields `changed_on_disk` and `unwatched` are in `tabs.rs`, which this
     task does not change; the comments in `main.rs` that say "F3 acts on" are brought up to
     date.
- **Tests to add** (`main.rs`, free functions, as the other free functions of `main.rs` are
  tested):
  - `disk_listing_held`: false with nothing running; true for each of moves, delete and an open
    dialog; true for a folder inside or equal to the import's destination, false for a folder
    outside it (a sibling with a longer name) and with no import.
  - `disk_change_text`: the current image removed gives its text; added and removed without the
    current image give `folder_changed_text`'s; nothing changed gives `None`.
  - point 9's function: a failed listing gives `folder_gone_text` and leaves the tab as it was;
    a listing with a new file and one gone gives a `Relisted` with them (`new` holds the new
    file); a listing with the same files changes nothing; a file `pending` names counts as
    listed.
- **Hand check (the owner, F3 as a whole):** with a release build; a folder of copies on the PC
  (for example 20 RAW files), File Explorer beside RawStride:
  1. Open the folder. Copy 3 more images into it in File Explorer: within about a second they
     appear in the contact sheet with their thumbnails, and the status line says "3 new images".
     (During a longer copy each listing says only what it found, so the text may first say
     "1 new image" and then "2 new images".) Delete 2 images in File Explorer, **not** the one
     marked current in RawStride: they disappear, and it says "2 images removed by another
     program". Give one image 3 stars and turn it, delete it in File Explorer, and restore it from the
     Recycle Bin: it comes back with its 3 stars and its turn.
  2. In the preview, delete the image shown in File Explorer: the following image is shown and
     the status line says "<file> was removed by another program".
  3. With a star filter (for example 1 star or more), copy in one rated and one unrated image:
     only the rated one appears, after a moment; the images already shown do not disappear or
     blink. Do the same with the order "Rating" and with "Capture Time": the new images are
     placed in the order, nothing already shown moves, and the contact sheet does not jump
     (scroll down a little first).
  4. Watch the thumbnails while copying in about 50 images, also with a star filter and the
     order "Rating" (each new image placed makes the thumbnails anew too): say whether the
     thumbnails already shown are made again (flicker) and whether it disturbs.
  5. With two tabs, copy images into the folder of the tab **not** shown, then switch to it: the
     new images are there, and the status line says how many.
  6. RawStride's own changes: rate and turn images, press X with a `_rejected` tab open, undo with
     Ctrl+Z, delete an image to the Recycle Bin, save metadata in the metadata window: no text
     "by another program" appears, and nothing changes that did not before.
  7. Rename the shown tab's folder in File Explorer (a rename gives no notification of its own),
     then copy an image into the renamed folder: the tab keeps its images and the status line says
     the folder is gone or cannot be read. F5 says so again.
  8. Import from a card into a folder that is open in a tab (the destination as the import window
     shows it, a full path): during the import no text "removed by
     another program" appears; when it ends the tab has the new images.
  9. Copy RAW files with their `.xmp` sidecars (rated in another program) from a USB stick: say
     whether a new image shows without its stars; F5 then shows them (accepted limit, see Not in
     this task).
  10. Optional, with a USB stick: open a folder on it, pull the stick out, put it back, press F5 in
     that tab, then copy an image into the folder in File Explorer: it appears.
- **Not in this task:** images shown while importing (F4); a timer for folders without a watch
  (F-Q3: none); reading again a file that only changed (F-Q7); the listing off the UI thread (as
  Reload Folder and opening a folder, the listing runs on the UI thread; a network folder that
  answers slowly holds the window for that time, also for the second listing each tab gets when
  its watch starts); a file listed while it is still being copied, or a RAW whose `.xmp` sidecar
  comes after it, is read once and may show without its rating or capture time until F5 (F-Q7:
  only files that come and go are followed; to `known-limits.md` at Done); a rating key on a new
  image whose rating is not read yet starts from no rating, as during a folder read today (to
  `known-limits.md` at Done); the thumbnail cache keeping images across a
  list change (a task only if hand check point 4 finds the flicker disturbing).
- **Rules:** `docs/tasks/README.md` rules 1–13 (the texts come from `shell_text.rs`, T249);
  `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added (ADR 0026 F3, last of T249 → T250 → T251); waits for T250 |
| 2026-11-21 | Blocked | Chain review (T249 → T251): removed files forgotten with `forget_deleted` (`Relisted::gone`, T250) instead of `prune_metadata_to_open_tabs`, which left them in `images` so a file brought back came without its rating and turn, and dropped the values of files moved to a folder with no tab; `add_files` before `mark_unread`, which marks only what it sent; point 8: an undo reads the tab when any of its files is not in the cache; accepted and noted: a file listed while being copied or a RAW before its sidecar (F5 reads it), a rating key on an unread image, the second listing when a watch starts; hand check: texts during a copy, a restored image keeps its stars and turn, placements in step 4, RAW+XMP from a USB stick |
| 2026-11-21 | Blocked | Cold read: the anchor only when the listing changed something; `mark_unread` + `add_files` + `refilter_active` named (T250 no longer fills `unread` in `relist`); `preview_must_leave` dropped (`active_changed` already returns to the contact sheet); `prune_metadata_to_open_tabs`'s side effects named and the text set last; the "no images" text kept on activation; the ghost file under `write_pending` noted as accepted; point 9: one free listing function, tested; hand check: deleted images not the current one, step 7 copies a file into the renamed folder (a rename gives no event), step 8 with a full destination |
| 2026-11-21 | Open | T250 Done. Its review added to Files: `lib.rs` (export `inside_lexically`), and in `metadata.rs` and `tabs.rs` only the `allow(dead_code)` lines of `add_files`, `rating_known` and `inside_lexically` removed |
