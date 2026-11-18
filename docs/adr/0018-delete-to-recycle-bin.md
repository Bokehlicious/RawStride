# ADR 0018: Deleting images to the Recycle Bin

- Status: **accepted by the owner (2026-11-17):** every question answered, all proposals approved
  (§3); B-Q14 and B-Q15 answered when B1 was written, B-Q16–B-Q18 when B2–B5 were
  written (2026-11-17). Nothing is built, installed
  or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit.
- Date: 2026-11-17
- Related:
  - ADR 0017 §2.6 (companion files, R2) and step U9b with its tasks T169–T174 (moving an image
    with its companion files, the move as a request to the metadata worker, the tabs' lists after
    a move); this ADR reuses those parts and is built after T174;
  - ADR 0005 choice 3 (never delete from the card) and choice 5 (videos are never handled);
  - ADR 0007 §9.5 (the write journal and the order of writes), ADR 0013 (ratings, reject),
    ADR 0014 §2.7 (one key on the marked images, otherwise the current one);
  - AGENTS.md §3 (a new `windows-sys` feature needs the owner's approval, given in B-Q11 and
    B-Q14).

## 1. Context

- RawStride can rate, label, caption and (after U9b) move images, but it cannot remove one. A
  culling session ends with images that should go, and today the user must leave RawStride and
  find the files and their companions in the file manager.
- U9b already answers the hard parts for moving: which files belong to an image
  (`relocate::companions`, T169), that a pending rating or metadata write is finished before the
  file is touched and the view cores release it (the metadata worker's order and
  `ImageCheck::release`, T170), and how every tab of the folder and the active view follow when
  images leave the list (`replace_files`, `tabs_of_folder_mut`, T172–T174). Deleting is the same
  path with the Recycle Bin as the destination.
- RawStride never removes a file permanently: everything deleted goes to the Windows Recycle Bin,
  where the user can restore it to its old place. A memory card is never touched (ADR 0005
  choice 3).

## 2. Decision

### 2.1 Which images

- **Delete** in the contact sheet and in the preview, and the item **"Delete to Recycle Bin"** in
  the Image menu, without a modifier (B-Q1).
- In the contact sheet: the marked images, otherwise the current one (ADR 0014 §2.7). In the
  preview: only the current image, even when several are marked (B-Q2).
- **"Delete Rejected Images"** (B-Q9): every image of the active tab's folder whose rating is
  reject, with the same confirmation and rules; a step of its own (B5).
  A rejected image whose RAW+JPEG partner is not rejected is refused and named in the
  confirmation (owner, 2026-11-17, B-Q16).

### 2.2 Companion files

- The same rule as a move (ADR 0017 §2.6, R2; T169's `companions`): the image, its RAW+JPEG
  partner, both XMP sidecar forms and the edit files named by the companion suffix list
  (`dop`, `pp3`, `on1` by default, `settings.txt`, U9-Q10). Names compared without regard to case.
- Deleting only one file of a RAW+JPEG pair is not offered now (B-Q3).
- Every image file of the same name goes along (a TIFF or DNG too, not only the RAW+JPEG
  partner), as for a move; the confirmation names those besides the chosen images (owner,
  2026-11-17, B-Q18, from the chain review of T185–T190). Only image files the user did not
  mark are named: a marked JPG that goes with its marked RAW is not named again (owner,
  2026-11-18, during T187).

### 2.3 Order

- As a move (T170): the delete is a request to the metadata worker, so a rating or metadata write
  of the image sent earlier is finished first, and an image with a rating write still waiting in
  the shell's queue is sent only after that write has left it (T172's `to_send`). On the worker,
  the view cores' sessions of the image and its partners are released, then the files go to the
  Recycle Bin. Nothing is done on the UI thread.
- Windows shows no dialogs of its own: RawStride's confirmation (§2.5) is the only question, and
  errors come back as text.

### 2.4 All or nothing per image

- Before anything of an image is deleted, each of its files is checked: it exists, it is not open
  in another program (`fileops::is_locked`), and its drive is allowed (§2.6). If any check fails,
  none of the image's files is deleted and the reason names the file (B-Q8).
- If Windows still fails in the middle of an image's files, the files already in the Recycle Bin
  cannot be taken back by RawStride; the message names the files that were left. Accepted
  (B-Q8).

### 2.5 Confirmation

- Always asked (B-Q4): the number of images, the number of files and their total size, and that
  the files go to the Recycle Bin and can be restored from there. Buttons "Delete" and "Cancel";
  Cancel is the default, so Enter does not delete by mistake.
- No setting to skip the confirmation in this ADR (B-Q4).

### 2.6 Where nothing is deleted

- **A memory card** (B-Q5, ADR 0005 choice 3), also when the card is opened as a folder: a
  removable drive, or a drive with a `DCIM` folder at its root (as the import's card list,
  `pw_ingest::volumes`; this also covers a card in a reader that Windows sees as a fixed disk,
  T156). A fixed drive on USB without `DCIM`, for example an external archive disk, is allowed
  (owner, 2026-11-17). The keys and menu item refuse with a message naming the reason; nothing is
  sent.
- **A drive without a Recycle Bin** (B-Q6), for example a network share: refused with a message,
  since Windows would remove the files permanently. How RawStride knows this before deleting is
  tested in B1 [T].
- **Where Windows would delete permanently** (B-Q14, owner, 2026-11-17): with `FOF_NO_UI`
  Windows deletes a file permanently, without asking, when the Recycle Bin is turned off for the
  drive ("Don't move files to the Recycle Bin", or the policy `NoRecycleFiles`) or the file is too
  large for it. RawStride reads the drive's Recycle Bin settings first (the volume's
  `NukeOnDelete` and `MaxCapacity` under the user's `Explorer\BitBucket\Volume\{GUID}` key, and
  `NoRecycleFiles` in the user's and the machine's Explorer policies) and refuses the image when
  the bin is off, when its files together are larger than the bin's size, or when the settings
  cannot be read. The keys were seen for every local fixed drive on the owner's machine
  (reviewer, 2026-11-17).
  Known limit: if the settings change between this check and Windows' call, Windows would
  delete permanently, since `FOF_NO_UI` asks nothing; the check right before the call is the only
  guard (reviewer, 2026-11-17). The folder is resolved with `canonicalize` before the drive
  check, so a junction or folder symlink is judged by the drive it points to (reviewer, chain
  review of T185–T190, 2026-11-17).
- RawStride never removes a file permanently, and never empties the Recycle Bin.

### 2.7 After the delete

- As after a move (B-Q10): the deleted images leave every tab of their folder; the current image
  becomes the one that followed (the last one if none followed); the compare closes when its held
  image was deleted (`follow_compare`); the view cores and thumbnails follow as after a reorder.
- The rating store and the metadata cache forget the deleted paths; the thumbnail cache is in
  memory and drops them with the tab's list.
- Failures are shown in the status line and go to the Tasks error list (ADR 0017 U5-Q5), as a
  move's do. On success: "{n} images moved to the Recycle Bin." (the exact texts in B3).
- **Known limits accepted at the backlog sort after B4** (owner, 2026-11-18):
  - *Quitting while a delete runs* waits only for the rating writes, so the images not yet sent
    to the worker are not deleted; each image is all or nothing, so nothing is lost and the files
    stay where they were.
  - *A delete refused at the worker* (a file open in another program, or the drive or the size
    refused after the confirmation) has already released the image's view sessions, as a failed
    move has; the image opens again. Seen and approved in the owner's hand check of B4.
  - The other known limits of B4 (a worker that never answers, what is shown during a delete,
    path corner cases, the metadata window during a delete, a network folder not yet checked by
    hand) are kept in `docs/tasks/backlog.md` with their reasons. A tab that follows changes on
    disk (for example files restored from the Recycle Bin, §2.8) is a wish of the owner's at the
    hand check of B4: a new ADR (0026) with questions to the owner, written after B5.
- **Known limits accepted at the backlog sort after B5** (owner, 2026-11-18; from T190's
  reviews, none seen in the hand check of B5):
  - *A partner un-rejected too late:* Delete Rejected Images checks an image and its partners
    before it sends the delete; a reject cleared after that still lets the partner go, since the
    rating write is queued behind the delete.
  - *The group gathered again:* the worker gathers an image's group again when it deletes, so a
    same-name image file that appears after the confirmation goes along without being named (for
    Delete too).
  - *A tab closed during the batch:* its remaining images are skipped without a word (nothing
    wrong is deleted; the count is lower).
  - The B4 limits kept in `docs/tasks/backlog.md` stay there (not seen at B4 or B5); a network
    folder is still not checked by hand. ADR 0026 takes up a tab that follows changes on disk
    together with two wishes of ADR 0017 §7.2 (owner, 2026-11-18).

### 2.8 Undo

- **No undo in RawStride** (B-Q7). Ctrl+Z stays the undo of moves (U9-Q8). Restoring a file from
  the Recycle Bin from inside RawStride would mean searching the Recycle Bin through the Windows
  Shell, which is fragile; the user restores in Windows instead, and the files return to their
  old paths. The confirmation and the success message say so.
- When restored files come back into an open folder, the tab shows them after it is loaded again
  (as any file added from outside).

### 2.9 How the files go to the Recycle Bin

- `SHFileOperationW` with `FO_DELETE` and `FOF_ALLOWUNDO | FOF_NO_UI` (no dialogs, no
  confirmation of its own), all files of one image in one call, from the metadata worker's
  thread; no COM is needed (B-Q11) [T, confirmed in B1].
- This adds the feature `Win32_UI_Shell` to `pw-metadata`'s existing `windows-sys` =0.61.2; no new
  package (owner, 2026-11-17, B-Q11), and `Win32_System_Registry` for the Recycle Bin's settings
  (owner, 2026-11-17, B-Q14). Off Windows the function returns `Unsupported`, as the safe write's
  file operations do.
- **Tests** (B-Q15, owner, 2026-11-17): the tests that send files to the real Recycle Bin are
  `#[ignore]` and run by hand in the review with a named command, so `.\tools\check.ps1` puts
  nothing in the user's Recycle Bin; it runs the decision and the checks.

### 2.10 How the steps fit (reviewer, 2026-11-17, when B2–B5 were written as T186–T190)

- **Two requests to the metadata worker** (B2): `prepare_delete` finds each image's companion
  files and checks them without the "open in another program" check (the view cores still hold
  the current image), for the confirmation's count and size; `delete_image` releases the
  sessions, checks every file again with that check and deletes. A card or a drive without a
  Recycle Bin is refused in the prepare, so no window opens; the request runs on the worker, not
  on the UI thread (§2.3). The answers carry the structured `DeleteError`.
- **One file operation at a time:** Delete while a move or an undo runs or waits says so and does
  nothing; while a delete prepares, asks or runs, a waiting move batch is not started and Ctrl+Z
  says so. X still sets the rating at once.
- **The undo list of moves forgets deleted files:** a moved image whose file was deleted (for
  example in `_rejected`) is dropped from its undo step, so Ctrl+Z does not try to move it back.

## 3. Questions (answered by the owner, 2026-11-17: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| B-Q1 | Key and menu item | Delete in the contact sheet and the preview, no modifier; "Delete to Recycle Bin" in the Image menu |
| B-Q2 | Which images | Contact sheet: the marked, otherwise the current one (ADR 0014 §2.7); preview: only the current image |
| B-Q3 | Companion files | The rule of a move (ADR 0017 §2.6, T169), RAW+JPEG partner included; deleting only one file of a pair is not offered now |
| B-Q4 | Confirmation | Always: images, files, total size; "Delete" / "Cancel", Cancel the default; no setting to skip it for now |
| B-Q5 | Memory cards | Refused with a message on a removable drive or a drive with `DCIM` at its root (ADR 0005 choice 3). **Changed by the owner the same day:** not every fixed drive on USB, so an external archive disk can delete |
| B-Q6 | Drives without a Recycle Bin | Refused with a message; RawStride never deletes permanently. How it is detected is tested in B1 |
| B-Q7 | Undo | None in RawStride; the user restores from the Recycle Bin, which the confirmation and the message say |
| B-Q8 | All or nothing | Every file of an image checked first, nothing of it deleted when a check fails; a failure inside Windows' own call cannot be rolled back and is reported with the files left |
| B-Q9 | Delete every rejected image of the folder | Yes, "Delete Rejected Images", a step of its own (B5) after B4 |
| B-Q10 | After the delete | The images leave every tab of the folder, the current image becomes the following one, the compare closes if its image went, the caches forget the paths; an image with a queued rating write is deleted after it |
| B-Q11 | Windows API and dependency | `SHFileOperationW` with `FOF_ALLOWUNDO \| FOF_NO_UI`; the feature `Win32_UI_Shell` of the existing `windows-sys` in `pw-metadata` |
| B-Q12 | Names | Steps B1–B5, questions B-Q1… |
| B-Q13 | Not included | As §5 |
| B-Q14 | Recycle Bin off for the drive, or a file too large for it (asked when B1 was written, 2026-11-17) | Refused before Windows is called, from the drive's Recycle Bin settings in the registry (§2.6); also refused when they cannot be read. Adds the feature `Win32_System_Registry` (§2.9) |
| B-Q15 | Tests that use the real Recycle Bin (asked when B1 was written, 2026-11-17) | `#[ignore]`, run by hand in the review; `.\tools\check.ps1` does not put files in the user's Recycle Bin |
| B-Q16 | "Delete Rejected Images" and a partner that is not rejected (asked when B5 was written, 2026-11-17) | A rejected image whose RAW+JPEG partner is not rejected is refused, and the confirmation names it; deleting it alone would break B-Q3 |
| B-Q17 | A structured error for the move's answer too (plan row B2, 2026-11-17) | No, not now: only the delete's answer gets one; the move's stays text (the backlog item "Leftovers from the review of T170" keeps it) |
| B-Q18 | Image files of the same name besides the RAW+JPEG partner (chain review, 2026-11-17) | They go along, as for a move, and the confirmation names them; only those the user did not mark (owner, 2026-11-18, T187) |

## 4. Steps

Each step ends with `.\tools\check.ps1`; the owner's hand check comes after B4 and again after B5.
B1 starts after T174 is done.

| Step | Content |
|---|---|
| **B1** | The file part in `pw-metadata` beside `relocate.rs`: refuse a card drive or a drive without a Recycle Bin, check every file of a group, send the group to the Recycle Bin (§2.4, §2.6, §2.9); the `windows-sys` feature. The detection of a drive without a Recycle Bin and the absence of Windows dialogs are tested here [T] |
| **B2** | The delete as a request to the metadata worker, with its answer event, after earlier writes and with the sessions released (§2.3; as T170) |
| **B3** | The shell's plain-Rust part: the targets (§2.1), the batch sent behind queued rating writes, the tabs' lists after a delete, the confirmation's content, the texts (rule 13) and the glossary |
| **B4** | Keys, menu item, the confirmation window and the wiring in `main.rs`; independent code review; the owner's hand check on a copy of R2's folder (an ARW with `.ARW.dop` and `.XMP` goes to the Recycle Bin and returns to its place from there; a card and a network folder are refused; a file open in another program stops its image) |
| **B5** | "Delete Rejected Images" in the active tab's folder (B-Q9) |

## 5. Not included

- Deleting from a memory card (ADR 0005 choice 3) and any permanent delete.
- A Recycle Bin of RawStride's own, emptying the Recycle Bin, and undo inside RawStride (§2.8).
- Deleting one file of a RAW+JPEG pair alone; a setting to skip the confirmation.
- Videos and audio notes (ADR 0005 choice 5; they are not in the tabs' lists).
- Folders: only image files and their companions are deleted, never a folder (an emptied
  `_rejected` stays).

## 6. Consequences

- A session can end inside RawStride, with the companion files gone together with the image, so
  no other program is left with edit files for an image that no longer exists.
- Nothing is lost for good: every delete can be restored in Windows.
- `pw-metadata` gets one more `windows-sys` feature; the delete path depends on T169–T174 being
  built first.
