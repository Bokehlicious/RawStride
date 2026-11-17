# ADR 0020: Correcting the capture time

- Status: **proposed.** The owner answered every question on 2026-11-17 (all proposals approved,
  §3). Nothing is built, installed or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit.
- Date: 2026-11-17
- Related:
  - ADR 0007 §4 (storage per file type), §6.3 (writing Exif), §7 (date, time and time zone) and
    Q12, **amended by this ADR** (§2.8);
  - ADR 0007 §9.5 (the safe write and batches), ADR 0017 U12c (the batch confirmation and summary)
    and U14 (sorting by capture time);
  - ADR 0017 §7.2: the GPS ADR, which will match a GPX track against the corrected capture times.

## 1. Context

- A camera's clock is often wrong: never set, not moved to summer time, or left on the home time
  zone after a journey. With two cameras on one job, their images also sort in the wrong order
  against each other.
- ADR 0007 §7 and Q12 left a manual correction out of scope, with one rule for when it came: it
  would be the only way DateTimeOriginal ever changes, and only on the user's explicit command.
  This ADR is that command.
- A manufacturer RAW file is never changed (ADR 0007 §4), so for RAW the corrected time can live
  only in the sidecar. `pw_raw::inspect` reads the make, model and DateTimeOriginal but not the
  camera's serial number.

## 2. Decision

### 2.1 The correction

- One window, **"Adjust Capture Time…"** in the Image menu, without a key (C-Q13), gives the
  correction in one of two ways (C-Q1):
  - **a shift**: days, hours, minutes and seconds, forwards or backwards;
  - **a known time**: "this image was taken at …" for the current image, and the window computes
    the shift (for example from a photo of a clock).
- The same shift is added to every chosen image. Sub-seconds are kept as they are.

### 2.2 Which images

- The marked images, otherwise the current one (C-Q2; ADR 0014 §2.7).
- The window lists the cameras among them with each one's number of images, each with a check box;
  only the ticked cameras' images are shifted (C-Q2).
- **A camera** is its make, model and body serial number; when an image has no serial number, make
  and model (C-Q3). Reading the serial number is a small addition to `pw-raw` (C1).
- An image without a capture time is left out and counted in the summary.

### 2.3 What is written

- **Manufacturer RAW** (ARW, RW2, …): the sidecar gets `exif:DateTimeOriginal` with the corrected
  time (with `exif:SubSecTimeOriginal` and the time zone when they exist). The embedded Exif is
  never changed (C-Q4; ADR 0007 §4).
- **JPEG, TIFF and DNG**: Exif DateTimeOriginal and DateTimeDigitized (CreateDate) are written in
  the file by the safe write with ExifTool, and the XMP forms are kept in step (C-Q5).
- **Never changed**: ModifyDate, the GPS time stamp, sub-seconds (C-Q5).

### 2.4 Time zone

- An optional time-zone field in the window (for example `+02:00`), written as
  OffsetTimeOriginal (in XMP: the time with its offset). Empty: no zone is written, as ADR 0007 §7
  requires (no zone invented). The zone can also be set alone, without shifting the time (C-Q6).

### 2.5 Date Created

- IPTC Date Created (`photoshop:DateCreated`) is shifted by the same amount when it was equal to
  the old capture time. Otherwise the user wrote it, and it is left as it is and named in the
  summary (C-Q7).

### 2.6 How RawStride reads the corrected time

- When the XMP (sidecar or embedded) has `exif:DateTimeOriginal`, it is the capture time for Info,
  sorting (U14) and every other use, before the embedded Exif (C-Q8; XMP wins, ADR 0007 rule 4).
- For a manufacturer RAW, a difference between that XMP value and the embedded DateTimeOriginal is
  **not a conflict**: it is a correction. Info shows the time with "corrected", and the original
  time in the tooltip.

### 2.7 Safety

- **Before writing** (C-Q10), the window lists file, old time and new time for the first images,
  and "{n} images, {k} files changed"; for JPEG, TIFF and DNG it says that the file itself is
  changed. Buttons "Apply" and "Cancel" (Cancel the default).
- The write is a batch under ADR 0007 §9.5, each file its own safe write, with the summary of
  ADR 0017 U12c: written, failed with reasons, skipped (no capture time, Date Created left as it
  was).
- **The original time is not kept** in a field of its own (C-Q9): for RAW it stays in the file;
  for JPEG, TIFF and DNG it is overwritten. A correction is undone by shifting back by the same
  amount. Keeping the original in a RawStride namespace waits for the namespace decision (ADR 0007
  D1).

### 2.8 Changes to ADR 0007 (written there with this ADR, 2026-11-17)

- §6.3: "DateTimeOriginal is never written" gets the exception of this ADR.
- §7 and Q12: the manual correction is no longer out of scope; it is this ADR, and the rule that it
  is the only way DateTimeOriginal changes, and only on the user's explicit command, stands.
- Rule 4 and §6.4: a sidecar `exif:DateTimeOriginal` that differs from a RAW's embedded value is a
  correction, not a conflict (§2.6).

## 3. Questions (answered by the owner, 2026-11-17: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| C-Q1 | How the correction is given | Both a shift (days, hours, minutes, seconds, ±) and a known time for the current image, in one window |
| C-Q2 | Which images | The marked, otherwise the current one; the window lists the cameras with counts, a check box each |
| C-Q3 | What a camera is | Make, model and body serial number; make and model when there is no serial number; `pw-raw` reads the serial number (C1) |
| C-Q4 | Manufacturer RAW | `exif:DateTimeOriginal` (with sub-seconds and zone when they exist) in the sidecar; the embedded Exif never changed |
| C-Q5 | JPEG, TIFF, DNG | Exif DateTimeOriginal and DateTimeDigitized in the file by the safe write, XMP kept in step; ModifyDate, GPS time and sub-seconds never changed |
| C-Q6 | Time zone | An optional field written as OffsetTimeOriginal; empty writes no zone; the zone can be set alone |
| C-Q7 | Date Created | Shifted when it equalled the old capture time, otherwise left and named in the summary |
| C-Q8 | Reading | XMP `exif:DateTimeOriginal` before the embedded value everywhere; for RAW not a conflict; Info shows "corrected" with the original in the tooltip |
| C-Q9 | The original time | Not kept in a field of its own now; undone by shifting back; the confirmation says JPEG, TIFF and DNG are changed in the file |
| C-Q10 | Before writing | A list of file, old and new time, a count; "Apply" / "Cancel"; a batch under ADR 0007 §9.5 with U12c's summary |
| C-Q11 | At import | Not in this ADR (§5) |
| C-Q12 | GPS | The GPS ADR uses the corrected time; no GPX matching here |
| C-Q13 | The command | "Adjust Capture Time…" in the Image menu, no key |
| C-Q14 | Steps | C1–C5 as §4 |
| C-Q15 | Not included | As §5 |

## 4. Steps

Each step ends with `.\tools\check.ps1`; the owner's hand check comes after C5.

| Step | Content |
|---|---|
| **C1** | `pw-raw` reads the body serial number (Exif BodySerialNumber, where it exists), with tests on the test files |
| **C2** | Reading: the capture time from XMP `exif:DateTimeOriginal` first (Info, sorting); for RAW no conflict against the embedded value; "corrected" in Info (§2.6) |
| **C3** | Writing in `pw-metadata`: the sidecar for RAW, the file with ExifTool for JPEG, TIFF and DNG, Date Created by §2.5, the time zone by §2.4; through the safe write |
| **C4** | Plain Rust in the shell: the shift from a known time, the images grouped by camera, the new times, the preview list's content, the texts (rule 13) and the glossary |
| **C5** | The window, the menu item and the batch with its summary; independent code review; the owner's hand check (two cameras in one folder, one shifted; a RAW and a JPEG; a time zone; Date Created written by hand left alone; sorted by capture time afterwards; another program reading the corrected time from the sidecar) |

## 5. Not included

- A shift per camera in the import dialog, so the import's folder and file templates use the
  corrected time (C-Q11); a later ADR (GPS or import).
- Matching a GPX track (the GPS ADR).
- Setting a time on an image that has none.
- ModifyDate and the GPS time stamp.
- Keeping the original time in a namespace of RawStride's own.
- Videos (ADR 0005 choice 5).

## 6. Consequences

- Images from several cameras sort in true order, and the GPS ADR gets correct times to match.
- DateTimeOriginal changes for the first time in RawStride, only on this command; JPEG, TIFF and
  DNG files are changed, RAW files never.
- RawStride reads the capture time from XMP before Exif, so other programs that read the sidecar
  see the same time.
