# ADR 0029: Editor profiles: what an editing program leaves beside the images

- Status: **proposed (2026-11-19).** A draft for the owner. Nothing is placed in the step plan,
  and nothing is built, installed or added as a dependency by this ADR. When the questions are
  answered, each step in §5 becomes its own task, with its own approval and commit.
  - **Step P0 done (2026-11-19):** the owner's test with DxO PhotoLab 10.3, written into §1.1;
    P-Q3, P-Q6 and P-Q12 now carry what it showed, §2.8 is new, and two changes to the metadata
    writing went to ADR 0007 revision 26.
- Date: 2026-11-19
- Related:
  - ADR 0019 (opening images in another program): the program list (§2.1) that this ADR extends,
    and §2.7 (RawStride does not watch what the other program writes);
  - ADR 0017 §2.6, U9-Q10 and R2: the companion suffix list (`dop`, `pp3`, `on1` by default) and
    the file names DxO PhotoLab was seen to write (`<base>.<ext>.dop`);
  - ADR 0021 (renaming, copying and moving): edit files of the companion list follow their image;
  - ADR 0026 (a folder that follows changes on disk): when a new edit file or export is seen;
  - ADR 0007 §9 (the safe write) and ADR 0013 (ratings and labels in XMP), which the editing
    program is expected to read;
  - ADR 0030 (the catalog, to be written): written separately on the owner's decision
    (2026-11-19), since this ADR needs no catalog;
  - [clean-room.md](../clean-room.md): the functions are described from RawStride's own needs; the
    file names are interoperability facts read on the owner's own files.

## 1. Context

What exists today (code and ADRs read 2026-11-19):

- ADR 0019 E1–E3 are done and hand-checked: up to five programs with a name and a path, "Open in
  {name}" and Ctrl+E send the marked images to a program in one start (T200–T203).
- After that RawStride knows nothing about what the program did. An edited image looks the same in
  the contact sheet as one never touched, and an exported JPEG lies in the folder (or elsewhere)
  without any link to its original.
- The companion suffix list already moves and renames an image's edit file with it (U9b, ADR 0021),
  but nothing shows whether an edit file exists.

The owner's goal (2026-11-19): cull and rate in RawStride, edit in DxO PhotoLab, and see the result
back in RawStride, at least approximately: which images are edited, which are exported, the export
beside its original, and a warning when the export is older than the edit. The owner may change to
Capture One later, so the link must not be built for DxO PhotoLab alone.

### 1.1 What step P0 showed (2026-11-19)

The owner's test with DxO PhotoLab 10.3, on copies of the owner's ARW files and a Ricoh GR III DNG
in `D:\PhotoWorkflowData\P0-dxo`; every file was compared before and after each step (clean-room
level C: what the program writes).

| # | Question | Result |
|---|---|---|
| 1 | Images from different folders | Two "Open in" from two tabs in a row: the program receives the images of both |
| 2 | RawStride's rating and label | Read from the XMP sidecar the first time the program sees an image |
| 3 | RawStride's reject (`xmp:Rating` −1) | **Not read:** the image shows no rating at all. The program's own reject is **not written** to XMP either (only to its `.dop`), so a reject never crosses in either direction |
| 4 | Only viewing an image | Nothing written: no `.dop`, no XMP change |
| 5 | Export with the owner's preset | `{base}_DxO.jpg` in the image's own folder. The JPEG carries `xmpMM:PreservedFileName` = the original's file name, and the rating and label |
| 6 | `.dop` and export times | The `.dop` is written about 2.2 s **after** the export; a later edit makes it minutes newer |
| 7 | The program's setting "Synchronize metadata with XMP sidecar files" **off** | XMP read only the first time; later changes in RawStride reach the program only through its manual "read metadata from file" command; its "write metadata to disk" then wrote its old values over the file, and **a keyword and a title written by RawStride were lost** |
| 8 | The same setting **on** | The program writes rating, label, keywords and title to XMP by itself and keeps RawStride's fields, but reads a change only when the file's modification time is newer than its own `.dop`. RawStride set the old time back on every write (ADR 0007 §9.4), so its changes were ignored until the time was changed by hand |
| 9 | Keywords | The program also writes the keywords to `lr:hierarchicalSubject` and reads them back from there; RawStride left that property unchanged (ADR 0007 §8), so a keyword renamed in RawStride came back beside the new one |
| 10 | DNG | Same as 8 and 9. The program writes into the DNG itself (no sidecar), image data unchanged; it adds Exif Rating and RatingPercent, `xmp:RatingPercent`, `exif:DateTimeOriginal` in XMP, a Location Shown city copied from the city, and an Image Creator name copied from the creator |

What follows: the program's synchronisation setting must be on (§2.8); ADR 0007 revision 26 gives
written files a new modification time and keeps a flat `lr:hierarchicalSubject` and the rating
percentages equal to RawStride's values (decided by the owner, 2026-11-19).

## 2. Decision (proposed)

### 2.1 An editor profile

An **editor profile** is a short description, kept in the settings, of the files an editing program
leaves behind. It has three parts:

1. **The program**: one of the programs in the ADR 0019 list (P-Q2).
2. **The edit file rule**: where the program keeps its edit for an image, written as a name
   pattern relative to the image, for example `{file}.dop` for DxO PhotoLab (R2: `IMG_1.ARW` →
   `IMG_1.ARW.dop`).
3. **The export rule**: where and under which name the program's exported images land, for example
   `{base}_DxO.jpg` in the image's folder (the owner's preset, P0), or `{base}.jpg` in a
   subfolder (P-Q3). An export found by the pattern is confirmed by its `xmpMM:PreservedFileName`
   when it has one (§2.2).

Patterns use a few placeholders (`{file}` the image's full file name, `{base}` without the
extension, `{ext}` the extension) and may start with a relative subfolder (P-Q1). RawStride ships a
filled-in profile for DxO PhotoLab, adjusted to what P0 shows; other programs are described by
the user with the same patterns, with no code per program.

Several profiles may be active at once (one per program), so the edits made in one program stay
visible after a change to another.

### 2.2 Only names, never contents

RawStride finds edit files and exports **by name**, in the folder listing it already reads. It
never opens, parses or changes an edit file (P-Q10): their formats belong to their programs, and
nothing in them is needed. The time an edit file was last changed (its mtime) is the only thing
read about it. An export is an ordinary image: its `xmpMM:PreservedFileName`, read with the rest
of its metadata, confirms which original it belongs to, and an export whose value names another
file is not linked.

### 2.3 What the user sees

- **Marks in the contact sheet** (P-Q5): a small mark in the cell for "edited" (an edit file
  exists), "exported" (an export exists) and "export older than the edit" (§2.5). No mark without
  an active profile, so nothing changes for a user without one.
- **The inspector** names the profile, the edit file and the export, with their times.
- **Original ↔ export in the preview** (P-Q7): a key and a View menu item switch the preview
  between the original and its export, so the result can be assessed with RawStride's own view and
  zoom. The status line says which one is shown.
- **The export in the tab** (P-Q4): an export in the image's own folder would today be listed as an
  image of its own, since `IMG_1_DxO.jpg` does not pair with `IMG_1.ARW` as RAW+JPEG does. The
  proposal is that an export found by a profile is shown under its original and not as a cell of
  its own, with a View setting to list exports as their own images again.

### 2.4 When it is read

- When a tab reads its folder, the profiles' patterns are matched against the names already
  listed; a subfolder of an export rule costs one more listing per folder, only when a profile
  names one. No file is opened.
- After "Open in {name}" nothing is watched (ADR 0019 §2.7). A new edit file or export is seen when
  the folder is loaded again, and by itself once ADR 0026 F2–F3 follow changes on disk (P-Q11).

### 2.5 An export older than its edit

- An export counts as **older than the edit** when the edit file's mtime is later than the
  export's by more than **60 seconds** (P-Q6). P0 showed the `.dop` written about 2.2 s after its
  export, so a margin of a few seconds would mark every fresh export as old; a real later edit
  made the `.dop` minutes newer. An edit made within a minute of the export is not marked.
- Known limits, to be stated in the user guide: copy tools that keep mtimes may make an old pair
  look current; a changed rating or label in the program also rewrites the `.dop` and so marks
  the export as old; a removed edit leaves no mark at all. Only viewing an image does not touch
  the `.dop` (P0).

### 2.6 Renaming and moving

- The edit file already follows its image through the companion list (U9b, ADR 0021). The
  profile's edit file suffix is added to the companion list automatically when it is not there
  (P-Q8).
- An export has its own name and is not a companion: renaming the original breaks the link by
  name, and the mark goes away. Whether exports follow a rename is P-Q8.

### 2.7 Capture One and programs that keep edits elsewhere

Some programs keep their edits outside the image's folder, in a session subfolder or in a
database of their own. A session subfolder can be described by the same patterns (a relative
subfolder); a database cannot, and such a program only gets the export rule. Capture One is not
part of the first steps (P-Q9): it is tested with the owner's files first, and added by an
amendment. Known in advance: since such edits do not lie beside the image, a rename or move in
RawStride breaks the editing program's own link to the image, so images are renamed and moved
before they are edited; a session works better than a database for this.

### 2.8 Metadata both ways with DxO PhotoLab

- **The program's setting "Synchronize metadata with XMP sidecar files" must be on.** With it off,
  the program keeps its own copy and can write it over RawStride's later changes (§1.1 row 7). The
  user guide says so, and the shipped DxO PhotoLab profile names the setting in Settings.
- With the setting on and ADR 0007 revision 26 done, ratings, labels, keywords and text go both
  ways for ARW sidecars and DNG files alike.
- **Rejects do not cross** (P-Q12, P-Q13): the program neither reads RawStride's `xmp:Rating` −1
  nor writes its own reject to XMP. A reject is made in RawStride; an image rejected in RawStride
  shows no rating in the program.
- **Until revision 26 is done**, a change in RawStride reaches the program only through its
  "read metadata from file" command; the user guide says so for that time.
- Fields the program adds by itself (Location Shown city, Image Creator name, `exif:DateTimeOriginal`
  in XMP) are kept by RawStride as any other value; the time is noted in ADR 0020.

## 3. What this costs [estimate, 2026-11-19, unmeasured]

| Part | Lines, tests included |
|---|---|
| Profiles in `settings.txt`, reading, writing, the DxO PhotoLab profile | 250–400 |
| The profile section in the Settings window | 250–400 |
| Matching patterns against a folder listing, the age rule | 250–400 |
| Marks in the contact sheet, the inspector rows | 200–350 |
| Original ↔ export in the preview, exports under their original | 300–500 |
| Texts, glossary, user guide | 100–200 |
| **Total** | **≈ 1,350–2,250**, about 5–8 tasks |

No new dependency.

## 4. Questions (open)

| # | Question | Proposal |
|---|---|---|
| P-Q1 | **Form of a profile:** (a) built-in kinds only (DxO PhotoLab, later others, each in code); (b) name patterns with placeholders, edited by the user, with built-in filled-in profiles | (b): a new program needs no code; DxO PhotoLab ships filled in |
| P-Q2 | Is a profile a part of a program in the ADR 0019 list, or a list of its own? | A part of the program: the path is already there, and at most five programs means at most five profiles |
| P-Q3 | Where DxO PhotoLab's exports land and what they are called (same folder or subfolder, suffix) | **P0:** `{base}_DxO.jpg` in the image's folder with the owner's preset; the shipped profile uses it, confirmed by `xmpMM:PreservedFileName` (§2.1, §2.2) |
| P-Q4 | An export in the image's folder: its own cell as today, or shown under its original? | Under its original, with a View setting "Show Exports as Own Images" (off) |
| P-Q5 | The marks in the contact sheet: which, where, and are they on by default? | Three marks (edited, exported, export older than the edit) in a corner of the cell; on when a profile is active; a View setting to hide them |
| P-Q6 | The age rule: mtime of the edit file against the export, and its margin | **P0:** 60 s (§2.5). The alternative, remembering the edit file's time when the export is first seen, needs stored state and fits the catalog (ADR 0030) better |
| P-Q7 | Key and names for switching original ↔ export | Proposed in step P4, checked against the key map (U8) and the glossary |
| P-Q8 | Renaming and moving: the edit file suffix added to the companion list automatically? Do exports follow a rename or move? | Suffix added automatically; exports do not follow (they are the program's output), the lost link said in the rename dialog when exports exist |
| P-Q9 | Capture One in this ADR? | No: an amendment after a test with the owner's sessions (where edits and exports land, colour labels against XMP Label) |
| P-Q10 | Read the contents of edit files (for example to show the edit's settings)? | Never (§2.2) |
| P-Q11 | New edit files and exports seen without loading the folder again? | Through ADR 0026 F2–F3 when they are done; until then on "Load a folder again" |
| P-Q12 | Can the culling marks that RawStride writes (rating, label, reject) be read by DxO PhotoLab? | **P0:** rating and label yes, both ways (with the setting of §2.8 on); reject no, in neither direction |
| P-Q13 | Rejects between the programs: accept that they do not cross, or let RawStride write something the program reads (for example a reject also as a colour label or a keyword)? | Accept it: the reject is made in RawStride, and writing a second mark would break the owner's own label and keyword rules |

## 5. Steps (proposed)

Each step ends with tests (GR11) and the owner's hand check; steps that change the preview also
with `latency.ps1`.

| Step | Content | Needs |
|---|---|---|
| ~~**P0**~~ | **Done 2026-11-19 (§1.1):** the owner's test in DxO PhotoLab, no code: images from several folders, ratings, labels and rejects, the export's folder and name, the `.dop` when only viewing, the program's XMP writing with its synchronisation off and on, ARW and DNG | ADR 0019 E2 (done) |
| ~~**P0b**~~ | **Done 2026-11-19, hand-checked by the owner the same day.** ADR 0007 revision 26 (the modification time, Q15, Q16; decided by the owner 2026-11-19): three tasks: T206 (`pw-ingest`, the import recognises a written file by its creation time) → T204 (the modification time) → T205 (Q15, Q16); needed for §2.8 | – |
| **P1** | Profiles in the settings: data, `settings.txt` lines, the shipped DxO PhotoLab profile, the companion suffix (P-Q8) | P0, P-Q1, P-Q2 |
| **P2** | The profile section in the Settings window | P1 |
| **P3** | Matching when a folder is read; the marks in the contact sheet and the inspector; the age rule | P1 |
| **P4** | Original ↔ export in the preview; exports under their original (P-Q4); `latency.ps1` | P3 |

Order: P0b first (it helps without any profile), then P1, P2, P3, P4; P2 and P3 may swap. Priority after the work already under way (ADR 0019
E4, ADR 0024 O1, ADR 0027 W1–W9), unless the owner places it otherwise.

## 6. Not included

- Reading or changing edit files (P-Q10).
- Capture One and programs that keep edits in a database (P-Q9).
- Starting an export in the other program from RawStride.
- The catalog across folders and search (ADR 0030).

## 7. Consequences

- **ADR 0007:** revision 26 (from P0): a written file gets a new modification time; a flat
  `lr:hierarchicalSubject` and the rating percentages kept equal to RawStride's values.
- **ADR 0020:** DxO PhotoLab writes `exif:DateTimeOriginal` into XMP, which C2 reads before the
  embedded time.
- **ADR 0019:** each program in the list may carry a profile (P-Q2); §2.7 gets a pointer here.
- **ADR 0017:** the companion list gets the profile's suffix automatically (P-Q8); the View menu
  gets the settings of P-Q4 and P-Q5 and the switch of P-Q7.
- **ADR 0021:** the rename dialog tells when exports lose their link (P-Q8).
- **Settings and glossary:** the profile lines and the new texts.
- With no active profile nothing is matched or drawn, so V1–V8 are unchanged for a user without
  one.
