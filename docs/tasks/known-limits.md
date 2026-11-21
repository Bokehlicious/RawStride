# Known limits

Limits and leftovers that were accepted when a task was done or at a backlog sort: none breaks
anything the owner has seen. They are records of how RawStride behaves, not work waiting to be
done. Each becomes a task only if it is seen in use, or it is taken up when its area is next
changed (the item says which). They are read when a task in the same area is written, not at
every hand check.

Ordered by area. Each item names where it came from (the task or review) and the date. Leftovers
that belong to a planned step are in that step's row or notes in `plans/`, not here; ideas and
work waiting for a decision are in [`backlog.md`](backlog.md). An item that becomes a task or is
dropped is removed; git history and `done/` keep it.

## Metadata and the safe write (`pw-metadata`)

- **Lists with qualifiers** (T193's chain review and review, 2026-11-18; sorted after T194, owner
  2026-11-19). Editing such a list with its qualifiers kept is in `backlog.md`. Known limits until
  then, each with its reason:
  - the import's template drops a change to such a list without a notice (`apply_template_safely`
    returns no plan notices): waits for templates in full (ADR 0017 U13);
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
- **List entries in `Edit::Entries`** (reviewer and `/code-review` of T179, 2026-11-17; none is
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
    loops (quadratic, small at today's list sizes). Until `edit.rs` or `template.rs` is next
    changed (for example U13); none is reached from the window, whose list text is trimmed and
    has no empty entries.
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
- **`ImageCheck`: a cleaner design** (`/code-review` of T177, 2026-11-17; for when `ImageCheck` is
  next changed): the stamp is part of the write, not of the release. A write-only hook (for
  example `ImageCheck::before_write`, called only by the safe write) would let `release` be the
  same for writes and moves, and `with_move_release` and the second closure could go.

## Moves to `_rejected` and `_picked`

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
menu by the owner's choice, ADR 0019 §2.9; the template submenu waits for ADR 0017 U13.)

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

- *Ctrl+A while nothing is shown clears the kept marks:* under an active filter while the ratings
  are read, `visible` is empty and Ctrl+A (`Selection::all` of nothing) clears the marks that
  ADR 0014 amendment 3 keeps (cold read of T240).
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

## Card eject

(Independent review of T156, 2026-11-16; none affects the owner's readers.)

- `reader_device` ejects the disk's direct parent, so a reader that is a composite USB device
  (`USB\VID_…&MI_00`) would be refused; walking up with `CM_Get_Parent` to the nearest node with
  `DN_REMOVABLE` would fix it.
- `on_usb` uses a buffer of exactly `STORAGE_DEVICE_DESCRIPTOR`'s size; a driver that answers
  `ERROR_MORE_DATA` would make the reader count as not USB (not ejected, safe).
- `CR_FAILURE` (19) is reused for "no matching disk" and reads like Windows error 19 in a bug
  report. The veto-name length is passed as `MAX_PATH` instead of the buffer's own length.

## LibRaw (`pw-libraw`)

(Reviewer and an independent review agent of T197, 2026-11-19; none is caused by T197, which
avoids the first for X-Trans by refusing a box under 512 px before LibRaw is called.)

- *A failed LibRaw call kills the session:* LibRaw's exception handler calls `recycle()` on an
  error such as `IO_CORRUPT`, so the unpacked RAW data is gone and every later call on that
  `pw_libraw::Session` fails ("Out of order call of libraw function"), Bayer files too;
  `statistics` then gives `Ok(None)` without an error. A fix would reopen the session or report it
  as dead.
- *Panic paths in `pw-libraw/src/lib.rs` from before T197* (AGENTS.md §3): `r.x + r.w` and
  `r.x + r.w + margin` in `render_region_with` can overflow `u32` (a panic in a debug build; in
  release the shim's bounds check and the box check refuse the result), and `Rgb8::crop` is a
  public function with an `assert!`.
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
