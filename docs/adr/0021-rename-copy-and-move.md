# ADR 0021: Renaming, copying and moving images

- Status: **proposed.** The owner answered every question on 2026-11-17 (all proposals approved,
  §3). Nothing is built, installed or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit.
- Date: 2026-11-17
- Related:
  - ADR 0017 §2.6 and U9b with T169–T174 (companion files, the move as a request to the metadata
    worker, the undo list, the tabs after a move); §7.2's wish "Copy, move and rename tools for
    selected images", which this ADR takes up;
  - ADR 0005 (the import's templates, shots and verified copy; choice 3, never change a card);
  - ADR 0007 §9 (the safe write and its journal, recovered at start);
  - ADR 0018 (the Recycle Bin, the card rule), ADR 0020 (the corrected capture time);
  - ADR 0017 U4 (favourites and recent folders).

## 1. Context

- After an import the images keep the names they were given then. A job often needs other names
  later (a client's numbering, a name after the culling), or the chosen images copied or moved to
  a delivery or archive folder. Today that is done outside RawStride, and the companion files are
  easily left behind.
- What exists: the import's template syntax and its Windows-safe names (`pw-ingest`'s
  `Template`, ADR 0005 point 4); the import's verified copy (`pw_ingest::copy_verified`); T169's
  companion files and all-or-nothing moves within one drive (`fileops::move_new` uses
  `MoveFileExW` without `MOVEFILE_COPY_ALLOWED`, so it fails across drives); T170–T174's order,
  sessions, tabs and undo.
- Renaming differs from moving in one way: new names can be names that other images of the same
  rename have now, so files must pass through temporary names, and a crash between the two steps
  must be repaired.

## 2. Decision

One ADR in two parts, renaming (§2.2–§2.4) and copying or moving (§2.5), with shared rules
(N-Q1).

### 2.1 Which images, and shots

- The marked images, otherwise the current one; in the preview only the current image (N-Q2, as
  ADR 0018 and 0019).
- Each image is handled as a **shot**: the image, its RAW+JPEG partner, its XMP sidecars and the
  edit files of the companion suffix list (T169's `companions`). A shot gets one new base name,
  and all its files follow it: `A.ARW`, `A.ARW.dop`, `A.XMP` become `B.ARW`, `B.ARW.dop`, `B.XMP`
  (N-Q3). A partner that is itself marked is the same shot, not a second one.
- **The files' contents are never changed**, even when a companion file names the old file
  inside. Whether the owner's RAW editor still finds its edits after a rename is checked on the
  owner's real files before N1 (level C, as R2) (N-Q4).

### 2.2 Renaming

- **One image:** a name field holding the current base name; the extension cannot be changed.
  **F2** and "Rename…" in the Image menu (N-Q5, N-Q17).
- **Several images:** a name template in the import's syntax and variables (N-Q6):
  - the time variables come from the capture time, the corrected one where there is one
    (ADR 0020);
  - `{name}` is the current base name; `{job}` a text field in the window;
  - `{seq}` and `{seq:N}` count in the tab's shown order, from a start value in the window;
  - no new variables now.
- The window remembers the last template (`settings.txt`).

### 2.3 The plan before anything happens

- The window lists old → new name for every shot, computed without touching a file (as the
  import's plan, ADR 0005 point 1). Collisions are marked in red, and "Rename" is greyed while
  there is one (N-Q7).
- **A collision with a file outside the rename** is never solved silently (no `_1` as at
  import): the plan shows it and the user changes the template (N-Q8). Two shots given the same
  new name are a collision too.

### 2.4 Doing the rename

- **Names that other shots of the rename hold now** (A→B and B→A, or a renumbering where 0003
  becomes 0002 while 0002 is renamed too): every file is first renamed to a temporary name in its
  own folder, then to its final name (N-Q9).
- **A rename journal** beside the write journal (`LOCALAPPDATA`, ADR 0007 §9.5) lists every file's
  old, temporary and new name before the first file is touched, and is removed when the rename is
  done. After a crash, the next start finishes or rolls back the rename from the journal, as the
  write journal is recovered today (N-Q9).
- **All or nothing** (N-Q10): if any step fails, every shot already renamed is renamed back, and
  the reason names the file. A rollback that fails itself names the files left, as T169's
  `MoveError` does.

### 2.5 Copying and moving to a folder

- **"Copy To…"** and **"Move To…"** in the Image menu, without keys (N-Q17). The target folder is
  chosen with the folder dialog; the window also offers the favourites and recent folders (U4).
  A name template can be used at the same time, with the same plan and collision rules as §2.3
  (N-Q11).
- **Copy:** each file by the import's verified copy (`copy_verified`: a `.pwpart` file, flushed,
  read back and compared, then renamed); the source is not touched.
- **Move on the same drive:** T169's `move_group` (all or nothing per shot).
- **Move to another drive** (N-Q12): first the verified copy; when every file of the shot is
  verified, the source files go to the Recycle Bin (ADR 0018), never removed permanently. On a
  drive without a Recycle Bin the move is refused and copying is offered.
- **Memory cards** (N-Q13): copying from a card is allowed (nothing on the card changes); renaming
  on a card or moving from it is refused, with the card rule of ADR 0018 §2.6 (a removable drive or
  a drive with `DCIM` at its root).

### 2.6 Order

- As a move (T170, N-Q14): each rename, copy or move is a request to the metadata worker, run after
  the writes already sent; an image with a rating write still waiting in the shell's queue is sent
  after it; the view cores release the files first.

### 2.7 Afterwards

- **Rename** (N-Q15): the images stay in every tab of their folder under their new names, in the
  same places until the next sort; the marking, the current image, the ratings, the metadata cache
  and the thumbnails follow to the new paths.
- **Move:** the images leave the tabs of their folder, as after T174.
- **Copy, and the target of a move:** tabs showing the target folder are listed again.
- Failures go to the status line and the Tasks error list, as a move's do.

### 2.8 Undo

- **Ctrl+Z** undoes a rename and a move on the same drive, in the undo list of U9b (20 steps, the
  session only) (N-Q16).
- A copy is not undone (the copies can be deleted with ADR 0018), and neither is a move to another
  drive (the originals are in the Recycle Bin).

## 3. Questions (answered by the owner, 2026-11-17: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| N-Q1 | One ADR or two | One ADR in two parts, each with its own steps |
| N-Q2 | Which images | The marked, otherwise the current one; preview: only the current image |
| N-Q3 | Shots | Partner, sidecars and edit files get the same new base name and follow at copy and move (T169's rule) |
| N-Q4 | File contents | Never rewritten; the owner's RAW editor checked on real files before N1 |
| N-Q5 | One image | A name field with the current base name; the extension fixed; F2 |
| N-Q6 | Several images | The import's template syntax; the corrected capture time; `{name}` the current name; `{job}` a field; `{seq}` from a start value in the tab's shown order; no new variables |
| N-Q7 | The plan | Old → new for every shot, collisions in red, "Rename" greyed while there is one; the last template remembered |
| N-Q8 | Collision outside the rename | No silent `_1`; the user changes the template |
| N-Q9 | Names swapping inside the rename | Temporary names first, then final; a rename journal beside the write journal, repaired at the next start |
| N-Q10 | All or nothing | Every shot already renamed is renamed back when a step fails |
| N-Q11 | Copy To / Move To | The folder dialog plus favourites and recent folders; an optional name template with the same plan |
| N-Q12 | Move to another drive | Verified copy, then the sources to the Recycle Bin (ADR 0018); refused without a Recycle Bin, copying offered |
| N-Q13 | Memory cards | Copy from a card allowed; rename on or move from a card refused (ADR 0018's card rule) |
| N-Q14 | Order and sessions | As a move (T170) |
| N-Q15 | Afterwards | Rename: the images stay, marking, ratings, caches and thumbnails follow; move: they leave the tab; copy: target tabs listed again |
| N-Q16 | Undo | Ctrl+Z for rename and same-drive move, U9b's list; no undo for copy or a move across drives |
| N-Q17 | Menu | "Rename…" (F2), "Copy To…", "Move To…" in the Image menu |
| N-Q18 | Steps | N1–N5 as §4, after T174 |
| N-Q19 | Not included | As §5 |

## 4. Steps

Each step ends with `.\tools\check.ps1`. The steps start after T174; the check of the owner's RAW
editor after a rename (N-Q4) comes before N1.

| Step | Content |
|---|---|
| **N1** | The rename plan in plain Rust: shots from the chosen images, the template per shot (from `pw-ingest`'s `Template`, the capture time by ADR 0020), collisions inside and outside the rename, the temporary names |
| **N2** | The rename in `pw-metadata`: the journal, the two steps, the rollback, the recovery at start; the request to the metadata worker |
| **N3** | The shell for renaming: the window (one name or a template, the plan), F2 and the menu item, the tabs and caches following the new paths, the undo; texts and glossary; independent code review; the owner's hand check |
| **N4** | Copy and move in `pw-metadata` and `pw-ingest`: the verified copy of a shot, the move on one drive and across drives (copy, then the Recycle Bin), the plan with a template and its collisions, the card rule |
| **N5** | The shell for copying and moving: the window with the folder choice, the menu items, the tabs of the source and target folders, the undo of a same-drive move; texts and glossary; the owner's hand check |

## 5. Not included

- Changing a file's extension.
- Rewriting file names inside companion files (N-Q4).
- Find and replace in file names.
- New template variables (rating, label, IPTC fields).
- Drag and drop between folders.
- Copying to several targets at once.
- Videos (ADR 0005 choice 5).

## 6. Consequences

- Images can be renamed, delivered and archived from RawStride with their companion files, so no
  other program's edits are left behind.
- A crash during a rename is repaired at the next start; nothing is ever overwritten or removed
  permanently.
- The rename journal is a second journal beside the write journal, with its own recovery at start.
