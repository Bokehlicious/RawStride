# ADR 0026: A folder that follows changes on disk

- Status: **accepted by the owner (2026-11-18):** every question in §5 (F-Q1–F-Q12) answered as
  proposed. Nothing is built, installed or added as a dependency by this ADR. Each step in §6
  becomes its own task, with its own approval and commit.
- **The owner, 2026-11-21** (when F1 was written as T240–T242): §2 point 2, the marks also stay
  under an active filter while the ratings are read (ADR 0014 amendment 3); §2 point 4, the
  placement also when tabs are listed again after a move or an undo; §2 point 5 (review of
  T242), an undo always scrolls to the image brought back.
- **The owner, 2026-11-21** (backlog sort after F3, from T252's hand check): F-Q7 gains an
  addition, §3.2: a listed image file that changes size or write time also counts as a change,
  and the active tab makes the thumbnail of a file another program added anew when the file has
  changed since its thumbnail was asked for (T253, T254).
- **The owner, 2026-11-21** (when F4 was written as T256–T260), §3.4: the import dialog stays
  shown when the first tab opens (the user hides it); the import's progress shows only the copy,
  the template write being part of each file (the separate step "Writing IPTC" goes); after a
  cancel nothing more is written, and copied files the import has not shown are shown only by
  the listing after the import, without the template; and, first, a change of the active tab's
  files keeps the thumbnails already made (step F4a, §3.5), so a tab filling during an import
  does not redraw every thumbnail at each new file.
- **The owner, 2026-11-21** (hand check of T260, point 3): F-Q10 changed. A later destination
  folder's tab becomes active when it opens (its first image shown), if the user has not moved
  since the import last made a tab active; otherwise it opens behind, as before. The end rule at
  the import's end goes (§3.4).
- Date: 2026-11-18
- Related:
  - the owner's wish at the hand check of ADR 0018 B4 (2026-11-18): a tab shows new files, for
    example images put back from the Recycle Bin or copied in by another program, without the
    folder being opened again (`docs/tasks/backlog.md`, "The folder follows changes on disk");
  - ADR 0017 §7.2, two wishes the owner placed here (2026-11-18): **"Load a folder again"** (ADR
    0019 E-Q12) and **"Images shown while importing"**;
  - ADR 0010 §3.1 "Open the result" and GR12, GR13: when an import's tabs open, which this ADR
    may change (§3.4);
  - ADR 0014 (filter and marking), ADR 0017 U9b (moves, undo and `relist_tabs`), U14 (sort, U14-Q5
    "a new read never moves an image"), ADR 0018 (delete to the Recycle Bin, §2.7 and §2.8);
  - ADR 0007 §9 (the safe write and its temporary files), §10 (the ingest template);
  - GR10 and ADR 0002: no new dependency without the owner's approval.

## 1. Context

What the code does today [M, code read 2026-11-18]:

- **A tab is read once, when it is opened.** `list_folder` (`pw-loupe` `lib.rs`) lists the
  folder's image files by name; `Tabs::open_new` / `open_in_active` (`tabs.rs`) store the list in
  the tab, and the filter, the marks and the sort are reset. Switching to another tab and back does
  **not** read the folder again: `load_active_tab` (`main.rs`) uses the stored list, though it
  reads every image's rating again (`MetadataCache::begin_load`) and makes the thumbnails anew.
- **Opening the same folder again** (from the library, Recent or a drop) activates its tab and
  reads it again (GR12), but resets the filter, the marks and the sort, and returns to the
  contact sheet. That is the only way to see files added by another program today.
- **RawStride's own changes already update the tabs.** After a batch of moves, an undo or a delete,
  `replace_tab_files` gives every tab of the folders touched a new list through
  `Tab::replace_files` (`browse.rs`), which keeps the marks of the files still there, the filter
  and the current image (or takes the following one), and `relist_tabs` lists the `_rejected` and
  `_picked` tabs again (T183). This is the building block this ADR reuses.
- **Nothing watches the disk.** No code uses Windows' change notifications
  (`ReadDirectoryChangesW`), and there is no `notify` crate. `pw-loupe` already depends on
  `windows-sys` 0.61.2 with `Win32_Storage_FileSystem`, where `ReadDirectoryChangesW` lives.
- **RawStride's own temporary files sit next to the images.** A safe metadata write (ADR 0007
  §9.2) writes `<name>.pwtmp-<hex>.<ext>` and keeps the original as `<name>.pwbak-<hex>.<ext>`
  for a moment; both end in an image extension, so `list_folder` would list them if it ran during
  a write. The import copies through `<name>.pwpart`, which is not listed.
- **An import opens its tabs at the end.** The import worker copies and verifies every file, then
  writes the IPTC template to the copies one after another with its own ExifTool
  (`import_worker.rs`, `apply_iptc`), then sends `Done`; only then does every destination folder
  open in its own tab, the newest one active (T163).

Three wishes rest on the same thing, a tab whose list of files can change while it is open:

1. **Load a folder again**: a command, the user decides when.
2. **Follow changes on disk**: the tab updates by itself when files come or go.
3. **Images shown while importing**: the destination tab opens on the first file and fills as the
   import goes on.

## 2. Rules for every change of a tab's files

These hold for all three, so a tab behaves the same whatever changed it. They are what
`Tab::replace_files` already does for moves, with the additions marked **new**.

1. **The current image stays** when it is still in the folder. When it is gone, the following file
   becomes current (the previous one at the end), as after a move. In the preview the view of the
   new current image is opened; the status line says why (F-Q5).
2. **The marks of the files still there stay**; a file that is gone loses its mark. Also under an
   active filter while the ratings are read again (owner, 2026-11-21: ADR 0014 amendment 3, §16).
3. **The filter stays.** A new file is shown when it matches, which for a rating or label filter
   is known only once its rating is read; until then it is not shown under such a filter (as a
   tab load does today, ADR 0014 Q7).
4. **The sort stays, and nothing already shown moves** (U14-Q5). **New:** a new file is placed
   where the chosen order puts it among the others; the order is not applied again to the whole
   tab. With a capture-time order, a new file is placed when its capture time is read (F-Q6);
   with a rating order, likewise when its rating is read (as the sort itself waits for the
   ratings, U14-Q4). **The owner, 2026-11-21:** this also holds when RawStride lists its tabs
   again after a move or an undo (the `_rejected` and `_picked` tabs in `relist_tabs`, T183, and
   the tab the files come back to after an undo), which until then applied the whole order again
   (step F1, T242). A delete only removes files and never applied the order again.
5. **New:** the contact sheet does not jump. The image at the top of the visible part stays where
   it is on the screen, even when files are added above it. **The owner, 2026-11-21** (review of
   T242): after an undo the contact sheet always scrolls to the image brought back, since it
   becomes the current image; a reload, a placement and the relist after a move keep the top
   image. A preview left with nothing to show after a change of the tab's files goes back to the
   contact sheet, as after a filter (ADR 0014 §2.3).
6. **New: RawStride's own temporary files are never listed.** `is_listed` leaves out names with
   `.pwtmp-` and `.pwbak-` (and `.pwpart`, already left out by its extension). This also closes a
   small gap today, when `relist_tabs` runs while a rating is being written.
7. **Only the folder itself**, not its subfolders, as today.

## 3. Decision

### 3.1 Load a folder again (step F1)

- A command **"Reload Folder"** in the menu that holds "Open Folder…", with the key **F5**
  (F-Q1). F5 already reads the drives again when the pointer is over the library (U4-Q1); that
  stays, so F5 elsewhere reloads the active tab.
- It lists the active tab's folder again and applies §2 through `replace_files`; then it reads
  **every image's rating and metadata again**, makes the thumbnails anew, and drops the views the
  view core keeps, so a file changed by another program (a new rating from Bridge, an edit in
  Photoshop) is shown as it is now. This is what `load_active_tab` does today, plus the new list.
- A folder that no longer exists, or cannot be read, leaves the tab as it was and says so in the
  status line (F-Q4).
- In the preview it works the same; the current image is shown again from the file.

### 3.2 Follow changes on disk (steps F2 and F3)

- **How the change is noticed: Windows' change notifications** (`ReadDirectoryChangesW`) on one
  watcher thread, with one handle per open folder, through the `windows-sys` that `pw-loupe`
  already uses (a feature line may be added, for example `Win32_System_IO`; no new crate) (F-Q2).
  The alternative, reading every open folder again every few seconds, is simpler but costs a
  folder listing per tab all the time and is slow to notice; it is kept as the fallback where
  notifications do not work (F-Q3).
- **What a notification does: it only says "this folder changed".** After a quiet moment
  (proposed 0.5 s, at most 2 s while files keep coming), the folder is listed again and §2 is
  applied, exactly as F1 does for the list. Notifications are not applied one by one: Windows can
  drop them when its buffer fills, a rename comes as two events, and a folder can be spelt in more
  than one way (`same_folder`), so a new listing is the safe answer every time.
- **What is followed: files that come and files that go** (F-Q7). A file that only changed
  (another program wrote a rating or edited the pixels) is **not** read again by itself; F5 does
  that. Reason: RawStride's own rating writes change files all the time, and telling its own
  writes from another program's is not reliable; reading a whole folder's metadata again after
  every change would also cost too much.
  **Addition (the owner, 2026-11-21, after T252's hand check):** a file copied in slowly (to a
  USB stick) is listed while it is still being written, and its thumbnail is made from the part
  written so far, which fails ("Unreadable") or, when File Explorer has already given the file
  its full size, may look whole but be partly empty. So a change of size or write time of a
  listed image file also counts as a change, and the folder is listed after the quiet moment
  as for any change. RawStride keeps the files another program added to an open tab, with the
  size and write time each had when its thumbnail was asked for; at each listing of the active
  tab, a file whose size or write time differs has its thumbnail made anew, whether it was made
  or failed, and one still being made is checked again once it comes back. Only those files are
  checked, not the whole folder. Nothing else is read
  again: a changed file's rating and metadata still wait for F5, as above.
- **Which tabs:** the active tab follows at once; another tab is marked as changed and listed when
  it becomes active (F-Q8). A tab's watch ends when it is closed or opened on another folder.
- **Network folders and removable drives:** a watch that cannot start, or ends (the drive is
  gone), falls back to listing the folder when the tab becomes active and on F5; nothing is shown
  as an error unless F5 fails (F-Q3). Ejecting a card already empties the tabs inside it
  (`clear_inside`); the watch must not keep the card busy, so it is ended before the eject.
- **With RawStride's own changes:** moves, undo and delete keep updating the tabs at once as
  today; the notification that follows finds the same list and changes nothing.

### 3.3 What the user sees (step F3)

- New files appear in the contact sheet as they come, with their thumbnails made in the
  background. Files that are gone disappear.
- The status line says it briefly, for example "3 new images" or "2 images removed by another
  program" (F-Q9); the texts through text functions (GR14, rule 13).
- No sound, no dialog.

### 3.4 Images shown while importing (step F4)

- **When the tab opens:** when the **first file of a destination folder is copied and verified**,
  that folder opens in its own tab as a contact sheet (GR13), instead of when the whole import is
  done (changes ADR 0010 §3.1 "Open the result" and GR12).
- **How it fills:** from the import's own progress, not from the disk watch: the import worker
  knows each file the moment it is renamed into place, so the tab gets it without waiting for a
  notification and also on a drive where notifications do not work.
- **Which tab is active** (F-Q10): proposed, the first destination folder's tab becomes active when
  it opens, and the later folders' tabs open behind it, so the user is not moved while looking at
  images; T163's rule (the newest folder active) applies at the end only if the user has not
  changed tab or image meanwhile. **Changed by the owner, 2026-11-21 (hand check of T260):** a
  later folder's tab becomes active when it opens, if the user has not moved since the import
  last made a tab active; otherwise it opens behind it. There is no rule at the end.
- **The IPTC template:** today it is written to all files after the whole copy, so an image would
  be shown, rated or opened for a while before its template is in it, and a rating key and the
  template's write could meet on the same file (two writers: the metadata worker and the import
  worker's own ExifTool). Proposed (F-Q11): **the template is written to each file right after its
  copy is verified, and the file is shown only after that**; the import worker already writes the
  template file by file, so it changes the order, not the method. A file whose template write
  fails is shown anyway and is listed in the error list, as today.
- **A rating key during the import** acts on the shown images as usual; since the template is
  already written (F-Q11), it does not meet the import's write.
- **Cancel and failure:** the tabs keep the files that were copied (and, with F-Q11, written);
  nothing more happens to them.
- **The order:** the files are copied in capture order (`pw-ingest`, the shots sorted by time),
  so a tab fills in that order; ADR 0010's "oldest images first" for the tabs is unchanged.
- **Settled when F4 was written (the owner, 2026-11-21, and the reviewer):**
  - *Per shot, not per file:* a shot's files (a RAW, its JPEG, a sidecar from the card) are
    copied one after another; the template is written to the shot's images once all of the
    shot's files are copied and verified, so a sidecar from the card is in place before the
    template writes into it, as today. Then the shot's destination images are shown.
  - *How the tabs know:* before copying, the import tells which destination images it will put
    in place; after each shot, which it has shown. A listing of a destination folder from disk
    while the import runs (a tab opened, Reload Folder, a folder opened from the library) leaves
    out the images the import has not shown yet. The watch's listing stays held while the import
    runs (F3, T251).
  - *A folder already open in a tab* gets the new files added (§2), and is not opened again; the
    first destination folder's tab becomes active, a later one's does not.
  - *The end rule of F-Q10:* "has not moved" means the active tab, its current image and its
    view (contact sheet or preview) are those of when the first folder's tab became active.
    **Changed by the owner, 2026-11-21 (hand check of T260):** "has not moved" is checked when
    each later folder's tab opens, against the active tab, current image and view of when the
    import last made a tab active (the first folder's, or a later one's); the check at the end
    goes.
  - *The import dialog* stays shown when the first tab opens; the tab opens behind it, and the
    user hides the dialog to see the images (the owner).
  - *The progress* shows the copy only; the template write is part of each file, and the
    separate step "Writing IPTC n / total" goes (the owner).
  - *Cancel:* after a cancel nothing more is written; a shot not finished, or finished but not
    yet given its template, is not shown by the import; its copied files are shown by the
    listing after the import, without the template, as a cancel today writes no template (the
    owner).

### 3.5 Thumbnails kept when a tab's files change (step F4a)

The owner, 2026-11-21 (when F4 was written): today every change of the active tab's files (a
listing after a change on disk, a placement in the sort, a new order) starts a new thumbnail
generation, which empties the thumbnail worker's cache and the shell's textures, so every visible
thumbnail is made and drawn anew. A tab filling during an import changes several times a second,
so the contact sheet would flicker all the time. Before F4: a change of the active tab's files
**keeps the thumbnails already made**, matched by file, not by place: the worker changes
generation without emptying its cache (its cache is kept by path), and the shell carries each
made cell and its texture to the file's new place. A cell still being made is asked for again.
Opening a folder, another tab and Reload Folder still make everything anew.

## 4. Not included

- Following subfolders, and a catalogue across folders (`pw-catalog`, ADR 0017 §7.2).
- Reading a changed file's rating or metadata again by itself (§3.2); F5 does that.
- Restoring from the Recycle Bin inside RawStride (ADR 0018 §2.8): a file restored in Windows'
  Recycle Bin simply comes back through §3.2.
- Following the library's tree of folders (U4-Q2: a folder read when expanded, never in the
  background); F5 over the library already reads the drives again.

## 5. Questions (answered by the owner, 2026-11-18)

| # | Question | Answer |
|---|---|---|
| F-Q1 | The name, menu and key of "Load a folder again"? | "Reload Folder" in the menu with "Open Folder…", key F5 (F5 over the library still reads the drives) (as proposed) |
| F-Q2 | Windows' change notifications or reading the folder again at times? | Notifications through the existing `windows-sys`, no new crate; reading again only as the fallback (F-Q3) (as proposed) |
| F-Q3 | Where notifications do not work (a network drive, some card readers): list again at times, or only when the tab becomes active and on F5? | Only on activation and F5; no timer (as proposed) |
| F-Q4 | The folder is renamed, moved or deleted while its tab is open? | The tab keeps its files and the status line says the folder is gone; F5 says so again; the tab is closed by the user (as proposed) |
| F-Q5 | The image shown in the preview is removed by another program? | The following image is shown, and the status line says "{file} was removed by another program" (as proposed) |
| F-Q6 | A new file under a sort order: placed in the order, or put at the end until the tab is opened again? | Placed in the order (§2 point 4); with a capture-time order once its time is read, at the end until then (as proposed) |
| F-Q7 | Follow only files that come and go, or also files that change? | Only files that come and go; F5 reads changed files (§3.2) (as proposed). Addition (owner, 2026-11-21): a size or write-time change of a listed image counts as a change, and the thumbnail of a file another program added is made anew when the file changed since it was asked for (§3.2) |
| F-Q8 | Follow tabs that are not active? | Marked as changed, listed when they become active (as proposed) |
| F-Q9 | Say in the status line when another program added or removed files? | Yes, briefly; no dialog (as proposed) |
| F-Q10 | During an import with several destination folders, which tab is active? | The first folder's tab when it opens; the newest at the end only if the user has not moved (§3.4) (as proposed). **Changed by the owner, 2026-11-21 (hand check of T260):** each later folder's tab becomes active when it opens, if the user has not moved since the import last made a tab active; no rule at the end |
| F-Q11 | The IPTC template: written to each file right after its copy (and the file shown after that), or after the whole copy as today? | Right after each file's copy; to be measured on a card of the owner's (one ExifTool, file by file, as today) (as proposed) |
| F-Q12 | Order of the steps (§6) and where they go in the step plan | F1 first, on its own (it is useful at once); F2–F4 after it, in that order; after ADR 0024 O1, which is next by the plan (as proposed) |

## 6. Steps

Each step ends with tests (GR11) and the owner's hand check.

| Step | Content |
|---|---|
| **F1** | `is_listed` leaves out RawStride's temporary files (§2 point 6); "Reload Folder" (F5): list again, `replace_files`, ratings, metadata, thumbnails and views read again (§3.1); the placement of new files in the sort (§2 point 4) and the contact sheet that does not jump (§2 point 5) |
| **F2** | The watcher thread with `ReadDirectoryChangesW`, one handle per open folder, the quiet moment, start and end with the tabs, ended before an eject; tested with a temporary folder |
| **F3** | The tabs follow the watcher (§3.2, §3.3): the active tab at once, others on activation, the status texts; the fallback where a watch cannot start |
| **F4** | First (F4a, §3.5, the owner 2026-11-21): a change of the active tab's files keeps the thumbnails made. Then images shown while importing (§3.4): the tab opens on the first file, filled from the import's progress; the template per file (F-Q11); the active tab rule (F-Q10); ADR 0010 and GR12 updated |

## 7. Consequences

- **ADR 0010 §3.1 and GR12**: the destination tabs open on the first copied file instead of at the
  end (F4); the template is written per file (F-Q11).
- **ADR 0017 §7.2**: the rows "Load a folder again" and "Images shown while importing" point to
  this ADR's steps.
- **`docs/tasks/backlog.md`**: the item "The folder follows changes on disk" is removed (done
  2026-11-18, at the acceptance).
- **The step plan**: F1–F4 are placed in
  [`adr-0018-0025-steps.md`](../tasks/plans/adr-0018-0025-steps.md) after ADR 0024 O1 (F-Q12).
- **`pw-loupe`**: a watcher module in plain, tested Rust apart from the Windows calls; `Tab` gains
  "changed on disk"; `replace_files` gains the placement of new files in the sort; a new command
  and key; possibly a new `windows-sys` feature line (GR10, the owner's approval in F-Q2).
- **`pw-loupe` `import_worker.rs`**: progress events name each file put in place; the template
  write moves into the copy loop (F-Q11).
- **Cost:** a watch is a sleeping thread and one handle per open folder; a listing after a change
  is the same `list_folder` used today. Nothing changes for a user who does not change files
  outside RawStride.
