# ADR 0017: The RawStride interface – main window, preview and inspector

- Status: **accepted by the owner (2026-11-12).** The owner approved the proposals of every open
  question (U-Q1–U-Q4, U-Q6–U-Q8, §6), the technical assessment additions (§2.6), the metadata
  window (§2.3.1), the order with a usable version first (§7.1) and the list of what is not
  included (§7.2). Nothing is built, installed or added as a dependency by this ADR. Each step in
  §7 becomes its own work part, with its own approval and commit.
- **Amended by the owner (2026-11-13) after a review of 32 findings:**
  - the deprecated IIM fields stay read-only (finding 1, §2.3.1);
  - one screen and **one window** first: the preview is a full-screen mode of the main window, and
    the separate preview window waits for a second screen (finding 5, §2.2, U-Q1);
  - the filmstrip moves to stage A (U3);
  - current image and selection (finding 6, §2.2);
  - the rules for previous/next in the metadata window (finding 7, §2.3.1);
  - menu names are decided in U1 (finding 21, §2.1).

  The remaining findings were clarifications, written in on the owner's go-ahead the same day.
- **Recorded 2026-11-13 (no new decision):** the owner's answers to the questions of U1, U2, U3
  and U12, approved in the tasks T109–T112, are in §6.1; the inspector's Metadata section shows no
  origin line under its values (T114, §2.1).
- **Recorded 2026-11-14 (no new decision):** the owner's answers to the questions of U10 (T123) and
  U10c (T124) are in §6.1; the section names of §2.3.1 are English in the program (GR14, U10-Q1).
  The answers to the questions of U12b (T125) and U12c (T126) are in §6.1 too.
- **Wishes added by the owner (2026-11-15):** the import's IPTC template is filled in a window of
  its own and saved as an XMP template file (U13, §7); and the imported images appear in their tab
  while the import runs, not only when it is done (§7.2).
- Date: 2026-11-12; amended 2026-11-13
- Related:
  - [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md) (GR1–GR13), in particular GR6 (keyboard first),
    GR7 (DPI, other screen), GR12 (imports open in a tab) and GR13 (folders open as a contact sheet);
  - [requirements-image-viewer.md](../requirements-image-viewer.md) (V1–V8, G1–G4), which is
    unchanged and applies to every view that shows an image. Its "Not covered" item (a second
    screen) stays future: stage A uses one screen (§2.2);
  - ADR 0009 (the loupe program), 0010 (import and tabs), 0011 (contact sheet), 0013 (ratings and
    labels), 0014 (filter and multi-selection), 0016 (colour management), 0007 (IPTC; D16 the modal
    metadata window on `M`);
  - [clean-room.md](../clean-room.md): this ADR describes workflow and layout in our own words.

## 1. Context

- **The owner's goal from the start** is a culling and ingest program that works the way
  professional photo-desk culling tools work: a fast contact sheet, a separate key-driven preview,
  metadata always at hand, ingest running in the background. Plus what such tools lack: a real RAW
  view next to the embedded JPEG (G4, V7).
- **That goal was studied on 2026-11-04** (levels A, B, C and D under clean-room.md; the raw notes
  are outside the repo). The study proposed an interface of its own. Only the performance part
  became requirements (V1–V8). The interface proposal was never turned into a decision.
- **What was built instead** (ADR 0009–0016 and milestone 8) follows the feature list of the first
  Tauri prototype (GR8): one window, folder tabs, a contact sheet that switches to a loupe in the
  same area, a status line, and separate dialogs for import, settings and metadata. Each part works
  and is tested, but the whole does not feel like the intended tool.
- **The core is reusable as it is:** `pw-viewcore` (views, caches, RAW and JPEG modes), `pw-thumbcache`,
  `pw-ingest`, `pw-metadata` with the safe write, `pw-colour`, the rating store, the filter and the
  selection logic. This ADR changes the **shell** around them, not the core. The one exception is
  the RAW histogram, which needs a new function in `pw-libraw` (§2.6, U7).

## 2. Decision

RawStride gets a **main window** for browsing and a **preview** for viewing (in stage A a
full-screen mode of the main window, §2.2), with the inspector and tasks always available.
Workflow: contact sheet → preview → metadata, all from the keyboard.

### 2.1 Main window

```
┌ Menu bar (menus named and ordered in U1) ──────────────────────────────────────────────────────┐
│ Command bar: Importera · Öppna mapp · Visa (alla/markerade/…) · ★ filter · färgfilter · storlek │
├──────────────┬────────────────────────────────────────────────────────────┬──────────────────┤
│ Library      │ Folder tabs: [2026_10_30 fågel ×] [RAW ×] [+]               │ Inspector        │
│ ▾ Mappar     │ Contact sheet (virtualised grid, size control)              │ ▾ Info (Exif)    │
│ ▸ Favoriter  │   cell: thumbnail · name · stars · label bar · RAW/JPEG     │ ▾ Metadata       │
│ ▸ Senaste    │   badge · reject mark                                        │ ▸ Histogram (U7) │
│ ▾ Uppgifter  │                                                              │ (collapsible)    │
│  (import,    │                                                              │                  │
│   writes)    │                                                              │                  │
├──────────────┴────────────────────────────────────────────────────────────┴──────────────────┤
│ Status: n of N selected (MB) · filter · source of the current image · task progress           │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Menu bar** with every command, each showing its key. Commands that exist today are moved into
  it (import, settings, ratings, labels, filter, RAW/JPEG mode, metadata). **The menus are named and
  ordered from RawStride's own commands in U1**, not after another program's menu bar (clean-room.md;
  owner, 2026-11-13, review finding 21).
- **Command bar** with the controls used while culling: import, **open folder** (new: Ctrl+O, which
  ADR 0009 §3.3 reserved, with the folder dialog `rfd` already used), the filter of ADR 0014
  (shown/selected, stars, labels) and thumbnail size. **Sort order** is new function and comes in
  its own step (U14), not in stage A.
- **Library (left):** a folder tree of the computer's drives, favourites, recently opened folders.
  Clicking a folder opens it in a tab (GR13 unchanged: as a contact sheet).
- **Tasks (left, below):** running and finished background work: import with per-file progress and
  a **stop** button for a running import, pending metadata and rating writes, errors as a list (not
  modal message boxes).
- **Folder tabs and contact sheet:** as today (ADR 0010, 0011); sort order is added in U14.
- **Inspector (right):** collapsible sections for the current image:
  - **Info:** an Exif summary (camera, lens, focal length, aperture, exposure time, ISO, capture
    date). This is new function: the program shows no Exif today. The source per file type is
    decided in U2 (`pw-raw` already reads lens, exposure time and ISO for RAW files).
  - **Metadata:** every value the image has, as the `M` window shows them but read-only, without an
    origin line under each value; the conflicts keep theirs (owner, 2026-11-13, T114: less text in
    the windows): the 22 fields of D12 and the rest, the open and decided conflicts and the notices.
    Read with the rules of T103 for a panel that follows the current image: a 200 ms wait, at most
    one read at a time, old answers discarded. With several images selected it shows the current
    image.
  - **Histogram:** comes with U7.

  Editing stays in its own window (§2.3).
- **Status bar:** selection and total, active filter, the source of the shown image (G1), progress.
- Both side columns can be resized, collapsed and hidden with one key (named in U1; not Tab, which
  changes tab today); their state is remembered (§2.5). The right column starts collapsed and is
  opened from Visa (owner, 2026-11-13): the inspector is mainly for judging an image, which the
  preview's right column serves.

### 2.2 Preview

```
┌ Title line: TN6073.ARW · RAW 9504 × 6336 · 100 % · 3 of 270 · RAW | JPEG ───────────────────┐
├──────────────────────────────────────────────────────────────────────┬──────────────────────┤
│ Image area (one image; two with compare, U6)                         │ ▾ Info               │
│ overlay: name · stars · label · source                               │ ▾ Zoom               │
│                                                                      │ ▸ Histogram (U7)     │
│                                                                      │ ▸ Clipping (U7)      │
├──────────────────────────────────────────────────────────────────────┴──────────────────────┤
│ Filmstrip (all shown images, or only the selected)                                          │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Stage A: a full-screen mode of the main window (owner, 2026-11-13).** The owner uses one
  screen and cannot test two (review finding 5). Opening the preview switches the main window to
  full screen with only the preview: no frame, menu or taskbar. Escape returns to the contact sheet
  in the same tab. To the user this looks the same as a separate full-screen window on one screen,
  without the risk of a second OS window (§5).
- **A separate preview window** that can be moved to a second screen, with that screen's DPI (GR7)
  and colour profile (ADR 0016: images are converted to the profile of the screen the program
  started on, and the caches hold converted pixels), waits until the owner has a second screen to
  test with (§7.2).
- **Current image and selection (owner, 2026-11-13, review finding 6).** A step in the preview
  moves the **current image** in the preview and the contact sheet together; when the preview is
  closed, the last image shown is the current one in the contact sheet. A **selection of several
  images is not changed by a step** (this replaces ADR 0014 Q5 for the preview). With the filmstrip
  showing only the selected images, steps go only between them.
- **The loupe of today becomes the preview's image area.** V1–V8 and G1–G4 apply unchanged: zoom and
  position persist (V6), RAW and JPEG are chosen by the user (G4), the source is always stated (G1).
  The rating badge and the RAW/JPEG control (G4) stay visible in the preview whatever `I` does.
- **Right column:** Info and Zoom in stage A (U3); Histogram and clipping in U7.
- **Filmstrip** (U3, stage A): the contact sheet's thumbnails, order and filter in one row, or only
  the selected images.
- **Compare two images** side by side or above/below, with zoom and pan locked or free (U6).
- **Technical assessment tools** in the right column and on the image (§2.6).
- The right column and the filmstrip can be hidden with one key, leaving only the image.

### 2.3 Metadata

- The inspector shows the metadata of the current image read-only (§2.1). Today the values are
  only seen inside the `M` window, since D16 replaced the right-hand panel of T103; the inspector
  brings them back next to the image, without opening the window or locking the image.
- **Editing** happens in the metadata window, opened with `M` as today, extended as in §2.3.1.
- Later steps (not in this ADR): keyword panel, find and replace (§7.2).

#### 2.3.1 The metadata window: every field, and freedom to work (owner, 2026-11-12)

The window of D16 edits 22 fields (D12); every other property is shown read-only under "Övrig
metadata", and the window is locked to one image until OK or Avbryt. The owner wants the freedom of
a full photo-desk caption window.

- **What can be edited** (review finding 3): the fields of the six sections below. Every other
  property of the property table (for example image regions, artwork, products in the image, the
  AI fields, contributors, registry entries, rights expressions, data mining, maximum available
  size, model age, event id) is shown read-only under "Övrig metadata" until a later step decides
  on it. **The GPS coordinates** are read-only until the GPS ADR (§7.2).
- **Except the deprecated IIM fields (owner, 2026-11-13, review finding 1):** Prioritet, Kategori
  and Tilläggskategorier (`photoshop:Urgency`, `photoshop:Category`,
  `photoshop:SupplementalCategories`) are shown **read-only** in their section. ADR 0007 §5.5 is
  unchanged: they are kept and synchronised, never edited, created or removed. If a recipient ever
  requires them, §5.5 is revisited first.
- **Sections in a long scrolling form**, each collapsible, in this order:
  1. Innehåll: Titel, Rubrik, Beskrivning, Alternativtext, Utökad beskrivning, Beskrivningens
     författare, Nyckelord, Person i bilden, Organisation i bilden (namn, kod), Händelse;
  2. Fotograf och rättigheter: Fotograf, Yrkestitel, Kontaktuppgifter, Copyright, Credit, Källa,
     Användarvillkor, Webbadress för rättigheter, Upphovsrättsskyddad;
  3. Tid och plats: Skapad; **Plats, Stad, Län/delstat, Land, Landskod** (the older IPTC Core
     fields, also written to IIM, as today; review finding 2); Plats där bilden togs (underplats,
     stad, län/delstat, land, landskod, världsdel, plats-id); Plats som visas (several, with
     "kopiera från Plats där bilden togs"). The older fields and "Plats där bilden togs" are
     separate properties and are **not synchronised** automatically;
  4. Uppdrag: Instruktioner, Jobb-id, Prioritet, Kategori, Tilläggskategorier (read-only),
     Ämneskod, Genre, Scenkod;
  5. Licens (PLUS): Bildskapare, Upphovsrättsinnehavare, Licensgivare (each up to 3), Bildleverantör,
     modell- och egendomsrelease, Digital källtyp;
  6. Betyg och etikett.

  The user chooses which sections and fields are shown and in which order (Settings), so unused
  fields take no room. The Swedish names follow D11 (Yrkestitel and Kontaktuppgifter: owner,
  2026-11-13); new names are proposed with the step. The
  PLUS fields are built in U10 together with milestone 8's I11, after the PLUS vocabulary is read
  (ADR 0007 §16.4).
- **Lists and structures** (persons, locations, creators, licensors) get one row per entry with add
  and remove buttons, within the cardinalities of ADR 0007 §8.
- **Not locked to the image.** Previous/next buttons and "spara och nästa" (also as keys) move the
  window to the next image in the contact sheet's order. With unsaved edits, moving or closing asks
  "Spara ändringarna?" (Spara / Släng / Avbryt), so nothing is lost and nothing is written
  silently. This replaces D16's lock (U-Q8, approved 2026-11-12). The rules (owner, 2026-11-13,
  review finding 7; they apply from stage A):
  - the window stays **modal**: the contact sheet and preview take no keys while it is open;
  - previous/next follow the **shown images** in the contact sheet's order (after the filter), and
    skip image types without metadata support;
  - **Escape** asks the same Spara / Släng / Avbryt question, replacing D16's "Släng ändringarna?";
  - if the **write fails**, the window stays on the image with the edits kept and the reason shown;
    with **write notices** it stays and shows them, and moves on only on the next key press.
- **Several images:** "använd på alla markerade" writes the edited fields to every selected image,
  with one confirmation that names the count. Fields not touched are left as they are on each image.
  The rules of ADR 0007 apply (review finding 19):
  - it runs as a batch under §9.5: each file is its own write, with a summary of the files that
    failed;
  - an emptied copyright field is not emptied on the other images (§8); other emptied fields are
    named in the confirmation (as D14);
  - files with open conflicts are listed (Q1).

  The same rules apply to "klistra in alla fält" on several images.
- **Speed while captioning:**
  - each field has a list of recent values and the user's own values (▼ beside the field);
  - "kopiera alla fält" and "klistra in alla fält" between images;
  - load and save a template from the window (the template format of ADR 0007 §10); which of the
    modes of §10 (keep, replace, append, clear) a template uses when loaded in the window is
    decided in U13;
  - variables in text fields (for example date, file name, camera), expanded when written; the
    variable list is decided in its step.
- **What stays read-only, and why** (rules that protect the files, ADR 0007):
  - values RawStride cannot read safely (unknown namespaces, unreadable values): shown, never
    rewritten (§8);
  - hierarchical keywords: kept unchanged until a keyword step can edit them (§8);
  - camera data (Exif such as exposure and lens): shown in the inspector's Info, not edited.

  A manufacturer RAW file is still never changed; its edits go to the sidecar as today (§4).

### 2.4 Keyboard

- One key map for the contact sheet and the preview, stored in a file and changeable in Settings
  (U8). Until then the keys are fixed in code, as today.
- RawStride defines its own default map. Common functional keys are kept (arrows, Home/End, Space,
  Z for fit/100 %, +/−, Escape, F11). The rating and label keys of ADR 0013 are unchanged.
- **Stage A** (review finding 15; today's keys of ADR 0009 and 0011 kept):

  | Action | Keys |
  |---|---|
  | Open the preview from the contact sheet | Enter, Space, G, double-click |
  | Close the preview | Escape, G, Enter |
  | Fit ↔ 100 % in the preview | Z, double-click (as ADR 0009) |
  | Hide the side columns (main window) or the right column and filmstrip (preview) | one key, named in U1; not Tab (it changes tab) |
  | Full screen for the main window | F11 (U1) |
  | Show and hide the inspector | `I` (U2) |

- No other program's complete map is copied (clean-room.md, own expression).

### 2.5 Look

- A dark, colour-neutral theme for image work; a light theme as an option (U-Q3; built in U1).
- Text contrast meets **WCAG AA** in both themes (a dark theme easily gets too little contrast).
  Font, size and spacing are chosen in U1.
  **Owner, 2026-11-16 (T155):** normal text uses egui's bundled fonts, and egui's bundled `Hack` is the
  last font of normal text, so that the arrows ↔ → ↑ ↓ are drawn instead of boxes. No new font and no
  emoji fonts.
- One accent colour for selection and active controls.
- **Icons:** an openly licensed icon set is a new dependency. It is chosen in its step with the
  owner's approval and a licence check (GR10, ADR 0002).
- **Windows-native window frames**, DPI-correct (GR7). The study proposed an own compact title bar;
  native frames are chosen because they keep Windows' own DPI, snapping and accessibility behaviour.
- **Remembered state** (window position, size, full screen, column widths, open sections) is saved
  in the settings file of D10 (`settings.txt`), not with eframe's `persistence` feature, which stays
  off (GUI_REQUIREMENTS §6).

### 2.6 Technical assessment in the preview (owner's choice, 2026-11-12)

The owner chose the main window and preview above as the direction, with these additions for
judging one image technically. They are drawn in the preview's right column and as overlays on the
image. None of them changes a file; they only affect what is shown. They are the owner's own wishes,
described in our own words.

- **RAW histogram and exposure statistics (U7).** In RAW mode the histogram is computed from the RAW
  data after white balance, per channel, not from the displayed 8-bit image. Below it: the share of
  pixels at the clipping level per channel, and the share in the deepest shadows. In JPEG mode the
  same panel shows the histogram of the embedded JPEG and says so (G1); it is computed **before**
  the conversion to the screen profile (ADR 0016). `pw-viewcore` only receives finished 8-bit RGB
  images; the unpacked RAW data stays in the LibRaw session (`pwlr.cpp` unpacks once). So this
  needs **a new function in the C++ shim and `pw-libraw`** that computes the statistics from the
  already unpacked data; no new decode [T, confirmed in U7]. The histogram section of the
  inspector comes in the same step.
- **Clipping overlay (U7).** Highlights at the clipping level and shadows below a threshold are
  marked on the image, each with its own key, in RAW mode from the RAW data and in JPEG mode from
  the JPEG. In RAW mode the RAW positions are mapped to the shown image (orientation, borders, half
  size).
- **Focus marking (U9).** Edges with high local contrast are marked in a colour on the image (one
  key), so sharp areas can be seen at fit without zooming. Computed from the shown image.
- **Move on reject or select (U9; setting, off by default).** When the setting is on, a reject
  moves the image with all its companion files (below) to a subfolder `_rejected`, and a select key
  moves them to `_picked`, in the image's own folder. Names and keys are decided in U9. This answers
  ADR 0013 §13 (review finding 8):
  - **both** happen: the rating is set and the files are moved;
  - a pending rating or metadata write to the file or its sidecar is **finished before** the move,
    since the write journal (ADR 0007 §9.5) points at the path;
  - the image **leaves the tab's list**; the contact sheet closes the gap;
  - the rating store, the metadata cache and the thumbnail cache **follow the files** to the new
    path;
  - **undo** (Ctrl+Z, U-Q6) moves the files back and restores the rating; changing the rating of a
    moved image back is done through undo, since the image is no longer in the folder's list.
- **Companion files move with the image (owner, 2026-11-12).** Every file in the same folder that
  belongs to the image moves with it, so no other program loses its edits:
  - the RAW+JPEG partner (same base name);
  - XMP sidecars, both `<base>.xmp` and `<base>.<ext>.xmp` (the second kind is never read or written
    by RawStride, ADR 0007 Q8, but it is moved);
  - other programs' edit files, for example DxO PhotoLab `.dop`, RawTherapee `.pp3` and ON1 `.on1`,
    named `<base>.<ext>.<suffix>` or `<base>.<suffix>`.

  The suffixes are a list in Settings with these as the default. The naming of each program's files
  is checked on real files before U9 (the owner's DxO PhotoLab files first).
  **R2 (reviewer, 2026-11-17; file names only, clean-room level C):** the owner's folder of five
  Sony ARW files edited in DxO PhotoLab holds, per image, `<base>.ARW`, `<base>.ARW.dop` and
  `<base>.XMP`. So DxO PhotoLab names its file `<base>.<ext>.dop`, with the image's extension in
  the image's own case and `.dop` in lower case; no `<base>.dop` was seen, and no other DxO file
  beside the images. The `.XMP` in upper case is Photo Mechanic's (ADR 0013 §14). Names are
  therefore compared without regard to case (Windows). No RawTherapee `.pp3` or ON1 `.on1` files
  were available; their names stay as above (both forms are moved) until real files are seen. The move is all or
  nothing: if one file cannot be moved (open in another program, name taken in the subfolder), the
  files already moved are moved back and the reason is shown. Edit data kept outside the image's
  folder (for example a Capture One session or a Lightroom catalogue) is not moved; such a program
  must find the file again itself.

Not included: temporary exposure or white balance correction for assessment (§7.2).

### 2.7 Own design

The ideas of the study are free to use, but the form is our own (clean-room.md). Order, texts and
placement of titles, columns and buttons are decided in each step from RawStride's own commands,
without the study notes as a template.

## 3. What stays

- The whole core (§1) and every behaviour requirement V1–V8, G1–G4, GR1–GR13.
- Folder tabs, the contact sheet, ratings, labels, filter, multi-selection, the import engine, the
  safe metadata write, colour management.
- The Tauri app is not touched (D7). Its removal is a separate decision (GR8; V3, V4, V7).

## 4. What changes in the existing program

| Today | Proposed |
|---|---|
| One window; the loupe replaces the contact sheet in the same area, inside the window frame | Main window with library, contact sheet and inspector; the preview as a full-screen mode with right column and filmstrip (a separate window later, §2.2) |
| No menu bar; commands only by key | Menu bar and command bar, keys shown in the menus; "Öppna mapp" (Ctrl+O) |
| `I` hides the status line in the loupe (with the rating badge and the RAW/JPEG control); no info panel (T103's panel was removed by D16) | `I` shows and hides the inspector; the rating badge and RAW/JPEG control always visible in the preview |
| No Exif shown | Inspector Info with an Exif summary |
| Import progress in the status line | Tasks area with per-file progress, a stop button and an error list |
| Folders opened by drag-and-drop, import or the command line | Folder dialog, folder tree, favourites and recent folders as well |
| Metadata window: 22 editable fields, the rest read-only; locked to one image until OK or Avbryt | The fields of six sections editable, in sections the user can arrange; previous/next and "spara och nästa"; recent values, copy/paste of all fields, templates, variables; "använd på alla markerade" (§2.3.1) |
| No compare, filmstrip, histogram or sorting | Filmstrip (stage A); two-image compare, RAW histogram with exposure statistics, clipping overlay, focus marking, sort order |
| A reject only writes the rating | Optionally also moves the image and its companion files (sidecars, RAW+JPEG partner, `.dop` and other edit files) to `_rejected` (select: `_picked`) |
| Fixed keys in code | One key map in a file, editable in Settings |

## 5. Risks and things to measure

- **A second OS window in egui with glow [T]** is avoided in stage A (§2.2). eframe 0.37's glow
  integration has multi-viewport support (one shared painter for all viewports, one OpenGL context
  made current and one buffer swap per window). When the separate window is taken up, a prototype
  (U0) must show that it meets V1–V8, including the time per step when both windows are redrawn
  (each swap may wait for the screen refresh), the choice between an immediate and a deferred
  viewport (how the program state is shared), and the second screen's DPI (GR7) and colour profile
  (ADR 0016).
- **Texture memory** when the contact sheet's thumbnails and the preview are both held: the separate
  texture budget (GUI_REQUIREMENTS §7) may need a new share.
- **The filmstrip** must not slow down stepping: its thumbnails are fetched below the shown image's
  priority, and U3 measures this with `latency.ps1`.
- **Focus** (GR6): keys must go to the view the user looks at, and never be lost after closing a
  dialog; with a separate window later, also between the two windows.

## 6. Questions (all answered by the owner, 2026-11-12: every proposal approved)

| # | Question | Answer (owner, 2026-11-12: as proposed) |
|---|---|---|
| U-Q1 | Should the preview also be usable inside the main window (for a one-screen laptop), or only as its own window? | Both: its own window by default; "preview in main window" as a setting. **Changed 2026-11-13 (owner):** stage A has only the full-screen mode in the main window; the separate window comes with a second screen (§2.2) |
| U-Q2 | Which key opens the preview from the contact sheet? | Space and double-click; Enter does the same. G is kept as today (§2.4) |
| U-Q3 | Dark theme by default? | Yes, with a light theme in Settings |
| U-Q4 | Order against the rest of milestone 8 (I11–I13) | Finish I12 and I13 first; I13 documents milestone 8 without the PLUS fields, which move with I11 to U10 (§7.1) |
| U-Q5 | — | Removed during the drafting (2026-11-12) |
| U-Q6 | Move on reject/select: what happens to a file already moved when its rating is changed back? | It is moved back to the image's folder; a move can be undone with Ctrl+Z (§2.6) |
| U-Q7 | Focus marking colour and strength | One colour (configurable later), one strength; tuned on the owner's test files in U9 |
| U-Q8 | D16 locked the window to one image until OK or Avbryt. May the window instead move between images, asking about unsaved edits? | Yes: previous/next and "spara och nästa", with Spara / Släng / Avbryt when there are unsaved edits; rules in §2.3.1 |

### 6.1 Questions of the steps (answered by the owner, 2026-11-13: every proposal approved)

Asked in the tasks of stage A (T109–T112, `docs/tasks/done/`) and of U10, U12b, U12c and U10d (T123–T126, T128, 2026-11-14) and of U4, U11a and U14 (T135, T136, T138, T139, 2026-11-14) and of U5, U12a, U7 and U11 (T140–T143, 2026-11-15) and of U7's inspector part (T144, 2026-11-15) and of U7b (T146, 2026-11-15) and of U6 (T150, 2026-11-16) and answered there; recorded here so
the decisions do not live only in the tasks.

| # | Decision (as proposed) |
|---|---|
| U1-Q1 | The menus are **Arkiv, Redigera, Bild, Visa**, in that order, with names from RawStride's own commands (§2.1) |
| U1-Q2 | The hide key is **`H`**; Tab stays free for egui's focus moves (any key can be chosen later, U8) |
| U1-Q3 | Before U2, U4 and U5 the side columns hold headings and one line each, with the hidden import's status under "Uppgifter" |
| U1-Q4 | egui's bundled font; body text 14, small text 11, headings 16 points, the status bar in the proportional font at 13; egui's default spacing; the image area and contact sheet stay a neutral dark grey in **both** themes, so an image is judged against the same surround (the light theme lightens menus, bars and columns) |
| U1-Q5 | Neither "Visa (alla/markerade/…)" in the command bar nor the selection's size in MB in the status bar is in U1; both are placed when the owner reorders after stage A |
| U2-Q1 | Info comes from `pw_raw::inspect` for every listed file type, on a background thread, without ExifTool |
| U2-Q2 | Info shows the seven values of §2.1; the camera without a repeated make; a decimal comma; the date as `2026-10-30 14:03:12`; a missing value gives no row. Exposure compensation and image size are left out. **Changed by the owner, 2026-11-17 (ADR 0023 A-Q10):** both are added, and the serial number after ADR 0020 C1 |
| U2-Q3 | `I` opens and collapses the right column (also from `H`'s hidden state); "Information" and "Höger kolumn" become one menu item "Inspektör"; the status line, rating badge and RAW/JPEG control are always shown, so the old meaning of `I` ends in U2 |
| U2-Q4 | The Exif labels (Kamera, Objektiv, …) are not IPTC properties and are made in `inspector_text.rs`, not by `property_label` (rule 13) |
| U2-Q5 | Both inspector sections are open by default; the column itself starts collapsed |
| U3-Q1 | The preview has no menu bar, command bar, tabs, status bar or side columns; a title line holds the loupe's status line and the RAW/JPEG control; Ctrl+O, Ctrl+I, Ctrl+W, the tab keys and F11 do nothing until the preview is closed |
| U3-Q2 | The preview's right column is open by default and remembered apart from the main window's; `I` opens and collapses it, `H` hides it together with the filmstrip |
| U3-Q3 | The filmstrip is one row of 96-point thumbnails with the label bar and the current and marked frames; "only the marked" on `S` and a button, offered from two marked images; then steps go only between the marked |
| U3-Q4 | In the preview the thumbnail worker runs only while the shown image is drawn and pauses on each step until the new one is drawn, so stepping stays as fast as before and the filmstrip fills in when the user stops |
| U3-Q5 | The Zoom section shows the zoom as text, buttons Anpassa, Anpassa ↔ 100 %, Zooma in, Zooma ut, and the check box Zooma vid pekaren |
| U3-Q6 | The preview is always full screen; closing it returns the window as it was; started with a file path, RawStride opens in the preview |
| U12-Q1 | Ctrl+PageDown is next, Ctrl+PageUp previous and Ctrl+Enter "Spara och nästa"; they work while typing in a field; plain arrows and Enter stay with the fields |
| U12-Q2 | After a move the contact sheet's current image (and the loupe or preview) is the window's image; the marking is not changed |
| U12-Q3 | No wrap: the button and key are disabled at the first and last image that has metadata support |
| U12-Q4 | "Spara ändringarna?" (Spara / Släng / Avbryt) is asked for a move, for Escape and for Avbryt; OK is unchanged; "Spara och nästa" never asks; D16's "Släng ändringarna?" goes away |
| U12-Q5 | After a write with notices the window stays and shows them; the move happens on the next press of a move button or key, not on any key |
| U10-Q1 | The sections are named **Content**, **Creator and Rights**, **Time and Place**, **Assignment**, later **Licence (PLUS)** and **Rating and Label** (GR14; in place of the Swedish names of §2.3.1) |
| U10-Q2 | Person Shown, the organisation fields, Subject Code and Scene Code are one line separated by commas, as Keywords (T108); an entry cannot hold a comma for now |
| U10-Q3 | Copyrighted (`xmpRights:Marked`) is a check box writing `True` or `False`; a file without it shows it unchecked |
| U10-Q4 | Genre in Assignment is the structure `Iptc4xmpExt:Genre` and comes in U11; Intellectual Genre (legacy) stays under Other Metadata |
| U10-Q5 | Rating and Label come into the window in a later small task (U10c), written through the rating path, not the form's edits |
| U10-Q6 | Each section's open or collapsed state is remembered in `settings.txt`; all open at first |
| U10c-Q1 | The Rating and Label section has buttons for no stars, 1–5 and Reject, and for the five labels and no label, each acting as its key (a second press of the current label or Reject clears it) |
| U10c-Q2 | While the window is open, Alt+0–5 set stars and Ctrl+0–5 labels, also while typing in a field; plain digits, 6–9 and X stay with the text fields |
| U10c-Q3 | A rating or label from the window is written at once, as a key in the contact sheet; Cancel and Discard do not undo it |
| U10c-Q4 | The window keeps its image when a new rating puts it outside the filter; the contact sheet is refiltered when the window moves or closes |
| U10c-Q5 | The section is last in the form, open at first, its state remembered |
| U12b-Q1 | "Copy All Fields" takes the form as shown (unsaved edits included): every editable field of sections 1–4 and Copyrighted, not the read-only fields, the rating and label or the conflict choices; the copy is kept in memory until the next copy or the end of the program (not the Windows clipboard, not saved) |
| U12b-Q2 | "Paste All Fields" fills the form as unsaved edits (nothing written until OK, Save or "Save and Next"); only non-empty copied fields are pasted, so a paste never empties a field |
| U12b-Q3 | The two buttons sit in the button row before "Previous"; the Paste button's tooltip names the number of fields and the source file |
| U12b-Q4 | Ctrl+Shift+C copies and Ctrl+Shift+V pastes while the window is open, also while typing in a field; Ctrl+C and Ctrl+V stay with the fields |
| U12b-Q5 | English "Copy All Fields" / "Paste All Fields" (Swedish "Kopiera alla fält" / "Klistra in alla fält"), in the glossary |
| U12c-Q1 | With two or more images marked, `M` opens the window on the current image if it is marked, else on the first marked one, showing that image's values and "{n} images marked" |
| U12c-Q2 | "Apply to {n} Marked" sits beside the other buttons; OK and Save still write only the window's image |
| U12c-Q3 | Only the fields changed in the form go to the other marked images; never the chosen conflict values, and no emptied copyright-management field (emptied on the window's image only, ADR 0007 §8) |
| U12c-Q4 | One confirmation before anything is written, naming the count, the changed fields, the emptied fields, the copyright fields emptied only on this image and the skipped files; "Write" / "Cancel" |
| U12c-Q5 | One safe write per file (ADR 0007 §9.5); the window shows "Writing {i} of {n} …" and is locked until all have answered; then the window's image is read again |
| U12c-Q6 | The summary: "Written to {w} of {n} images", the failed files with their reasons, and the files where an unresolved conflict was kept |
| U12c-Q7 | The wait at program exit includes the batch's writes |
| U12d-Q1 | (owner, 2026-11-17, after the job test: keywords added to several marked images replaced their own) "Apply to {n} Marked" sends only the difference of a list field to the other marked images: an entry added in the window is added to each marked image that lacks it (at the end, exact text), an entry removed in the window is removed from each marked image that has it; every other entry of each image is kept. The window's image is written as before (U12c-Q3) |
| U12d-Q2 | U12d-Q1 holds for every list field of plain text entries (Keywords, Person Shown, Subject Code …). Creator is a copyright-management field (ADR 0007 §8): its added entries go to the other images, its removed entries are removed on the window's image only. The structure lists are replaced as whole lists as before (U11a-Q5) |
| U12d-Q3 | The confirmation (U12c-Q4) names, for each such list field, what is added and what is removed, for example "Keywords: adds vinter, älg; removes skog" |
| U12d-Q4 | With two or more images marked, the window says under "{n} images marked" that OK and Save write this image only |
| U12d-Q5 | Two tasks: the add and remove edit of a list in `pw-metadata`, applied to each file's own values when it is written (T179), then the shell's use of it, the confirmation and the line (T180) |
| U12d-Q6 | (owner, 2026-11-17, review of T179) A marked image whose list field has an unresolved (`Open`) conflict between its XMP and IIM entries gets no change to that field from the difference: the conflict is kept and decided by the user in that image's own window, so no entry that only one side holds is lost unseen. The user is told with the batch summary's existing line "Unresolved conflict kept: {file} ({field})" (U12c-Q6), also when that field was the image's only change and nothing was written (T179 gives the notice, T180 shows it) |
| U12d-Q7 | (owner, 2026-11-17, chain review of T179 and T180) Intended, from U12d-Q1 as it stands: metadata writes have no undo, so taking a batch back is the reverse change applied again, and removing an added entry removes it also from the images that had it before the batch; a pasted list replaces the window's list, so its difference removes from every marked image the window's entries the paste lacks. Adding an entry an image already holds never removes or changes anything |
| U12d-Q8 | (owner, 2026-11-17, same review) A marked image the difference leaves unchanged (an `Open` conflict, U12d-Q6, or every added entry already there) counts as written in "Written to {w} of {n} images"; the "Unresolved conflict kept" line tells the conflict apart. Known limits, not changed now: an entry with a comma ("Lund, Sweden") is split by the window's list text (as since T108), so removing it removes nothing on the other images; after the batch the window's image is read again, so a failed file is retried by making the change again |
| U12d-Q9 | (owner, 2026-11-17, hand check of U12d, approved) The info panel shows a list's entries joined with "; " (since T98), the metadata window's list text with ", " (T108): kept so. The panel cannot be edited, and "; " shows the entries' bounds also when an entry holds a comma ("Lund, Sweden; hav") |
| U10d-Q1 | The Licence (PLUS) section edits all four PLUS choice lists: Model and Property Release Status, Minor Model Age Disclosure and Data Mining, with the two release ids; Model Age stays under Other Metadata |
| U10d-Q2 | Digital Source Type is a choice list from IPTC's NewsCodes vocabulary, saved by the reviewer in `D:\PhotoWorkflowData\references\` (R2, 2026-11-14) |
| U10d-Q3 | The choices show the vocabularies' own English labels, the full URI as tooltip (GR14; ADR 0007 Q9 updated) |
| U10d-Q4 | A value not in the vocabulary is shown as the extra entry "Unknown value: {uri}", selected, with a warning; never removed unless the user picks another entry (ADR 0007 §8, §11) |
| U10d-Q5 | The Licence section's fields join "Copy All Fields", "Paste All Fields" and "Apply to All Marked", with the rules of U12b and U12c |
| U10d-Q6 | The section keeps the name "Licence (PLUS)" (U10-Q1), although Digital Source Type is an IPTC property |
| U4-Q1 | The drives come from `GetLogicalDrives`, `GetDriveTypeW` and `GetVolumeInformationW`, and the program calls `SetErrorMode(SEM_FAILCRITICALERRORS)` at start, so an empty card reader gives no "no disk" dialog: two more features (`Win32_Storage_FileSystem`, `Win32_System_Diagnostics_Debug`) of `pw-loupe`'s existing `windows-sys` =0.61.2, no new package (T135) |
| U4-Q2 | The tree shows drives, then folders only; hidden and system folders left out; sorted by name; a folder read when expanded, never in the background |
| U4-Q3 | Favourites are added with "Add to Favourites" (right-click on a tree folder or a tab, and in Arkiv) and removed with "Remove from Favourites"; kept in the order added as `favourite = <path>` lines in `settings.txt`; a missing one is greyed |
| U4-Q4 | Recent folders: the last 10 opened in any way, most recent first, `recent = <path>` lines; "Clear Recent Folders" |
| U4-Q5 | The tree highlights the active tab's folder when it is visible and does not expand by itself |
| U4-Q6 | A click opens the folder in a new tab, or in the tab that already shows it, as Ctrl+O and a drop (GR12, GR13) |
| U4-Q7 | The library has the sections **Folders**, **Favourites** and **Recent** (GR14), each collapsible and remembered; Tasks below as before |
| U4-Q8 | (Owner, 2026-11-16, T164.) In Favourites and Recent, a folder whose name another folder in the same list also has is shown with as many of its parent folders as make the names differ, for example "2025-07-25 (import\2025)"; the full path stays the tooltip |
| U11a-Q1 | U11a in two tasks: Location Created and Location Shown first (T136); Genre, Image Creator, Copyright Owner, Licensor and Image Supplier next |
| U11a-Q2 | The locations are in Time and Place after the older fields; one framed group per entry with seven sub-fields (Sublocation, City, Province/State, Country Name, Country ISO Code, World Region, Location Identifier) and "Remove"; "Add Location" under each list |
| U11a-Q3 | An all-empty entry is not written; removing every entry clears the property; no reordering; sub-fields the form does not show are kept (ADR 0007 §8) |
| U11a-Q4 | "Copy from Location Created" adds a new Location Shown entry from the first Location Created entry and never changes an existing one |
| U11a-Q5 | The two lists join "Copy All Fields", "Paste All Fields" and "Apply to All Marked", each as one field that replaces the whole list; empty lists are not pasted |
| U11a-Q6 | IPTC's English names (GR14); "Add Location", "Remove" and "Copy from Location Created" in the glossary |
| U11b-Q1 | The location lists of T136 become one general structure list; Genre and the four PLUS structures use it (T139) |
| U11b-Q2 | Genre is free text (CV-Term Name, ID, CV ID) for now; IPTC's Genre vocabulary as a choice list later |
| U11b-Q3 | Licensor shows Name, ID, Email, URL and the two telephones open, each telephone type a choice list from the PLUS vocabulary (work, cell, fax, home, pager); the six address fields under a collapsed "Address" line in each entry |
| U11b-Q4 | "Copy from Creator" under Image Creator adds an entry per Creator name not already there, within the limit; nothing synchronised automatically (ADR 0007 §8) |
| U11b-Q5 | The five lists join copy, paste and Apply to All Marked as the locations; Image Creator, Copyright Owner and Licensor emptied only on the window's image (copyright-management fields, ADR 0007 §8) |
| U11b-Q6 | IPTC's English names (GR14); Image Supplier at most 1 entry, Image Creator, Copyright Owner and Licensor at most 3; "Copy from Creator", "Address", "At most {n} entries" in the glossary |
| U14-Q1 | Sort keys: File Name (default), Capture Time, Modified, File Type, Rating (stars, then label); ties by file name (T138) |
| U14-Q2 | Ascending or descending for every key, one button beside the drop-down |
| U14-Q3 | One sort order for all tabs, remembered in `settings.txt` |
| U14-Q4 | Capture times read with `pw_raw::inspect` on a background thread, visible images first; the tab keeps its order and shows "{i} of {n}" until all are read, then is reordered once; no date sorts last |
| U14-Q5 | A rating key, a write or a refresh does not move an image; the order is applied when a sort is chosen, a folder is opened or a key read completes |
| U14-Q6 | Reordering keeps the current image (scrolled to) and the marking, and applies the filter again |
| U14-Q7 | Command bar: "Sort: {key}" drop-down and an ↑/↓ button; Visa ▸ "Sort By" with the keys and "Descending"; no key yet |
| U5-Q1 | In the left column under the Library, collapsible like the Library's sections, its state remembered |
| U5-Q2 | Source → destination, a progress bar by bytes and "Importing {i} of {n}: {file}" (IPTC and ejecting steps as in the dialog); a click shows the dialog; when done "Import done: {n} files" with failures counted, kept until "×" or the next import |
| U5-Q3 | "Stop" asks nothing: it does what the dialog's Cancel does; the files copied so far are kept |
| U5-Q4 | Rating writes ("Writing ratings: {n}"), "Apply to All Marked" ("Saving metadata: {i} of {n}") and the window's own save, each only while it runs |
| U5-Q5 | Failed rating writes and reads, an import's failed files and errors; newest first with the time; at most 200; "Clear"; not saved at exit |
| U5-Q6 | Only background errors go to the list; the boxes that need an answer (ExifTool missing or too old, unfinished writes at the last exit) stay |
| U5-Q7 | Tasks, No tasks, Stop, Errors ({n}), Clear, Writing ratings: {n}, Saving metadata (: {i} of {n}); English (GR14), in the glossary |
| U12a-Q1 | Every editable one-line text and list field (contact fields included); not the multi-line fields, the date, the check box, the choice lists or the structure lists' sub-fields |
| U12a-Q2 | What a successful write (OK, Save and Next, Apply to All Marked) put in the field; the last 10, newest first, no duplicates; a list field's entries one by one, only those the write adds (reviewer, T141) |
| U12a-Q3 | "Add to My Values" in the ▼ list adds the field's text; "×" removes one; no limit; alphabetical |
| U12a-Q4 | Replaces a text field's text; adds an entry at the end of a list field when it is not already there; written with the window's write as typing is |
| U12a-Q5 | A "▼" button right of the field, or Alt+↓ in the field; ↑/↓ and Enter choose, Escape closes only the list |
| U12a-Q6 | `field-values.txt` beside `settings.txt`, one line per value, written safely when changed; only on this computer |
| U12a-Q7 | Recent, My Values, Add to My Values, No values yet, tooltip "Recent and saved values"; English (GR14), in the glossary |
| U7-Q1 | Two: the histogram and statistics now, the clipping overlay next, on the same clipping level |
| U7-Q2 | Three channels R, G, B drawn over each other, partly transparent; no luminance curve |
| U7-Q3 | After black subtraction and the camera's white balance, scaled to the white level, with an sRGB-like curve so the shadows spread out as in an ordinary histogram |
| U7-Q4 | Clipped: a raw value at or above the white level, before white balance, per channel. Deepest shadows: every channel below 1/256 of the white level (eight stops under clipping) |
| U7-Q5 | The same numbers from the embedded JPEG (or the TIFF) before the screen-profile conversion; clipped at 255, shadows at 0; the panel names the source |
| U7-Q6 | Once per opened image, after its first image is shown, so the first image is not slower; `latency.ps1` checked |
| U7-Q7 | A collapsible "Histogram" section in the preview's right column (under Info and Zoom) and in the inspector (under Metadata), each remembered; the clipped shares per channel and the deep-shadow share under it |
| U7-Q8 | Histogram, RAW data, Embedded JPEG, TIFF, Clipped, Deep shadows, Reading …, No histogram for this file; English (GR14), in the glossary |
| U7c-Q1 | The inspector's Histogram uses the preview's statistics when its last frame showed the same image and mode; otherwise the reader of U7c-Q2 |
| U7c-Q2 | A reader thread of its own in `pw-loupe`, with the Exif reader's rules (shown, 200 ms, one in flight, newest wins), opening the file with the same `NativeBackend` and dropping the session at once; not the view core |
| U7c-Q3 | The last 64 results by path and mode, in memory; another folder empties them; a changed file is read again |
| U7c-Q4 | A metadata write of the file being read waits for the read; the result is kept |
| U7c-Q5 | A wanted 100 % view of the current image is picked before its statistics in the view core; `latency.ps1` before and after, the 100 % run reported. **Owner, 2026-11-15 (T144):** "wanted" includes the 100 % view the plan prefetches while the image is shown at fit (`Want::Target`), not only the shown view at 100 %, so Z pressed while the statistics would run finds that view ready; the T142 test helper `open_and_show` puts that view on the screen too. T144's 100 % run (7.5 → 10.0 ms median) is accepted: the step to a neighbour finds its 100 % view already prefetched, so that run does not show the case this decision is for |
| U7c-Q6 | No new texts: T142's, a failed read as "No histogram for this file" |
| U7c-Q7 | A kept histogram is not read again after RawStride's own metadata write of its file (owner, 2026-11-15, T145): a write does not change the pixels, so a kept result whose stamp equals the file's stamp just before that write takes the file's new stamp; a file changed in any other way is still read again (U7c-Q3) |
| U7b-Q1 | Four tasks: T146 the map from the RAW data (C++ shim, `pw-libraw`); T147 the view core asks for it and gives it with the frame, and makes it for JPEG and TIFF; T148 the keys, menu items, settings, section and texts in the preview; T149 the overlay drawn on the image; the owner's hand check after T149 |
| U7b-Q2 | Clipped: a part where at least one channel is at the clipping level, by T142's rule (RAW: a raw value at or above the white level, before white balance; JPEG/TIFF: 255); the map keeps which channel, so a colour per channel can come later |
| U7b-Q3 | Deep shadows: T142's rule, every channel below 1/256 of the white level above black (JPEG/TIFF: every channel 0), so the marks are what the panel's number counts |
| U7b-Q4 | Clipped highlights solid red, deep shadows solid blue, drawn over the image; nothing else of the image changes |
| U7b-Q5 | **J** clipped highlights on/off, **Shift+J** deep shadows on/off; both in the View menu, ticked when on, with their keys; they act in the preview |
| U7b-Q6 | In the preview only; each overlay stays on when stepping and is remembered after a restart (`settings.txt`) |
| U7b-Q7 | One mark per cell of 2×2 photosites (3×3 for X-Trans) in RAW mode, 2×2 pixels in JPEG and TIFF mode |
| U7b-Q8 | Computed only while an overlay is on, for the current image after its first view (as U7-Q6); the maps of the last 4 images shown kept; nothing marked until the map is ready; the overlay is one GPU texture of 4 bytes per cell, outside the view's texture budget |
| U7b-Q9 | A section "Clipping" under Histogram in the preview's right column, collapsible and remembered, with check boxes "Clipped highlights (J)" and "Deep shadows (Shift+J)"; "Reading …" while the map is computed, "No clipping map for this file" when there is none |
| U7b-Q10 | Clipping, Clipped highlights, Deep shadows, Show Clipped Highlights, Show Deep Shadows, No clipping map for this file; "Reading …" as today; English (GR14), in the glossary |
| U6-Q1 | Five tasks (four as approved; T152 split by the reviewer's cold read, 2026-11-16): T150 the view core takes a zoom and position from outside and gives its position in the frame; T151 the compare model, settings and texts in `pw-loupe`; T152 the second view core and the image area drawn by one function; T153 the second image drawn in the preview with the keys and menu items (zoom free); T154 the locked zoom and position; the owner's hand check after T154 |
| U6-Q2 | The held image is shown by a second view core, started with the program, given a list of the held image alone while comparing and an empty list otherwise; the first core and its stepping are unchanged; a metadata write releases the file in both cores |
| U6-Q3 | The second core gets a quarter of the first one's budgets (by default 1 GiB for sessions and 256 MiB for textures) |
| U6-Q4 | **C** in the preview: the current image becomes the held one and the current image steps to the next shown image (the previous one at the end); with "only the marked" and two marked images those two are compared; with one shown image C does nothing and the menu item is greyed. **(Owner, 2026-11-16, T153 review:)** with "only the marked" on and the current image not marked, the current image is not in the stepped list, so C does nothing and the item is greyed even with two or more marked images (T151's `open_indexes`, accepted as it is) |
| U6-Q5 | The held image left (side by side) or on top (above/below); the current image right or below, framed in the accent colour. **(Owner, 2026-11-16, T153 review:)** the accent colour of the current side's frame, and of the held cell's dashed frame in the filmstrip, is the dark theme's `MARKED_BLUE` in both themes, as the image area is dark in both and the light theme's accent is below 3:1 there |
| U6-Q6 | **K** holds the current image; **Shift+K** swaps the held and the current image |
| U6-Q7 | Side by side by default; **V** switches to above/below and back; remembered in `settings.txt` |
| U6-Q8 | **L** locks and unlocks zoom and position; locked by default, remembered. Locked: the held image shows the current image's zoom level (fit or the same scale) and the same relative point; zoom and drag on either side move both. Unlocked: the keys zoom the current image, the mouse acts on the side under the pointer |
| U6-Q9 | Ratings, labels, `M`, the title line and the right column follow the current image only; **R** switches both sides; the clipping marks are drawn on both sides |
| U6-Q10 | Each side has a line at its top: file name, source (G1) and zoom; the rating badge on both sides; the filmstrip frames the held image's cell with a dashed frame |
| U6-Q11 | **C** again or **Escape** closes the compare and keeps the current image in the preview; a second Escape closes the preview; closing the preview, changing tab or opening a folder closes the compare; compare is not remembered after a restart, the layout and the lock are. **(Owner, 2026-11-17, T165:)** when the step that C or Shift+K makes does not happen (the target dropped by the refilter before the step, for example after a rolled-back rating write), C leaves the compare closed and Shift+K leaves the held image as it was, so the two sides never show the same image by accident; a filmstrip click on the held image is a real step and stays as it is |
| U6-Q12 | View menu items "Compare" (C), "Hold Current Image" (K), "Swap Images" (Shift+K), "Above and Below" (V), "Lock Zoom and Position" (L), acting in the preview only; the side line "{file} · {source} · {zoom}"; Compare and Held image in the glossary |
| U9-Q1 | **(Owner, 2026-11-17, all of U9-Q1–U9-Q10:)** U9 is split: U9a focus marking (can start at once), U9b move on reject or select (after R2, the companion file names read on the owner's real files). Focus marking: key **E** in the preview, on ↔ off, remembered; View menu item "Show Focus Marking" |
| U9-Q2 | Edges of high local contrast in the luminance of the shown image, computed at the full resolution of the preview image that is shown (the half-size render, the same pixels the view draws at fit), not from the RAW data |
| U9-Q3 | One colour (green; red was approved first and changed by the owner the same day, since the clipped highlights are red, U7b-Q4) and one threshold, tuned on the owner's test files (U-Q7) |
| U9-Q4 | Can be on together with the clipping overlay; drawn on both sides of the compare; in the preview only, not in the contact sheet |
| U9-Q5 | One check box "Move Rejected and Picked Images to Subfolders" in the Image menu, off by default, remembered in `settings.txt` |
| U9-Q6 | With the setting on, **X** sets rejected and moves the image to `_rejected`; **P** moves it to `_picked` without changing the rating (XMP has no pick value). With the setting off, P does nothing and its menu item is greyed |
| U9-Q7 | In the contact sheet the marked images are moved; each image is moved all or nothing on its own, and the whole batch is undone as one step |
| U9-Q8 | **Ctrl+Z** undoes moves only: the files are moved back and the rating restored; up to 20 steps, for the running session only, one list for the whole program |
| U9-Q9 | In a folder named `_rejected` or `_picked` the keys only set the rating; nothing is moved (no `_rejected\_rejected`) |
| U9-Q10 | The list of companion suffixes (`dop`, `pp3`, `on1` by default) is kept in `settings.txt` for now; its page in Settings comes with U8 or U10b |
| U9-Q11 | (owner, 2026-11-17, the first backlog sort, from the review of T174) A tab open on `_rejected` or `_picked` itself follows a move and an undo as the images' own folders' tabs do (files that came are listed, files that left are dropped); Ctrl+Z while a batch or an undo runs or waits shows in the status line that a move is still running (T183) |
| U11c-Q1 | Person Shown with Details, Product Shown, Artwork or Object, Contributor, CV-Term About Image, Image Registry Entry; not Image Region (needs drawing on the image) nor the rights expressions |
| U11c-Q2 | Content after Person Shown: Person with Details, Product, Artwork; Creator and Rights at the end: Contributor; Assignment after Genre: CV-Term About Image, at the end: Image Registry Entry |
| U11c-Q3 | Title, Creator, Date Created, Source, Source Inventory Number and Copyright Notice open; the other eleven under a collapsed "More" line |
| U11c-Q4 | Lang Alt edits `x-default`; multi-value sub-fields one line split at commas; Artwork's Date Created checked as an IPTC date; Circa free text |
| U11c-Q5 | Not edited; kept unchanged |
| U11c-Q6 | A button "Copy from Person Shown"; nothing synchronised automatically |
| U11c-Q7 | As the other lists; none is a copyright-management field, so an emptied list is emptied on every image |
| U11c-Q8 | IPTC's English names (GR14); "More", "Copy from Person Shown" in the glossary |
| U11c-Q9 | (owner, 2026-11-17, the first backlog sort, from the review of T143) When OK, Save and Next, Save or "Apply to {n} Marked" writes nothing because of a field error (for example an Artwork or Object's Date Created under the collapsed "More"), the window's status line says so too, naming the fields and structure lists with an error; the texts under the fields stay (T181) |

## 7. Steps

Each step ends with tests (GR11), the owner's hand check and, where an image is shown, `latency.ps1`.

| Step | Content |
|---|---|
| **U0** | *Postponed (owner, 2026-11-13):* prototype of the separate preview window in a second egui viewport (glow); V1, V3, V6 and the step time with both windows redrawn measured; immediate or deferred viewport; focus between the windows. Done when the separate window is taken up (§7.2) |
| **U1** | Shell: menu bar (own names, §2.1), command bar with "Öppna mapp" (Ctrl+O), status bar, resizable and collapsible side columns with the hide key, F11; dark and light theme (§2.5); state remembered in `settings.txt`; existing commands moved into the menus. No sorting |
| **U2** | Inspector: Info (Exif summary, new; fields and sources §2.1) and Metadata (every value read-only, read with T103's rules, §2.1); `I` shows and hides the inspector; sections remembered. The `M` window is unchanged apart from previous/next (§7.1) |
| **U3** | Preview as a full-screen mode of the main window (§2.2): the loupe moved there; open and close keys (§2.4); the right column (Info, Zoom); the rating badge and RAW/JPEG control always visible; current image linked with the contact sheet and the selection kept (finding 6); the **filmstrip** (moved from U6, owner 2026-11-13): the contact sheet's thumbnails, order and filter in one row, or only the selected, scrolled to the current image, click to jump, hidden with the hide key; its thumbnails are fetched below the shown image's priority so stepping (V1–V8) is not slowed |
| **U4** | Library: folder tree, favourites, recent folders |
| **U5** | Tasks area: import progress per file, stop button, pending writes, error list |
| **U6** | Compare two images (side by side, above/below, locked zoom); the filmstrip moved to U3 (owner, 2026-11-13) |
| **U7** | RAW histogram and exposure statistics (new `pw-libraw` function), in the preview and the inspector; clipping overlay (§2.6) |
| **U8** | Key map file and its page in Settings |
| **U9** | Focus marking; move on reject or select as a setting, with undo (§2.6); companion file names checked on real files first |
| **U10** | Metadata window: every simple text and date field of the six sections editable, with the PLUS fields (merged with I11; the PLUS vocabulary read first, ADR 0007 §16.4); sections and the user's choice of fields (§2.3.1) |
| **U11** | Metadata window: lists and structures (persons, locations, creators, licensors, releases) with add and remove |
| **U12** | Metadata window: previous/next and "spara och nästa" (the first part in stage A), recent values, copy/paste of all fields, "använd på alla markerade" with the batch rules (§2.3.1) |
| **U13** | Metadata window: templates from the window (with the modes of ADR 0007 §10) and variables in text fields. **Also the import's template (owner, 2026-11-15):** instead of the template fields in the import dialog, a template window of its own (the metadata window's form with each field's mode), opened from the import dialog; the template is saved as an XMP template file (ADR 0007 §10) and chosen by name in the import. Needs the namespace of the modes decided first (as D1 of ADR 0007) |
| **U14** | Sort order in the command bar and the contact sheet |

### 7.1 Order: a usable version first (owner, 2026-11-12)

The owner wants a **usable version early**, to judge from real use which parts should move forward
or back. So the steps are not done in number order:

1. **Stage A – the usable version.** U1, U2 and U3, plus previous/next in today's `M` window (the
   first part of U12, with the rules of §2.3.1). U3 includes the filmstrip (owner, 2026-11-13:
   important; the two-image compare waits in U6). It moves today's features into the new shell
   (menu bar, side columns, inspector, full-screen preview) and adds as little new function as
   possible. Then the owner uses it on real work.
2. **The owner reorders** the remaining steps from that use. This list is only the starting order.
3. **Stage B (proposed order):** U10 and the rest of U12 (the metadata window's freedom, merged with
   milestone 8's I11 so the PLUS fields are built once), U5, U4, U14, U6, U7, U11, U13, U9, U8.

U0 is not part of stage A: the preview is a mode of the main window (owner, 2026-11-13), and the
separate window waits (§7.2).

### 7.2 Not in this ADR (known wishes, kept so they are not forgotten)

| Wish | When (owner, 2026-11-12) | Note |
|---|---|---|
| **GPS**: import a GPX track and match it to capture times (with a time offset), set a position on a map, fill in city, country and location from the position (reverse geocoding) | **Later: the next metadata ADR after this one** | Needs new dependencies (a map view, probably an online place service): an owner decision and a licence check (ADR 0002) |
| **Find and replace** in metadata across many images (fields, case, whole words, file names) | Further out | Uses the safe write of ADR 0007 §9 |
| **Keyword panel**: a flat list of approved keywords, and structured keywords (hierarchy, synonyms, import/export as text) | Further out | Lets RawStride edit the hierarchical keywords it only keeps today (ADR 0007 §8) |
| Tagging (a yes/no mark apart from stars and labels) | Later, after stage A (owner, 2026-11-12) | |
| Loupe under the mouse pointer in the contact sheet (100 % while a key or button is held) | Later, after stage A (owner, 2026-11-12) | |
| Copy, move and rename tools for selected images | Later, after stage A (owner, 2026-11-12) | Taken up by ADR 0021 (2026-11-17) |
| Chosen fields as text lines under each thumbnail | Later, after stage A (owner, 2026-11-12) | The thumbnail shows the file name until then |
| Temporary exposure and white balance for assessment | Later, after stage A (owner, 2026-11-12) | Needs RAW processing options the view core does not have today |
| The separate preview window (U0), movable to a second screen with that screen's DPI and colour profile | When the owner has a second screen to test with (owner, 2026-11-12 and 2026-11-13) | Review finding 5: colour conversion is per start screen today (ADR 0016) |
| **Images shown while importing**: the destination tab opens when the first file is copied and verified, and fills as the import goes on; today it opens when the copy is done (ADR 0010, "Open the result") | Not yet placed (owner, 2026-11-15); to be taken up by ADR 0026 (owner, 2026-11-18) | Changes ADR 0010's rule. The IPTC template is written after the copy, so an image is shown for a while before the template's values are in it; the contact sheet must take the folder's new files without a reload of the tab |
| Code replacements (short codes expanded to text while captioning) | Not yet placed (review, 2026-11-13) | From the study's list of missing functions |
| A catalogue across folders (`pw-catalog`) | Not yet placed (review, 2026-11-13) | From the study's list of missing functions |
| **Load a folder again**: a command that reads the active tab's folder and its ratings and metadata again, so files added or changed by another program are seen | Not yet placed (owner, 2026-11-17, ADR 0019 E-Q12); to be taken up by ADR 0026 (owner, 2026-11-18) | Today a tab is read when it is opened |
| Floating, freely placed panels | Future requirement (requirements-image-viewer.md, "Not covered") | |

## 8. Consequences

- RawStride gets the intended workflow without changing the tested core (apart from the RAW
  histogram function, U7).
- ADR 0009–0011 and D16 keep their behaviour rules; their layout parts were replaced by this ADR
  when it was accepted. ADR 0014 Q5 is replaced for the preview (§2.2), and D16's lock and Escape
  question for the metadata window (§2.3.1).
- `docs/architecture.md` and `docs/README.md` were updated when this ADR was accepted.
