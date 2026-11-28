# Known limits

Limits and leftovers that were accepted when a task was done or at a backlog sort: none breaks
anything the owner has seen. They are records of how RawStride behaves, not work waiting to be
done. Each becomes a task only if it is seen in use, or it is taken up when its area is next
changed (the item says which). They are read when a task in the same area is written, not at
every hand check, and an area's items are gone through with the owner when an ADR there is
finished (`README.md`, "Known limits after an ADR"); an item marked "Accepted for good" is not
asked about again.

Ordered by area. Each item names where it came from (the task or review) and the date. Leftovers
that belong to a planned step are in that step's row or notes in `plans/`, not here; ideas and
work waiting for a decision are in [`backlog.md`](backlog.md). An item that becomes a task or is
dropped is removed; git history and `done/` keep it.

## Metadata and the safe write (`pw-metadata`)

- **The free-space check under several writes** (T266, ADR 0007 §9.6 point 2; accepted in its
  task, 2026-11-22): with several write processes, each file's free-space check (§9.1, §9.2)
  sees the space before the other threads' copies exist, so several writes on an almost full
  volume can all pass it; a copy write then fails cleanly and the original is unchanged. A task
  only if it is seen in use.
- **Two pools of write processes can meet on one target** (T268, ADR 0007 §9.6 point 2; the
  owner, 2026-11-22): `write_key` keeps one target from being written twice at once only within
  one pool, so the metadata worker's pool and the import's pool can still meet on one target (a
  backup folder open in a tab, or a sidecar `A001.xmp` shared by an existing `A001.CR2` and an
  imported `A001.ARW`), as the worker's and the import's single processes could before. A task
  only if it is seen in use.
- **Writes cut off at the app's exit** (T268, 2026-11-22): at the exit up to four transactions
  of each pool can be cut off after `EXIT_WAIT_LIMIT` (one before T268); the journal recovers
  them at the next start.
- **Lists with qualifiers** (T193's chain review and review, 2026-11-18; sorted after T194, owner
  2026-11-19). Editing such a list with its qualifiers kept is in `backlog.md`. Known limits until
  then, each with its reason:
  - the editor gives no warning before OK that the list will not be written: not needed while the
    info panel's notice and the write notice tell the user;
  - `write_plan::plan` itself does not refuse a changed list with qualifiers (only its callers in
    `edit.rs` put it back; `write_rating_safely` writes no list): taken up when a new caller of
    `plan` comes;
  - recent values (`main.rs` `recorded`) also record a keyword that was put back: harmless to
    offer again;
  - the batch's "Written" count also counts an image whose write was `Unchanged`: ADR 0017
    U12d-Q8 decides that an unchanged image counts as written;
  - an `xml:lang` on a list item is not in ExifTool 13.59's JSON (so not marked) and is kept by a
    whole-list write, but also given to the new items: belongs with editing such lists;
  - T193's hand check (owner, 2026-11-19) did not try a JPEG with IIM, a list with qualifiers and
    an IIM/XMP conflict (no such file at hand), so the held list's `DigestNotWritten` is checked
    only by tests: done when such a file is at hand.
- **List entries in `Edit::Entries`** (**accepted for good, 2026-11-24**; reviewer and `/code-review` of T179, 2026-11-17; none is
  reached by T180's use):
  - *`same_value` compares list entries on the IIM prediction:* removing a keyword over 64 bytes
    and adding it cut to 64 bytes is no change and is lost without a notice (as for `Set`). Rare;
    a task if it is seen in use.
  - *Four blank tests:* `apply_entries` uses `trim()` for `add`, `Metadata::set` spaces only
    (`is_empty_text`), `iim::is_blank` spaces and NUL, `template.rs` its own `is_blank`; a
    NUL-only or tab-only entry is treated differently on each path. With the tidying below.
  - *Tidying:* the append rule repeats `template.rs`'s list Append (a shared helper);
    `entries_left_alone` works out afterwards what `apply_entries` skipped (one source would keep
    them in step); the separate empty-result branch beside the `Empty` fallback; `contains` in
    loops (quadratic, small at today's list sizes). U13 changed both files without it (T296,
    2026-11-23, removed the ingest template's modes and left this out on purpose; none of it
    became moot), so it waits for a task of its own (**kept** at the sort after U13c, 2026-11-23
    (owner)); none is reached from the window, whose list text is trimmed and has no empty
    entries.
  - *A comma in an entry* (chain review of T179 and T180; owner, ADR 0017 U12d-Q8): an entry
    with a comma ("Lund, Sweden") is split by the window's list text, so removing it removes
    nothing on the other images while the confirmation names it; a failed file of a batch can
    only be retried by making the change again. Known limits in ADR 0017; the comma needs another
    way to edit lists (since T108); a task if either is seen in use.
- **Keywords and ratings of other programs** (independent review of T205, 2026-11-19; stays,
  owner, after P0b's hand check): a keyword that itself holds `|` is copied into a flat
  `lr:hierarchicalSubject`, which then reads as nested and is kept unchanged from then on (none of
  68,647 keywords in ADR 0030's K0 sample holds `|`); flat entries of `lr:hierarchicalSubject`
  that are not among the keywords are replaced when the keywords are written (ADR 0007 §8, Q15,
  as decided); `XMP-microsoft:Rating` and `XMP-microsoft:RatingPercent` (Windows Explorer's) are
  not in ADR 0007 §6.3 and stay as they are when RawStride writes a rating: taken up if Windows
  Explorer is seen showing a stale rating.
- **Creating Exif in a JPEG was measured on JFIF only** (independent review of T233,
  2026-11-20): `verify.rs`'s `creates_exif_block` allows the five `EXIF_BLOCK_KEYS` that ExifTool
  13.59 adds when an orientation write creates Exif in the JFIF JPEG of `tests/edits.rs`. A JPEG
  without Exif of another kind (for example with an Adobe APP14 segment and no JFIF) may get
  another set, and then the safe write refuses the rotation (fails safe: the file is unchanged,
  the user sees a failed write). A task only if one is refused.
- **A RAW's own embedded XMP orientation is not read** (chain review of T230–T232, 2026-11-20):
  the folder's batch read takes `XMP-tiff:Orientation` from a RAW's sidecar only (`rating_file`),
  never from XMP inside the RAW file itself. ADR 0024 §2.4's "embedded" XMP is that of a DNG,
  JPEG or TIFF. A task only if a camera or program is found that puts a different orientation in
  a RAW's own XMP.
- **`ImageCheck`: a cleaner design** (**accepted for good, 2026-11-24**; `/code-review` of T177, 2026-11-17; for when `ImageCheck` is
  next changed): the stamp is part of the write, not of the release. A write-only hook (for
  example `ImageCheck::before_write`, called only by the safe write) would let `release` be the
  same for writes and moves, and `with_move_release` and the second closure could go.

- **A template file left in `.saving`** (**accepted for good, 2026-11-24**; T285's review, 2026-11-23): when ExifTool does not answer in time but writes the new file afterwards, `write_template_file` has already cleaned up, so the file (and the `.saving` subfolder of the templates' folder) stays. `.saving` is never listed among the templates, so nothing is loaded from it. A task only if it is seen in use.

- **Variables: the camera's tags in other groups** (**accepted for good, 2026-11-24**; T288's review, 2026-11-23; **kept** at the sort after U13b, 2026-11-23 (owner)): `{make}`, `{model}` and the date variables read `IFD0:Make`, `IFD0:Model` and Exif DateTimeOriginal. A RAW whose maker stores them elsewhere gives an empty `{make}` and `{model}` and no capture time (a date variable then fails that image). By ExifTool 13.64's tag list Panasonic RW2 has Make and Model in `IFD0`; where an RW2's DateTimeOriginal is was not seen, and no real file was checked. A task only if it is seen in use.

- **Variables: a structure's sub-field emptied by filling** (**accepted for good, 2026-11-24**; T288, accepted in its chain review, 2026-11-23; **kept** at the sort after U13b, 2026-11-23 (owner)): in a Set of a whole structure list from the metadata window (Location Shown, for example), a sub-field whose variable fills to nothing (`{make}` without a Make) is dropped from its entry, so that sub-field's old value leaves the entry too, as the window's Set replaces the whole list.

- **Variables: a Lang Alt inside a structure is not cleaned** (**accepted for good, 2026-11-24**; T288's review, owner 2026-11-23; **kept** at the sort after U13b, 2026-11-23 (owner)): a Lang Alt sub-field (Location Shown's Location Name, for example) whose `x-default` fills to nothing while another language has text keeps an empty `x-default` beside that language, and is written so. The metadata window's own Lang Alt fields send only `x-default`, so they are not affected (an empty one is no change). A task only if it is seen in use.

- **Variables: the §11 warnings count an escaped text** (**accepted for good, 2026-11-24**; T289, 2026-11-23; **kept** at the sort after U13b, 2026-11-23 (owner)): the metadata window's warnings look at the form's edits, where a text read from the file has its braces doubled (`variables::escape`), so an IIM length warning counts a keyword `{x}` from the file as the five bytes of `{{x}}`, and a typed text with its variables (`{date}`, not the date). A task only if a wrong warning is seen in use.

- **Variables: another program's brace in pasted text** (**accepted for good, 2026-11-24**; T289's review, 2026-11-23, ADR 0017 U13-Q9 amended; **kept** at the sort after U13b, 2026-11-23 (owner)): Paste All Fields counts as typing, for fields and for a structure list's shown sub-fields, so a text with another program's `{x}` copied and pasted is a variable error until it is written `{{x}}`. A text read from the file and left unchanged is kept as it is.

## Moves to `_rejected` and `_picked`

**Accepted for good, 2026-11-24** (owner, at the sort after ADR 0017's last step): every item below; not asked about again unless seen in use.

- **`relocate::companions`** (`/code-review` of T169, 2026-11-17; the moves are Windows only;
  until a Linux or macOS port):
  - *Case-sensitive file systems:* a file whose name differs from the image's only in case is
    skipped as if it were the image, so it does not move with the group.
  - *Links and read errors:* a sidecar that is a symbolic link or junction is not listed (it
    stays behind); one entry that cannot be read fails the whole listing.
- **From the review of T174** (`/code-review`, 2026-11-17; tidying the user does not see, and rare
  races a move's undo puts right):
  - *The undo's state lives in `main.rs`* (`MoveRun::Undo`, `UndoImage`) without unit tests,
    beside `Batch` in `moves.rs`; an `UndoRun` in `moves.rs` would make it testable. Also
    small: `forget_sort` after an undo clears every tab's sort, not only the listed tabs';
    `send_moves` builds the extension list each frame; the `tab.folder == None` branch in
    `undo_done` cannot run; `replace_tab_files` gives every tab of the active folder
    `ratings_ready` (their files and ratings are the active tab's).
  - *Left by T178* (its cold read): P then X within a frame moves the image to `_picked`
    though rejected (the Pick batch holds it); a RAW skipped (its reject write failed) while its
    JPG partner is sent goes along with the JPG to `_rejected`; a rating write whose answer
    never comes keeps its image's move waiting. From its review: a second P or X on an image
    whose move failed in the running batch is left out until that batch is done (`contains`
    holds it, since a failed answer may be a JPG its RAW partner moved along).
  - *Left by T183* (its cold read, 2026-11-17): a tab on `_rejected` or `_picked` made active
    while a batch runs is loaded again when the batch is done (its position, filter and
    inspector reset); a tab open on a `_rejected` folder that was deleted gives a listing error
    after every batch; an undo that partly fails is not pushed back, so its failed images can no
    longer be undone. From its review (`/code-review`, 2026-11-17): a move or a move back that
    fails and whose rollback also fails (files left in `_rejected` or `_picked`) relists neither
    that folder's tab nor the source tab when no other image of the folder succeeded
    (`relist_tabs` takes only the moved and restored images); the active `_rejected` tab loaded
    again (`load_active_tab`, `begin_load`) while a rating write of one of its images is in
    flight may show that rating as unread for a moment; `undo_done` forgets and follows every
    tab's sort twice when a `_rejected` or `_picked` tab is open, every finished batch with such a
    tab open re-sorts every tab (modification times read again), and `relist_tabs` resolves each
    folder with `same_folder` twice (its pre-check and `tabs_of_folder_mut`).
- **From the review of T170** (`/code-review`, 2026-11-17; not seen in the owner's hand check of
  U9b):
  - *A view core may open the file again between the release and the move:* the worker releases
    the image files one by one and then moves them; a render or prefetch in between opens a new
    session, and the move fails with "open in another program". The same gap exists before a
    safe write. If it is seen, a task holds the view cores' reopening of the group's files until
    the move's answer.
  - *The move's answer is text:* `Event::Moved` and `Event::MovedBack` carry `MoveError` as a
    `String`, so its `left` (files that could not be moved back) is lost for the undo list.
    ADR 0018 B-Q17 (owner, 2026-11-17): only the delete's answer gets a structured error now; the
    delete's `DeleteError` (T186) is the one to copy when the moves are next changed.

## Delete to the Recycle Bin

(T189's brief and its reviews, 2026-11-18; none seen in the hand checks of B4 and B5. The limits
the owner accepted outright are in ADR 0018 §2.7.)

- *The worker:* if the metadata worker stops after it took a request, its answer never comes
  and the delete stays busy, which also holds moves back (the moves have the same gap). Belongs
  with the ExifTool timeout in `backlog.md`.
- *What is shown:* a held Delete key repeats "A delete is still running"; nothing is shown while
  the prepare runs; between the first answer and the end of the batch the deleted images are
  still in the list, and a rating key on one fails (as for the moves).
- *Paths:* a JPG partner left out of the plan is not waited for, so a rating write still queued
  for it fails after the delete (noise, not loss); a folder read already sent may put a deleted
  path back into the metadata cache (the tab's list is the source); a `_rejected` folder reached
  under another spelling is not matched by `same_path`, so a later Ctrl+Z on it fails safely.
- *Other windows:* while a delete runs, the metadata window can be opened on an image being
  deleted, and its save then fails. While the confirmation waits, `same_folder` (which resolves
  the folder on disk) is asked several times a frame; the drive was accepted by T185's check, so
  it is a fixed drive.
- *Not hand-checked:* a network folder refused with "the drive has no Recycle Bin" (the owner
  had no network drive at the hand checks of B4 and B5, 2026-11-18). T185's decision tests cover
  `DRIVE_REMOTE` and a volume without a name. Checked when a network drive is at hand.
- *A drive without Recycle Bin settings* (T223, 2026-11-20): a fixed drive whose
  `Explorer\BitBucket\Volume\{GUID}` key does not exist is refused as "settings unreadable"
  (ADR 0018 B-Q14), so every delete there fails. GitHub's runner had no key for `D:`; when
  Windows creates the key is not known, so a user's second drive may lack it too. Whether a
  missing key should count as Windows' defaults (bin on, default size) is the owner's decision
  for ADR 0018. Not seen on the owner's machine (the keys exist for every local fixed drive,
  2026-11-17).

## Opening images in another program and in File Explorer

(T201's brief, 2026-11-19, and the reviews of T202 and T203; none seen in the hand check of
ADR 0019 E1–E3.)

- *The length:* `command_line_length` follows `std`'s quoting on Windows as it is today; a later
  `std` that quotes otherwise is caught only by the start's error (the start then fails with
  Windows' reason).
- *`.bat` and `.cmd`:* a batch file set up as a program is measured as an `.exe`; `std` starts
  it through `cmd.exe` and may refuse some arguments, so its start may fail with `Failed`.
- *A request that waits* (T202's brief): quitting while a request waits drops it (nothing is
  started or said); the wait has no time limit, so a worker that never answers a write, or a user
  who keeps rating the targets, holds the start back (the status line keeps "Waiting for writes
  …"); an image moved or deleted while its request waits is left out by its old path, not
  followed to its new place; a program renamed or removed in Settings while a request waits is
  still started by its old name and path; a request waits only for writes already queued, so a
  rating key pressed on one of its images after the start is not seen by the program until it
  reads the file again (ADR 0019 §2.7).
- *What is shown* (T202's independent review): while a request waits only for a move or a delete
  (its writes done), the status line says nothing; when a new request replaces one that waited,
  the old "Waiting for writes …" stays until the start; while a request waits behind a delete
  confirmation left open, a frame is drawn every 50 ms; a repeated Ctrl+E on a missing program
  adds one Tasks line per press.
- *Settings and the system:* after Save, a program whose path is not UTF-8 is used until a
  restart although T200 does not write it to the file; the Settings window has no scrolling, so
  on a very small screen its section "External Programs" may be cut off; the menu checks each
  program's path (`is_file`) every frame while the Image menu is open, so a program on a network
  share that is gone may slow the open menu.
- *Show in File Explorer* (T203's brief and its independent review): `resolve_paths` joins the
  working folder without normalising, so a tab opened on a `\\?\` path or a path with `..` makes
  File Explorer open another folder (it gives no error), and a path over 260 characters may do
  the same; it does not wait for a move, so right after X with "Move Rejected and Picked Images
  to Subfolders" on it may open the old folder (or say the image is no longer in its folder once
  the move is done).

## The right-click menu on images

(Review of T221, ADR 0019 E4, 2026-11-19; none seen in the hand check of E4. The filmstrip has no
menu by the owner's choice, ADR 0019 §2.9.)

- *Keys while the menu is open:* the keys keep acting while the right-click menu is open, as with
  the menu bar's menus: Escape closes it and also clears the marking, the arrows move the current
  image under it.
- *Opening it from the keyboard* (Shift+F10 or the menu key): not built.
- *The first frame:* in the frame of the right-click the menu is greyed by the image that was
  current before (the click is applied after the contact sheet is drawn); nothing can be chosen
  in that frame.

## Rotating images

(Chain review of T235–T237, 2026-11-20. The outside change of a written orientation is taken up
with ADR 0026 F3, in `plans/adr-0018-0025-steps.md`.)

- *A group move does not wait for the partner's turn:* a move by X with "Move Rejected and Picked
  Images to Subfolders" waits only for the image's own pending writes (`main.rs`, `send_moves`),
  not for its JPG partner's; a partner's turn still queued or waiting for its embedded value then
  fails on the moved file and is rolled back. Ratings have the same window; the embedded read
  makes it a little wider.
- *Turns waiting for their embedded value at exit* are listed as unsaved, not written (the exit
  wait does not drain the reader).
- *A RAW+JPEG pair:* a turn acts on the file turned, not on its partner (as a rating key).
  Accepted as it is by the owner in the hand check of ADR 0024 (2026-11-20).

## Reload Folder (ADR 0026 F1)

(T240–T242, 2026-11-21. The F1 leftovers that ADR 0026 F3 must take up are in
`plans/adr-0018-0025-steps.md`.)

- *A read in flight at a reload is still taken* (review of T241): the inspector's, the Exif
  summary's and the histogram's read in flight stays in flight at "Reload Folder", and its answer
  is kept for the same image, so it may show values read just before the reload.
- *The anchor uses the previous frame's layout* (independent review of T242): `anchor_step` is
  given last frame's column count and the current thumbnail size, so a window resize, a
  collapsed left column or a thumbnail size change in the very frame the anchor is set puts the
  top image some rows off. Rare.
- *A reload leaves the compare's held image and "Only the marked" as they were* (cold read of
  T242): `open_loupe_core` opens only the main view core, so a held image changed by another
  program is not read again, and after a reload the image that follows a removed current one may
  be unmarked under "Only the marked".
- *A gone folder's tab shows "Unreadable" thumbnails* (the owner's hand check of F1, item 5):
  after the folder of an inactive tab is renamed, making the tab active and pressing F5 keeps its
  images and says the folder is gone, but the thumbnails, no longer held for an inactive tab,
  cannot be read under the old name and show "Unreadable". A text that the folder is gone in
  place of each thumbnail, or keeping the thumbnails, would say it better. A task if it disturbs.
- *A turn RawStride wrote is kept at Reload Folder* (T250): F5 forgets the embedded turns read
  before, but a value RawStride wrote into a JPEG or TIFF (`confirmed_by_write`) stays, so a
  file turned by RawStride and then by another program still shows RawStride's turn after
  F5. Letting it go needs the thumbnail cache's key to hold the file's
  time and the view core to read the flip again. A task if it is seen in use.

## The folder watcher (ADR 0026 F2)

(T247, 2026-11-21, the reviewer and an independent review.)

- *The parent of a watched folder cannot be renamed:* while a folder is watched, renaming the
  folder above it fails with "Access denied" (NTFS; seen with `std::fs::rename`, T247). The
  watched folder itself can be renamed and deleted. It works again once the tab is closed.
- *A hung open holds back "applied":* a folder still being opened when it leaves the set (an
  unreachable network folder) keeps `wait_until_applied` from returning true until that open
  returns (the network's timeout, 20–60 s); an import that ejects in that time waits 2 s and
  starts anyway.
- *A broken completion port* (never seen) is logged every 50 ms until RawStride ends; the
  thread does not spin.
- *Two notes a tab may miss* (T248): a late `Ended`, sent before its folder left the set and
  was given again, can mark a tab whose new watch works as unwatched, which ends that watch; and
  a tab of the same folder under another spelling (a `subst` drive, a `\\?\` prefix) is not
  noted, since the notes compare paths without reading the disk. Both only cost F3's fallback a
  listing on activation; a `subst` letter mapped onto a card is also not left out before an
  eject.
- *A watch that goes dead without an error* (T252): when Windows does not end a watch whose
  drive went away (a USB stick pulled out: seen in T252's hand check), the tab no longer follows the
  folder and nothing says so; F5 starts the watch anew (`watch_again`). No device notification
  is watched (ADR 0026 F-Q3).
- *F5 on a folder whose open hangs* (T252): `watch_again` replaces an open still waiting on an
  unreachable network folder, which keeps `wait_until_applied` from returning true until that
  open returns (as "A hung open holds back applied"); an import that ejects right after waits
  its 2 s and starts anyway.

## Tabs following the disk (ADR 0026 F3)

(T249–T251 and T254, 2026-11-21, the chain reviews and the cold reads.)

- *A file listed while it is copied is read once* (F-Q7: only files that come and go are
  followed): a file listed before its copy has ended, or a RAW whose `.xmp` sidecar comes after
  it, may show without its rating or capture time until F5. Only its thumbnail is made anew
  when the file changes (ADR 0026 §3.2, the addition to F-Q7, T254); the large image in the
  preview and the order of a Modified or Size sort also wait for F5.
- *A folder opened while a copy into it goes on* (T254): the file being copied was not added by
  a listing, so it is not kept, and its thumbnail made from the part written so far stays until
  F5. The same for a tab made active for the first time during the copy.
- *A thumbnail read before the last write may stay* (T254): the stamp is taken on the UI thread
  just before the request; a request the worker merges into one already in flight (a zoom while
  a read is under way) may keep a thumbnail read before the last write, and its result may be
  cached after `forget_path`. Rare; F5.
- *Write times in 2 s steps* (T254): a USB stick's file system (FAT, exFAT) keeps write times in
  2 s steps, so a change inside one step can be missed; File Explorer setting the file's time at
  the end of the copy normally differs.
- *RawStride's own JPEG writes give a listing* (T254): every rating or turn RawStride writes into
  a JPEG (the safe write sets its write time) gives a `Changed` and a folder listing on the UI
  thread (a kept file's thumbnail is no longer made once more: T255). While a kept file's cell
  waits for its thumbnail, its size and write time are read on the UI thread each frame.
- *RawStride's write of a file still being copied in* (T255): the write renews the kept file's
  stamp whatever changed it before, so a thumbnail made from the part written so far stays until
  F5 when RawStride writes the file (a rating or turn) during its copy, or after the copy's last
  change but before the listing that would have found it changed; a mark set by an earlier
  listing is dropped when its thumbnail comes back while RawStride writes the file. A thumbnail
  read that failed while the safe write had renamed the file away, and whose answer comes after
  the write's answer, stays "Unreadable" until F5 (rare: the gap between the safe write's two
  renames; any turned JPEG could already meet it).
- *A rating key on a new image whose rating is not read yet* starts from no rating, as during a
  folder read.
- *The listing runs on the UI thread,* as Reload Folder and opening a folder: a network folder
  that answers slowly holds the window for that time, also for the second listing each tab gets
  when its watch starts (`WatchEvent::Started`).
- *A file moved while "Apply to All Marked" still writes it* counts as listed (`tab_pending`
  uses `write_pending`, a move waits only for `is_pending`) and stays in the source tab until
  the next listing.
- *A gone file's thumbnail made while it went may be kept* (T257 review, 2026-11-21): a file gone
  from the active tab is forgotten in the kept thumbnail cache (`forget_path` in
  `active_changed`), but a thumbnail of it still being made then is cached when it comes back
  (T256: kept across `set_generation_keeping_cache`). Another file that later comes under the
  same name may then be shown with the old image until F5 or another tab. Rare (the file must go
  while its thumbnail is made, and a new one come under its name in the same folder).

## Import: the template per shot (ADR 0026 F4)

- *A file another program makes under a planned name gets the template* (T258, accepted in its
  task; widened by the extra review, 2026-11-21): a shot's item counts as copied when every
  target it has exists. When another program makes a file of a planned destination name after
  the plan was made (before or during that item's copy), the copy fails ("destination exists"
  or "appeared during copy") and is listed as failed, but the file there gets the import's
  template and is named in `Placed`, so it is shown. The template written before T258 skipped
  failed items.
- *Two shots of the same name that are not next to each other* (T258 extra review,
  2026-11-21): with a name template without `{seq}`, two shots can get the same name with other
  extensions in one folder (for example after the camera's counter wrapped). When they are not
  next to each other in the plan, the first shot's template write can make the sidecar
  (`X.xmp`) before the second shot's card sidecar of that name is copied; that copy then fails
  ("destination exists"), is listed as failed and the card is not ejected. Before T258 the two
  shots shared one sidecar instead.
- *A tab on the backup folder is not held while importing* (T260 chain review, 2026-11-21; not
  new): only the destination's tabs are held (`disk_listing_held`), so a tab open on the backup
  folder may list a backup copy before the import's template is written to it.
- *A folder opened again during an import can keep a copied image hidden* (T260 extra review,
  2026-11-21): when the destination folder is already open and watched, and the user opens it
  again (Recent, the library, a drop) during the import, the tab's noted change is cleared and
  the new listing leaves out the images not yet placed. If the import is then cancelled, or a
  shot's backup copy fails, that shot's copied image is never placed and stays out of the tab
  after the import until F5 or the next change in the folder. Tabs the import opened, F5 and
  the relist after a move are not affected.
- *Another spelling of the destination is not recognised* (T260 extra review, 2026-11-21): the
  import compares folders lexically (`same_folder_lexically`), so a tab open on the destination
  through a `subst` drive or a `\\?\` path does not leave out the images not yet placed (one can
  show before its template is written). (Its last clause, a tab loaded once more at the end of
  the import, went with the end rule in T262.)
- *A cancel during a long copy is seen at the next file* (T268 extra review, 2026-11-22): with
  several write processes the import's pool keeps starting queued template writes until the
  cancel is seen at the next progress call (or in the end's wait); writes started finish and
  are listed. With one process no write ran during a copy. The template's writes add no
  progress step, so the progress may also stay at 100 % while they finish.
- *A backup copy's lines repeat the destination's* (**accepted for good, 2026-11-24**; `/code-review` of T294, 2026-11-23, owner;
  **kept** at the sort after U13c, 2026-11-23 (owner)): the template is written to the destination copy and the backup copy, and each line of a
  failure or a notice names the file only (`file_name(target)`), so with a backup every notice
  (and every failure) shows twice, the same, and the two take two of the ten lines. A task only if
  it is seen in use.
- *The template is copied for each pooled write* (**accepted for good, 2026-11-24**; `/code-review` of T294, 2026-11-23, owner;
  **kept** at the sort after U13c, 2026-11-23 (owner)): with more than one write process,
  `submit_template` gives each job its own clone of the whole `TemplateFile` (up to 50 fields
  with lists) where an `Arc` would share one. Not measured; a task only if a large import is
  seen to slow down.
- *A template reader that stops while the import waits* (**accepted for good, 2026-11-24**; T295's review, 2026-11-23; **kept** at
  the sort after U13c, 2026-11-23 (owner)): when the template thread ends after it has taken the
  import's read at Start, no answer comes, and the dialog shows "Reading the metadata template…"
  with the form and Start greyed until the dialog is closed (closing drops the wait). The thread
  ends that way only on a panic, which the rules forbid; a task only if it is seen.
- *A template path that cannot be read back is not remembered* (**accepted for good, 2026-11-24**; T295's review, 2026-11-23;
  **kept** at the sort after U13c, 2026-11-23 (owner)): `import_template` is written to
  `settings.txt` only when the path reads back as it is (`parse_folder`), so a chosen file whose
  path is not valid Unicode or has spaces at an end is used while the program runs but is gone
  after a restart, without a notice. A file chosen in the open dialog has a full path, so this is
  rare.
- *A change for this import names the import's file, not the one loaded* (**accepted for good, 2026-11-24**; T298's review,
  2026-11-24; **kept** at the sort after T297 and T298, 2026-11-24 (owner)): when another file
  is loaded with Load… in the template window opened from the import dialog and Use for This
  Import is pressed, the row shows the import's own file as "{file} (changed for this import)",
  though the values came from the other file. This follows U13-Q21 (the row names the import's
  file); a task only if it confuses in use.
- *Edit… with a change for this import ignores what the load leaves out* (**accepted for good, 2026-11-24**; T298's review,
  2026-11-24; **kept** at the sort after T297 and T298, 2026-11-24 (owner)): the window is
  filled with `TemplateForm::load` of the template the form made at Use for This Import, and
  `load`'s list of what it could not put in the form is not shown. A field the form makes but
  cannot load back would be lost from the window without a notice; no such field is known, and
  tests cover file → form → template but not form → template → form for every field.

## Templates in the metadata window (ADR 0017 U13d)

Every item below **kept** at the sort after U13d (2026-11-24, owner); the first is a decision,
each of the others a task only if it is seen in use. **Accepted for good, 2026-11-24** (owner, at the sort after ADR 0017's last step): every item below; not asked about again unless seen in use. U13-Q24 stays as decided.

- *Load Template… and "Apply to {n} Marked"* (ADR 0017 U13-Q24, owner 2026-11-24; recorded at
  T300's Done): a loaded field counts as typed, so the batch follows the window's own rules
  (U12d-Q1): a list without + is replaced only on the window's image, while the other images get
  the entries the form added and lose those the window's image had; a list with + adds on every
  image. To replace whole lists on many images, the right-click menu's Apply Template is used.
- *A Location Created error after the first entry* (T300's review, 2026-11-24): Save as
  Template… saves only the first Location Created entry, but an error in any entry refuses it,
  and the error is named by its sub-field without the entry, as if it were in the first. A task
  only if it is seen in use.
- *Apply Template and moves* (T303's chain review, recorded at its Done, 2026-11-24): a move or
  delete of an image while a template batch still has it pending is not held back (moves wait
  only for rating writes); the write then fails and the image is listed as failed.
- *A stopped thread greys the submenu* (T303): a template thread or metadata worker that stops
  with Apply Template's request taken leaves the submenu greyed until the program restarts.
- *No undo* (T303): a template write, from the right-click menu or the import, has no undo.
- *Apply Template during an import* (T303): a template batch on a backup or `subst` tab of a
  running import may write a file the import is about to write.

## The metadata window's layout (ADR 0017 U10b)

(T278's brief and chain review, 2026-11-22; **kept** at the sort after ADR 0017 U10b, 2026-11-22
(owner): none was seen in the hand check, so each becomes a task only if it is noticed in use.)
**Accepted for good, 2026-11-24** (owner, at the sort after ADR 0017's last step): every item below; not asked about again unless seen in use.

- *The Paste button's count after a layout change:* when the layout is changed between Copy All
  Fields and Paste All Fields, the Paste button's tooltip still counts the copied items that are
  now hidden, while the paste skips them (U10b-Q4).
- *An older RawStride:* a version before T277 reading a `settings.txt` with the layout's lines
  gives one problem per line (unknown key) and drops the lines at its next save, so the layout
  is back to the default.
- *A hand-written order line* naming only some entries puts each left-out entry right after its
  default predecessor, which can move the named ones (`editor_section_order = licence` alone
  gives the default order); RawStride always writes every entry (T277's review).

## The key map (ADR 0017 U8)

(T279's review, 2026-11-22; **kept** at the sort after ADR 0017 U8, 2026-11-22 (owner): none was
seen in the hand check, so each becomes a task only if it is noticed in use.)
**Accepted for good, 2026-11-24** (owner, at the sort after ADR 0017's last step): every item below; not asked about again unless seen in use.

- *Alt+Tab can be given to an action:* the map refuses Tab and Shift+Tab (egui's focus) but not
  Alt+Tab, which Windows takes before RawStride sees it; such a key never acts.
- *A hand-written `keys.txt` with a clash:* the clashing line's action gets back those of its
  default keys that are free at that point, which can take a key a later line names; that line
  then clashes too (one problem each). A file RawStride saved never clashes.
- *Key names in the texts* (T280's review): a menu, hover text or label shows the arrows,
  Escape and `−` as symbols, every other key by egui's name ("PageUp", "Equals", "Backtick"),
  so a key the user gives may read less well than the defaults.
- *`keys.txt` is read only at start* (U8's hand check, kept by the owner at the sort): a
  `keys.txt` edited while RawStride runs is used only after a restart, and a later Save of a
  changed map on the page Keyboard Shortcuts writes the program's map over the hand edits. A
  Save that does not change the map leaves the file as it is (T281).
- *The heading row follows sideways one frame late* (T286, accepted when the task was written;
  **kept** at the sort after T286, 2026-11-23 (owner)):
  on the page Keyboard Shortcuts the heading row above the scroll area is shifted by the rows'
  horizontal offset of the last frame, so while scrolling sideways it lags the rows by one frame.

## Card eject

(Independent review of T156, 2026-11-16; none affects the owner's readers.)

- `reader_device` ejects the disk's direct parent, so a reader that is a composite USB device
  (`USB\VID_…&MI_00`) would be refused; walking up with `CM_Get_Parent` to the nearest node with
  `DN_REMOVABLE` would fix it.
- `on_usb` uses a buffer of exactly `STORAGE_DEVICE_DESCRIPTOR`'s size; a driver that answers
  `ERROR_MORE_DATA` would make the reader count as not USB (not ejected, safe).
- `CR_FAILURE` (19) is reused for "no matching disk" and reads like Windows error 19 in a bug
  report. The veto-name length is passed as `MAX_PATH` instead of the buffer's own length.

## The view's reduction below 50 % (ADR 0027 W1)

(T309's extra code review, 2026-11-24; taken up when the view core's plan is next changed.)

Every item below **kept** at the sort after W1 (2026-11-24, owner): each a task only if it is
seen in use.

- *A panel shown or hidden at fit* costs a half-size render while the size changes and a
  reduced render after it; before, the resident half-size view served every fit scale.
- *A view size that alternates by a pixel every frame* (a rounding at some scale factors)
  would keep the view core in a zoom gesture: no exact reduced view, statistics or prefetch
  until it stops. Not seen.
- *The neighbours' prefetch at a free level below 50 %* asks for the current scale without
  the neighbour's own minimum scale, so a much larger image (a panorama) is prefetched larger
  than its fit view.

## Fine details (ADR 0027 W2)

(T313's chain review and independent review and T314's independent review, 2026-11-24; each
accepted by the owner 2026-11-24 when its task was reviewed. The owner decided after W2-Q7's
measurement that no more work is spent on fine details (ADR 0027 §4.2); each item becomes a task
only if it is seen in use.)

- *Waits behind a fine render* (T313, W2-Q2): the render holds the current image's only session
  through the reduction and `to_display` and cannot be cancelled, so a zoom to 100 % just after
  it starts (at a free level below 50 %), E, J and the release for a metadata write
  (`RELEASE_TIMEOUT`, 5 s) wait for it, up to T312's time (about 1.2 s for 61 Mpix).
- *The background worker is taken* (T313): after a step during a fine render only the current
  image is served until it ends, so +1 and +2 of the new position are prefetched late. "Only when
  +1 and +2 are ready" (W2-Q2) is reached by the plan's order: a neighbour's job refused by the
  budget is passed over, and the fine render may then start.
- *The compare's two renders* (T313): with the compare open both cores may make a
  full-resolution render at once; the render's temporary buffers are not in the budget (as for
  `Scaled`).
- *A protected stand-in* (T313's review): while no reduced view is resident, `protected_views`
  may protect the half-size view for the fine want, which makes a budget refusal of the fine
  render likelier under a tight budget.
- *An earlier fine view drawn while paused* (T313's review): after a zoom whose new fine render
  fails or is refused, an earlier fine view that still serves stays drawn while the mark shows
  paused.
- *Paused in JPEG mode for an error* (T313's review): an image that cannot be shown
  (`Status::Error`) gives `Paused(Failed)` also in JPEG mode.
- *A budget refusal pauses the image until a new folder* (T313's review, W2-Q6): a refusal that
  was only for a moment is not retried.
- *`plan_of(false)` twice* (T313's review): the interactive `pick` computes it a second time.
- *The dimmed mark over a bright image* (T314's review): the plate is about grey 0x55 there, so
  the dimmed "Fine" has about 1.9:1 (the test holds it against black).
- *The tooltip has no delay* (T314's review): the mark's tooltip shows at once, also while panning
  over the row.
- *A narrow compare side* (T314's review): in a side under about 200 px wide the rating badge can
  cover the row of marks.
- *A `keys.txt` from 0.5.0 with D on another action* (T314's review): that line is lost on
  loading (the key map's rule for a taken key); written in `CHANGELOG.md`.

## Sharpening (ADR 0027 W3)

(T316's independent review, 2026-11-24.)

- *A step's shown time with the other sharpening* (T316's review): `mark_shown` records a step as
  shown when a view of the other sharpening is drawn (a neighbour prefetched before sharpening was
  turned on), so that step's latency is measured too short. For `latency.ps1` after T318, U is
  pressed and the neighbours' prefetch redone before a series starts (`plans/`, W3's row).
- *A `keys.txt` from 0.5.0 with U or Backspace on another action* (T318, accepted as for D):
  that line is lost on loading (the key map's rule for a taken key); written in `CHANGELOG.md`.
- *Holding U* (T318, accepted as for D): sharpening toggles at the key-repeat rate, and each
  toggle plans new views.
- *The edge of a sharpened 100 % view or ring* (T321, accepted): its outermost pixels (about 3 at
  radius 0.8) are sharpened with the edge pixels repeated, as a scaled view's are, so they can
  differ slightly from the same pixels inside the next view.
- *The full-size jobs after U at 100 %* (T321, minor): `wants_a_full_size_view` asks `serving`,
  which ignores sharpening, so the statistics, clipping and focus jobs are not held back while
  the sharpened 100 % view is made.
- *Fine details and sharpening at fit* (T321): the fine render waits behind the sharpened 100 %
  target and ring of the current image, so the fine view comes later than with sharpening off.
- *A late 100 % view or ring of an old sharpening* (T321's independent review, low): `accept`
  checks whether a result is still wanted only for scaled and reduced views, so after U pressed
  twice quickly a late 100 % view or ring of the old sharpening is kept and takes texture budget
  until it is evicted (it is not protected, so it goes first).
- *A drag between 50 % and 100 %* (T322, T325, T326, accepted): the pan view (a scaled view at
  the shown scale over twice the visible region) is asked for only once the image has been
  dragged since it became current (ADR 0027 §4.3, owner 2026-11-25). The first drag on an image
  keeps the last drawn view still until the drag pauses and the new views arrive; the drags after
  it run on the pan view, reduced by the same filter, so nothing moves when the exact view
  replaces it. A drag that leaves the pan view keeps the last drawn view still the same way.
- *A drag and a step at once between 50 % and 100 %* (T326, accepted with the owner's choice A,
  2026-11-25): the pan view's render, once started, is not cancelled by a step, so the next image
  can wait for it (at 51.2 % nearly a whole-image render). Backlog: "A render that a step
  cancels".
- *The compare's second core counts a new image as dragged* (T326's cold read, accepted): a new
  held image shown at the same zoom with another centre (`set_view` from `pw-loupe`) sets
  `dragged` for the second core without a drag, so its pan view may be made needlessly.
- *The mark while the sharpened ring stands in* (T322, accepted; narrowed after T325 and T326,
  2026-11-25): between 50 % and 100 % a drag runs on the pan view, the shown view itself, so this
  is left only for a 100 % ring kept from zooming out of 100 %: the mark says shown while that
  ring is drawn meanwhile, since the ring carries the sharpening, as below 50 % with the
  half-size view.

## Exposure and shadows (ADR 0027 W5)

(T335, accepted 2026-11-26.)

- *A `keys.txt` with O, B, Shift+B, Ctrl+↑, Ctrl+↓ or Shift+Backspace on another action*
  (accepted as for D and U): that line is lost on loading (the key map's rule for a taken key);
  written in `CHANGELOG.md`.
- *The overlays are unexposed* (A-Q6): the histogram, the clipping marks and the focus marking
  are made from the unexposed RAW data, so at +2 EV the clipping marks do not match the picture.
- *The compare with a JPEG or TIFF first side* (or JPEG mode): the six commands are greyed and a
  step is refused there, so a RAW file on the second side cannot be stepped by hand (its modes
  and shadows still apply).
- *The compare's sides one step apart*: a step the second core refuses at its own limit (its
  image's base is higher or lower) leaves the two sides one step apart until a reset; each mark
  shows its own total.
- *A step in the same frame as a step to another image* on one side of the compare is kept on
  that side and reset on the other (the keys run before the drawing); rare, and the marks show it.
- *Reset Exposure and Reset Adjustments for one frame* (T335's review, cosmetic): the shell's
  last source and steps are taken only while the preview is drawn, so on returning to the
  preview after the steps were reset the two items may look enabled for one frame.

## The channel view (ADR 0027 W4)

(W4-Q3, accepted by the owner 2026-11-26.)

- *A TIFF's channel is the display's*: a TIFF is converted to the display's profile when it is
  opened (W1-Q5), so its red, green and blue are the display's channels, not the file's; for a
  RAW or JPEG file the channel is taken before the conversion.
- *A `keys.txt` line on N or Shift+N* (T347): a `keys.txt` that binds N or Shift+N to another
  action acting in the preview loses that line on loading (the key map's rule, as for O, B and U).
- *The histogram, the clipping marks and the focus marking* (§2.1) show the image's data, not
  the channel.
- *Each change of channel makes the views again* (W4-Q4): the shown image's and the prefetched
  neighbours' views are rendered again in the new channel.
- *A view that fails to render with a channel* shows today's error line, and the mark stays
  "being made again", as the shadows' does (T347).

## The white balance (ADR 0027 W6)

- *A `keys.txt` line on W* (T351): a `keys.txt` that binds W to another action acting in the
  preview loses that line on loading (the key map's rule, as for N, O, B and U).
- *As Shot sent again each frame outside the preview* (T351): the shell decides what to send from
  the last drawn frame, which is not drawn while the contact sheet is shown, so it asks the view
  core for As Shot on every frame there; the core does nothing when the mode is already As Shot
  (one lock per frame).
- *The presets of the current image only* (W6-Q4, T351): on the compare's held side the
  right-click menu lists the current image's presets, not the held image's.
- *A grey note kept by As Shot* (T352): after a click that was too bright or too dark, choosing
  As Shot while the mode is already As Shot changes nothing, so the note stays until the next
  click, another white balance, Reset Adjustments, the display mode or another image.
- *The measurements stay as shot* (W6-Q5): the RAW histogram, the exposure statistics, the
  clipping overlay and the focus marking use the camera's balance, so with another balance the
  overlay can mark a channel clipped that the view no longer shows clipped, or the other way
  round. The mark's hover text says so.
- *Auto exposure is not worked out again* (W6-Q5): its value comes from the unadjusted histogram,
  so a balance that lifts one colour can move the brightness a little.
- *Auto white balance is the grey-world rule* (W6-Q1): a picture dominated by one colour (a green
  forest, a sunset) gets a cast of the other kind; Pick Gray and the file's presets are the
  answer, and the hover text says what the rule is.

## Filter on text and metadata (ADR 0022)

- *A sidecar changed by another program* (**accepted for good, 2026-11-26**; ADR 0022 S2-Q6, owner 2026-11-25): the filter's values
  of a tab are read once and kept until its folder is listed again; only RawStride's own writes
  read them again (S2-Q5). A value another program writes is seen by the filter after F5 (Reload
  Folder) or when the folder is opened again. A task only if it is seen in use.
- *A late line of ExifTool's standard error* (**accepted for good, 2026-11-26**; T336's extra review, 2026-11-25): `ExifTool::execute`
  waits 2 ms for standard error after the answer; a line that comes later (a batch read's summary
  "N image files read", a "File not found") is given with the next command's answer. A read of
  many images then may give an image `"no answer for …"` instead of ExifTool's message, and a
  stale line may show in a later error text. The values read are not affected. Older than T336,
  seen more often with batch reads. A task only if it is seen in use.
- *An ExifTool message with `" - "` of its own* (**accepted for good, 2026-11-26**; T336, 2026-11-25): a batch read gives an image
  the standard error lines whose part after the first `" - "` is its path; a message whose own
  text has `" - "` before the path is not matched, and the image gets `"no answer for …"`
  instead. ExifTool's usual "Error: … - <path>" lines are matched. A task only if it is seen in
  use.
- *Two tabs of the same folder* (**accepted for good, 2026-11-26**; T337, 2026-11-26): each tab keeps its own filter values, so two
  tabs of one folder read the same files each. A task only if it is seen in use.
- *The Exif reads' order* (**accepted for good, 2026-11-26**; T337, 2026-11-26): the filter's Exif requests are queued in the capture
  reader's channel at once, so their order is fixed when they are queued; scrolling meanwhile
  does not move the newly visible images first. A task only if it is seen in use.
- *An ExifTool that hangs* (T337, 2026-11-26): `execute` has no read timeout, so an ExifTool that
  hangs on one file blocks the metadata worker, the filter's reads as every other request. A
  task only if it is seen in use.
- *Values of other tabs* (**accepted for good, 2026-11-26**; T337, 2026-11-26): only the active tab's values are read; another tab's
  are read when it becomes active. A task only if it is seen in use.
- *The search's counts in every frame* (**accepted for good, 2026-11-26**; review of T341, 2026-11-26): while a search is active,
  the status line's counts (`star_label_list` and `search_counts`, with each file's rating under
  a star filter) are made again over all the tab's files in every frame drawn, in the contact
  sheet and the preview. A task only if it is seen in use (large folders). Seen in the hand check
  of S1–S3 (owner, 2026-11-26): a little slow, manageable. The same holds for the counts of an
  extended filter (T344, `filter_counts`) and, while the "Filter…" window is open, for each value
  list of an "is one of" row (`value_counts` over the tab's files in every frame).
- *The filter window's draft when the tab changes* (**accepted for good, 2026-11-26**; T344, 2026-11-26): when the active tab stops
  being the one the "Filter…" window was opened for (an import can open a tab while it is open),
  the window closes as Cancel and the rows typed in it are lost. Rare. A task only if it is seen
  in use.
- *An Exif answer looked up in the tab's files* (**accepted for good, 2026-11-26**; review of T344, 2026-11-26): while the active
  tab's filter needs Exif, each capture read's answer is looked up in the tab's file list to
  decide whether to refilter, so a large folder's Exif reads cost time that grows with the
  square of its size, spread over the frames. A task only if it is seen in use.
- *Exif reads queued for a value list* (**accepted for good, 2026-11-26**; T345, 2026-11-26): the "Filter…" window's value list on
  Camera or Lens queues the folder's Exif reads in the capture reader at once; closing the window
  cannot take them back, so they are read and kept. A task only if it is seen in use.
- *Ctrl+Shift+F while the search box has the keyboard* (**accepted for good, 2026-11-26**; T344 chain review, 2026-11-26): the key
  does nothing then, as Ctrl+F; the button or View ▸ Filter ▸ Filter… opens the window. A task
  only if it is seen in use.
- *The capture reader's thread dying* (T345 chain review, owner 2026-11-26):
  `CaptureReader::try_recv` (`sort.rs`) does not tell a disconnected channel from an empty one, so if its thread dies the
  files asked for stay unknown, "Reading metadata {i} of {n} …" never ends and the sort by
  capture time waits. Older than S4. A task only if it is seen in use.

## LibRaw (`pw-libraw`)

(Reviewer and an independent review agent of T197, 2026-11-19; none is caused by T197, which
avoids the first for X-Trans by refusing a box under 512 px before LibRaw is called.)

- *A failed LibRaw render kills the session:* LibRaw's exception handler calls `recycle()` on
  every exception in `dcraw_process` (a stop, `IO_CORRUPT`, out of memory), so the unpacked RAW
  data is gone. Since T327 the shim marks such a session spent (`Session::is_spent`), and every
  later render, `statistics` and `clipping_map` fails with a `spent:` error without calling
  LibRaw; outside T328's stops, nothing opens the file again by itself. The view core (T328's
  review, 2026-11-25) drops a spent session only after a cancelled render: after another failed
  render it goes back to its slot with its bytes counted, and a later statistics, clipping map or
  focus job on it gets "none", which is kept for the image (kept here by the owner at the
  backlog sort after T328, 2026-11-25: no test file makes such a failure). Only the stop
  case is tested: no file in `PW_TESTDATA` and no generated file makes `dcraw_process` fail
  otherwise (a corrupt file is refused at the open, the X-Trans box under 512 px by the shim).
- *A half-size render of an X-Trans file is not deterministic* (found in T327, 2026-11-25, not
  caused by it): two fresh sessions of `_DSF4243.RAF` give half-size images that differ in about
  half the bytes (1.7 M and 3.1 M of 19.5 M in two runs); a whole-image PPG render of the same
  file and every ARW render are identical. `tests/stop.rs` compares only the size there. A task if
  it is seen in use (for example a flicker when the same RAF is shown again at fit).
- *A panic path in `pw-libraw/src/lib.rs` from before T197* (AGENTS.md §3): `Rgb8::crop` is a
  public function with an `assert!`. (The `u32` overflow of `r.x + r.w` and of the margin in the
  region renders was removed by T348, 2026-11-28: the check saturates.)
- *A DNG's daylight from the matrix* (T348's independent review, 2026-11-28): when a DNG has no
  `pre_mul` equal to the D65 matrix's multipliers, "Daylight (LibRaw)" is worked out from
  LibRaw's `cam_xyz`; if that came from a ColorMatrix for another illuminant (for example
  illuminant A), the balance would not be daylight. The owner's Ricoh GR III DNGs use this way;
  the tests only print DNGs. A task if a DNG's "Daylight (LibRaw)" is seen to look wrong.
- *The channel means' colour counter* (T354's independent review, 2026-11-28): it steps by a
  cell and starts again at the pattern's width, so it assumes that width is a multiple of the
  cell, which `pwlr_layout_of` gives for every layout it accepts (Bayer 2 and 2, X-Trans 6 and
  3). Taken up if `pwlr_layout_of` ever accepts another pattern.
- *The cost of an X-Trans region:* the shim's box reaches up to 1487 px plus the region plus
  64 px on each axis, about 0.26–0.45 s for a 1024 px tile on `_DSF4243.RAF` against 1.55 s for
  the whole image (T197's measurements). It matters only if the loupe ever renders many small
  tiles of a RAF.

## Old prototypes

- **The Tauri prototypes' lock files** (T224, 2026-11-20, as T207 accepted the prototypes' broken
  icon path): the prototypes' `run-*.ps1` and `build-*.cmd` seed a missing `Cargo.lock` by copying
  one in. `p2`, `p3`, `p6` and `p6b` copy the root `Cargo.lock`, which has had no Tauri since
  T207, so Cargo resolves Tauri afresh; `p6c-gpu-memory` copies `p6b-gpu-ppg`'s, which is gone, so
  `run-p6c.ps1` (line 25) stops at once and `build-p6c.cmd` (line 7) resolves afresh. A lock file a
  build writes shows in `git status` and is not committed (it would bring the `glib` alert back).
  The prototypes are history and not built; a task only if one is to be run again.
