# ADR 0018: Deleting images to the Recycle Bin

- Status: **proposed.** The owner answered every question on 2026-11-17 (all proposals approved,
  §3). Nothing is built, installed or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit.
- Date: 2026-11-17
- Related:
  - ADR 0017 §2.6 (companion files, R2) and step U9b with its tasks T169–T174 (moving an image
    with its companion files, the move as a request to the metadata worker, the tabs' lists after
    a move); this ADR reuses those parts and is built after T174;
  - ADR 0005 choice 3 (never delete from the card) and choice 5 (videos are never handled);
  - ADR 0007 §9.5 (the write journal and the order of writes), ADR 0013 (ratings, reject),
    ADR 0014 §2.7 (one key on the marked images, otherwise the current one);
  - AGENTS.md §3 (a new `windows-sys` feature needs the owner's approval, given in B-Q11).

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

### 2.2 Companion files

- The same rule as a move (ADR 0017 §2.6, R2; T169's `companions`): the image, its RAW+JPEG
  partner, both XMP sidecar forms and the edit files named by the companion suffix list
  (`dop`, `pp3`, `on1` by default, `settings.txt`, U9-Q10). Names compared without regard to case.
- Deleting only one file of a RAW+JPEG pair is not offered now (B-Q3).

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
- RawStride never removes a file permanently, and never empties the Recycle Bin.

### 2.7 After the delete

- As after a move (B-Q10): the deleted images leave every tab of their folder; the current image
  becomes the one that followed (the last one if none followed); the compare closes when its held
  image was deleted (`follow_compare`); the view cores and thumbnails follow as after a reorder.
- The rating store and the metadata cache forget the deleted paths; the thumbnail cache is in
  memory and drops them with the tab's list.
- Failures are shown in the status line and go to the Tasks error list (ADR 0017 U5-Q5), as a
  move's do. On success: "{n} images moved to the Recycle Bin." (the exact texts in B3).

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
  package (owner, 2026-11-17, B-Q11). Off Windows the function returns `Unsupported`, as the safe
  write's file operations do.

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
