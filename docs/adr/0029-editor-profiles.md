# ADR 0029: Editor profiles: what an editing program leaves beside the images

- Status: **proposed (2026-11-19).** A draft for the owner. No question in §4 is answered,
  nothing is placed in the step plan, and nothing is built, installed or added as a dependency by
  this ADR. Step P0 is the owner's test with the owner's own editing program and comes before
  anything else; when the questions are answered, each step in §5 becomes its own task, with its
  own approval and commit.
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

## 2. Decision (proposed)

### 2.1 An editor profile

An **editor profile** is a short description, kept in the settings, of the files an editing program
leaves behind. It has three parts:

1. **The program**: one of the programs in the ADR 0019 list (P-Q2).
2. **The edit file rule**: where the program keeps its edit for an image, written as a name
   pattern relative to the image, for example `{file}.dop` for DxO PhotoLab (R2: `IMG_1.ARW` →
   `IMG_1.ARW.dop`).
3. **The export rule**: where and under which name the program's exported images land, for example
   `{base}_DxO.jpg` in the image's folder, or `{base}.jpg` in a subfolder (P-Q3).

Patterns use a few placeholders (`{file}` the image's full file name, `{base}` without the
extension, `{ext}` the extension) and may start with a relative subfolder (P-Q1). RawStride ships a
filled-in profile for DxO PhotoLab, adjusted to what P0 shows; other programs are described by
the user with the same patterns, with no code per program.

Several profiles may be active at once (one per program), so the edits made in one program stay
visible after a change to another.

### 2.2 Only names, never contents

RawStride finds edit files and exports **by name only**, in the folder listing it already reads.
It never opens, parses or changes an edit file (P-Q10): their formats belong to their programs,
and reading them is neither needed nor clean-room safe. The time an edit file was last changed
(its mtime) is the only thing read about it.

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
  export's by more than 2 seconds (FAT32 keeps times to 2 seconds).
- Known limits, to be stated in the user guide: copy tools that keep mtimes may make an old pair
  look current; a program that rewrites its edit file merely when an image is opened makes a
  current export look old (checked in P0); a removed edit leaves no mark at all.

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
amendment.

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
| P-Q3 | Where DxO PhotoLab's exports land and what they are called (same folder or subfolder, suffix) | Answered by P0 on the owner's files; the shipped profile follows the owner's own export preset |
| P-Q4 | An export in the image's folder: its own cell as today, or shown under its original? | Under its original, with a View setting "Show Exports as Own Images" (off) |
| P-Q5 | The marks in the contact sheet: which, where, and are they on by default? | Three marks (edited, exported, export older than the edit) in a corner of the cell; on when a profile is active; a View setting to hide them |
| P-Q6 | The age rule: mtime of the edit file against the export, 2 s margin | As §2.5, checked in P0 |
| P-Q7 | Key and names for switching original ↔ export | Proposed in step P4, checked against the key map (U8) and the glossary |
| P-Q8 | Renaming and moving: the edit file suffix added to the companion list automatically? Do exports follow a rename or move? | Suffix added automatically; exports do not follow (they are the program's output), the lost link said in the rename dialog when exports exist |
| P-Q9 | Capture One in this ADR? | No: an amendment after a test with the owner's sessions (where edits and exports land, colour labels against XMP Label) |
| P-Q10 | Read the contents of edit files (for example to show the edit's settings)? | Never (§2.2) |
| P-Q11 | New edit files and exports seen without loading the folder again? | Through ADR 0026 F2–F3 when they are done; until then on "Load a folder again" |
| P-Q12 | Can the culling marks that RawStride writes (rating, label, reject) be read by DxO PhotoLab? | Answered by P0; if not, the profile says so and nothing else changes |

## 5. Steps (proposed)

Each step ends with tests (GR11) and the owner's hand check; steps that change the preview also
with `latency.ps1`.

| Step | Content | Needs |
|---|---|---|
| **P0** | **The owner's test, no code:** in DxO PhotoLab, (1) several images from different folders in one "Open in"; (2) whether RawStride's ratings, labels and rejects are read from XMP; (3) the export's folder and name with the owner's preset; (4) whether the `.dop` changes when an image is only opened; (5) whether DxO PhotoLab rewrites RawStride's XMP | ADR 0019 E2 (done) |
| **P1** | Profiles in the settings: data, `settings.txt` lines, the shipped DxO PhotoLab profile, the companion suffix (P-Q8) | P0, P-Q1, P-Q2 |
| **P2** | The profile section in the Settings window | P1 |
| **P3** | Matching when a folder is read; the marks in the contact sheet and the inspector; the age rule | P1 |
| **P4** | Original ↔ export in the preview; exports under their original (P-Q4); `latency.ps1` | P3 |

Order: P0, P1, P2, P3, P4; P2 and P3 may swap. Priority after the work already under way (ADR 0019
E4, ADR 0024 O1, ADR 0027 W1–W9), unless the owner places it otherwise.

## 6. Not included

- Reading or changing edit files (P-Q10).
- Capture One and programs that keep edits in a database (P-Q9).
- Starting an export in the other program from RawStride.
- The catalog across folders and search (ADR 0030).

## 7. Consequences

- **ADR 0019:** each program in the list may carry a profile (P-Q2); §2.7 gets a pointer here.
- **ADR 0017:** the companion list gets the profile's suffix automatically (P-Q8); the View menu
  gets the settings of P-Q4 and P-Q5 and the switch of P-Q7.
- **ADR 0021:** the rename dialog tells when exports lose their link (P-Q8).
- **Settings and glossary:** the profile lines and the new texts.
- With no active profile nothing is matched or drawn, so V1–V8 are unchanged for a user without
  one.
