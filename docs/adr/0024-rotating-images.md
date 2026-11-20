# ADR 0024: Rotating images

- Status: **accepted by the owner (2026-11-17):** every question answered, all proposals approved
  (§3). Nothing is built, installed or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit.
- Date: 2026-11-17
- Related:
  - ADR 0007 §4 (storage per file type), §6.3 (Exif/TIFF tags not duplicated into XMP) and rule 4
    (XMP wins), **amended by this ADR** (§2.6); §9 (the safe write);
  - ADR 0013 (rating keys: shown at once, written in order, rolled back on failure), ADR 0014
    point 6 (the targets of a key);
  - ADR 0003 point 4 and ADR 0008 §14.3 (the orientation tag and the rotation for display);
    `pw-viewcore`'s `flip_of_orientation`;
  - ADR 0020 (a sidecar value before the embedded one, not a conflict).

## 1. Context

- A camera sometimes records the wrong orientation (pointing up or down, or a tilted body), and
  some images have none. RawStride shows each image by its embedded Exif orientation and cannot
  turn one.
- The view core and the thumbnails take the orientation from `pw_raw::inspect`, which reads only
  the embedded tag; mirrored values (2, 4, 5, 7) are shown unmirrored. Nothing reads an
  orientation from XMP.
- A manufacturer RAW file is never changed (ADR 0007 §4), so its rotation can live only in the
  sidecar, as `tiff:Orientation`. ADR 0007 §6.3 does not duplicate Exif/TIFF tags into XMP; this
  ADR adds the exception.

## 2. Decision

### 2.1 Commands and keys

- **Ctrl+←** turns 90° anticlockwise and **Ctrl+→** 90° clockwise, in the contact sheet and the
  preview; "Rotate Left" and "Rotate Right" in the Image menu. 180° is two presses (O-Q1).
- The targets are those of a rating key: in the contact sheet the marked images, otherwise the
  current one; in the preview the current image (O-Q2; ADR 0014 point 6).

### 2.2 What is written

- **Manufacturer RAW:** `tiff:Orientation` in the sidecar; the original is never changed (O-Q3).
- **JPEG, TIFF and DNG:** the Exif tag IFD0 Orientation in the file by the safe write, metadata
  only; `tiff:Orientation` in the file's XMP is kept in step when it exists. The pixels are never
  turned; there is no lossless JPEG transform (O-Q4).

### 2.3 Mirrored orientations

- A turn is computed by the Exif orientation values, so a mirrored value stays mirrored (for
  example 2 turned clockwise gives 7). The display still shows mirrored images unmirrored, as
  today (O-Q5).

### 2.4 How RawStride reads the rotation

- An XMP `tiff:Orientation` (sidecar or embedded) is used before the embedded Exif tag, by the
  view core, the thumbnails and Info (O-Q6; XMP wins, ADR 0007 rule 4).
- It is read in the folder's batch call with the ratings (`XMP-tiff:Orientation` beside
  `XMP-xmp:Rating` and `XMP-xmp:Label`) and given to the view core and the thumbnails. Until it is
  read, the embedded orientation is used, and the image turns when the value arrives. How the view
  core takes an orientation per file is tried in O1 [T].
- For a RAW, a sidecar orientation that differs from the embedded tag is a rotation, **not a
  conflict**; Info shows nothing special (O-Q7).

### 2.5 Display and writing

- **As a rating key** (O-Q8, ADR 0013): the image turns at once in the view and its thumbnail; the
  write goes into the same ordered queue as the ratings; a failed write turns it back and shows the
  reason.
- **Zoom** (O-Q9): fit stays fit; at 100 % the same point of the image stays in the centre.
- **Undo** is a turn the other way; Ctrl+Z stays the undo of moves (O-Q10).
- **Memory cards**: the same rule as a rating on a card today; no new rule (O-Q11).

### 2.6 Changes to ADR 0007 (written there with this ADR, 2026-11-17)

- §6.3: Exif/TIFF tags are still not duplicated into XMP, except `tiff:Orientation`, written by
  a rotation of this ADR (in a RAW's sidecar, and kept in step in a file's XMP).
- Rule 4 and §6.4: a sidecar `tiff:Orientation` that differs from a RAW's embedded value is a
  rotation, not a conflict.

### 2.7 Other programs

- Before O1 the reviewer checks on the owner's real files whether the owner's RAW editor reads
  `tiff:Orientation` from the sidecar (level C, as R2). If it does not, the rotation is written
  all the same and the finding is recorded here (O-Q12).
- **Checked by the owner (2026-11-20): DxO PhotoLab reads it.** Copies of a Sony ARW, a
  Panasonic RW2 and a Fujifilm RAF, each with embedded orientation 1, were given a sidecar holding
  only `tiff:Orientation` (6, 8 and 3), in a new folder with no `.dop` files. DxO PhotoLab showed
  all three turned as the sidecar says (90° clockwise, 90° anticlockwise, 180°). The folder held
  no new or changed files afterwards.

## 3. Questions (answered by the owner, 2026-11-17: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| O-Q1 | Keys and menu | Ctrl+← anticlockwise, Ctrl+→ clockwise, in both views; "Rotate Left" / "Rotate Right" in the Image menu; 180° by two presses |
| O-Q2 | Which images | The targets of a rating key (ADR 0014 point 6) |
| O-Q3 | RAW | `tiff:Orientation` in the sidecar; the original never changed |
| O-Q4 | JPEG, TIFF, DNG | IFD0 Orientation in the file by the safe write, XMP kept in step; pixels never turned |
| O-Q5 | Mirrored | Turned by the Exif values, the mirror kept; still shown unmirrored |
| O-Q6 | Reading | XMP orientation before the embedded tag, read in the folder's batch call, given to the view and the thumbnails; the embedded tag until read |
| O-Q7 | Difference from the embedded tag | For RAW not a conflict; nothing special in Info |
| O-Q8 | Display and write | As a rating key: at once, queued, rolled back on failure |
| O-Q9 | Zoom | Fit stays fit; at 100 % the same point centred |
| O-Q10 | Undo | Turn the other way; Ctrl+Z stays for moves |
| O-Q11 | Memory cards | The rating rule of today |
| O-Q12 | Other programs | The owner's RAW editor checked on real files before O1; written all the same |
| O-Q13 | Steps | O1–O3 as §4 |
| O-Q14 | Not included | As §5 |

## 4. Steps

Each step ends with `.\tools\check.ps1`; the owner's hand check comes after O3. The check of §2.7
comes before O1.

| Step | Content |
|---|---|
| **O1** | Reading: `XMP-tiff:Orientation` in the folder's batch call; the view core and the thumbnails take an orientation per file before the embedded one, and turn the image when it arrives; the turn of an Exif value (mirror kept), tested |
| **O2** | Writing in `pw-metadata`: the sidecar's `tiff:Orientation` for RAW, IFD0 Orientation and the XMP in step for JPEG, TIFF and DNG, by the safe write; for RAW not a conflict |
| **O3** | The keys and menu items, the turn shown at once in the view and the thumbnail, the write in the ratings' queue and the rollback, the zoom rule; texts and glossary; the owner's hand check (a RAW and a JPEG turned, several marked, a failed write, after a restart) |

## 5. Not included

- Turning the pixels (lossless JPEG transform).
- Free angles and straightening; cropping.
- Mirror commands, and showing mirrored images mirrored.
- Automatic rotation.

## 6. Consequences

- An image on its side can be put right in one key, without changing a RAW file or any pixels.
- The view and thumbnails read one more value per image in the folder's batch call.
- ADR 0007 gets a second XMP value read before the embedded one, after ADR 0020's capture time.
