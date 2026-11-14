# ADR 0013: Milestone 4 – star ratings and colour labels in the egui program

- Status: **accepted by the owner (2026-11-09, step B0).** The scope (§3), R1–R6 as behaviour
  requirements (§7) and every proposal in §11 are accepted as written. For Q5 the owner added a later
  direction: X will move the image to a `_rejected` folder instead (§13); in this milestone X sets the
  rating "rejected". Nothing is built, installed or added as a dependency by this ADR. Each step in §9
  is its own work part, with its own approval and commit.
- **Implemented (2026-11-10):** steps B1–B5 done (agent tasks T41–T49); B6 is this documentation
  update.
- Date: 2026-11-09
- Related:
  - ADR 0004 (ratings and labels: storage, ExifTool, keys, write order). **Its decisions are carried
    over unchanged**; this ADR only decides how the egui program implements them;
  - [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md) GR6 (keyboard first), GR8 (feature parity), GR11
    (testable);
  - ADR 0009 §3.3 (digits and Ctrl+digits are reserved for ratings and labels);
  - ADR 0011 (milestone 3, the contact sheet; §3.2 and Q3 leave ratings under the thumbnails to this
    milestone);
  - `ui/web/app.js` (today's Tauri behaviour: `setRating`, `handleRatingKey`, `paintLabelBar`).

## 1. Context

- The egui program (RawStride, binary `pw-loupe`) has the loupe, the contact sheet, folder tabs and the
  import dialog. GR8 still lists ratings and labels, filter and multi-selection, IPTC metadata and TIFF
  viewing before the Tauri app can be retired.
- **Already in place:**
  - `pw-metadata`: `ExifTool::write_rating` / `read_ratings` with the ADR 0004 storage rules
    (sidecar for proprietary RAW; embedded for DNG, JPEG and TIFF), and `MetadataWorker`, a background
    thread that owns ExifTool and runs requests one at a time. It already has `write_rating` and a
    `RatingWritten { id, path, rating, result }` event. Its request queue is bounded (8).
  - `pw-loupe`: `MetadataCache` reads the ratings of a tab's folder when the tab is loaded, and the
    loupe's status line shows "Rating: 3 stars, Red" (English).
- **Missing:** keys that set ratings and labels, sending writes, showing the values in the contact
  sheet and as a badge in the loupe, rollback and messages when a write fails.

## 2. Decisions already fixed (ADR 0004, unchanged)

1. Storage: `xmp:Rating` (1–5, −1 = rejected) and `xmp:Label` (Red, Yellow, Green, Blue, Purple).
   Sidecar `<stem>.xmp` for proprietary RAW; embedded with `-overwrite_original_in_place -P` for DNG,
   JPEG and TIFF. The RAW file is never modified.
2. One persistent ExifTool process (here: the existing `MetadataWorker`).
3. Keys: 0–5 or Alt+0–5 stars (0 clears); 6–9 Red, Yellow, Green, Blue; Ctrl+1–5 Red … Purple;
   Ctrl+0 no label. Pressing an image's current label again removes it.
4. The UI updates at once; writes are queued in key order; a failed write is rolled back in the UI and
   reported.

## 3. Scope

### 3.1 In scope

| Area | Content |
|---|---|
| Keys | The ADR 0004 keys, in the grid (current image) and in the loupe (shown image). Digits on the main row and the number pad. Shift+digit and Ctrl+Alt (AltGr) do nothing |
| Writes | Optimistic: the value is shown in the same frame; the write goes through `MetadataWorker`; writes for one image keep key order and the last key wins |
| Read merge | The folder read that runs when a tab is loaded never overwrites a value the user set after the read was sent |
| Failure | The image goes back to its last value that was confirmed on disk; the status line names the file and the error |
| Unsupported files | A rating key on a file that cannot carry ratings shows a message and changes nothing |
| Grid | Each cell shows the stars and the label colour below the thumbnail (§11, Q3) |
| Loupe | A small badge with the stars and label colour on the image, and the value in the status line (§11, Q4) |
| Tabs | Ratings are per file, not per tab: a write that is still running when the tab is switched or closed still completes (or is reported) |
| Exit | Writes still queued when the window closes are completed before the program exits (§11, Q6) |
| Texts | Rating texts in Swedish, as in the Tauri app and the import dialog (§11, Q2) |

### 3.2 Out of scope

- **Filter and multi-selection** (ADR 0004 addendum 2026-11-04); a separate milestone (§11, Q1). The
  write queue (§4.2) is designed so that one key can later target several images.
- Keywords, title, description and other IPTC editing (ADR 0007) (done: ADR 0007 milestone 8,
  2026-11-14; the metadata window of ADR 0017).
- Sorting by rating.
- Moving rejected or picked images to `_rejected` / `_picked` folders (§13; a later decision).
- Noticing changes made by other programs while a folder is open (no file watching). A tab reads the
  values again when it is loaded, as today.
- `pw-catalog` (SQLite). XMP stays the only copy (architecture goal 3).
- Ratings for TIFF in the egui program are written as for the Tauri app, but TIFF viewing itself is a
  later milestone (GR8).

## 4. Architecture

### 4.1 Where the code goes

| Part | Crate | Notes |
|---|---|---|
| Rating key map: key + modifiers → `RatingKey` (`Stars(0..=5)`, `Label(name)`, `ClearLabel`, `Reject` if Q5 is accepted) | `pw-loupe` (library) | Plain Rust next to `key_action` and `grid_key_action`; the loupe's `key_action` keeps returning `None` for digits |
| Applying a key to a value: `0` clears stars, the same label again clears it, reject toggles | `pw-loupe` (library) | Pure function `apply(current: &Rating, key: RatingKey) -> Rating` |
| Rating store and write queue (§4.2) | `pw-loupe` (library), new module `ratings.rs` | Replaces the rating part of `MetadataCache`; the load states stay. No threads, no I/O; tested with a fake worker |
| ExifTool I/O | `pw-metadata` | **Unchanged.** No new request type is needed |
| Drawing: the bar under a thumbnail, the loupe badge, status texts | `pw-loupe` (binary) | egui painter, no new crates |

### 4.2 Rating store and write queue

Per image path (absolute, as today):

- `confirmed`: the last value known to be on disk (from a read or a successful write);
- `shown`: what the UI draws (`confirmed` plus any writes not yet confirmed);
- the writes for that path: queued (not yet sent) and in flight (sent, with a `RequestId`).

Rules:

1. **A key** computes the new value from `shown`, sets `shown`, and queues a write. If the same path
   already has a queued write that was not sent yet, its value is replaced (the last key wins, and
   pressing 1-2-3-4-5 quickly gives one or two writes, not five).
2. **Sending:** each frame, queued writes are sent in order while `MetadataWorker` accepts them.
   `SubmitError::QueueFull` leaves them queued for the next frame (the worker's queue holds 8);
   `SubmitError::Stopped` fails them (rule 4).
3. **Success:** `confirmed` becomes the written value. `shown` is unchanged if more writes for the path
   are pending.
4. **Failure:** `confirmed` is unchanged. If no newer write for the path is pending, `shown` goes back
   to `confirmed`; otherwise the newer write decides. A message names the file and the error.
5. **Read merge:** a `RatingsRead` result sets `confirmed` for each path, but sets `shown` only for
   paths with no queued write and no write sent after that read (request ids increase; the worker is
   serial, so a write sent before the read is already in the read's result).
6. **Tabs:** the store keeps its entries when the active tab changes; a tab load re-reads its folder
   (rule 5 protects pending values). Entries of closed tabs are dropped once they have no pending write.
7. **Embedded files** (DNG, JPEG, TIFF): after a successful write nothing in the egui program needs to
   be forgotten. `pw-viewcore` calls `pw_raw::inspect` each time it opens a file and `pw-thumbcache`
   keeps decoded thumbnails, not file offsets; the image data itself does not change (ADR 0004 §3). This
   is checked by hand in B5, including a DNG shown in RAW mode while it is rated (§10).

### 4.3 Keyboard focus

Rating keys act only when no text field has focus (`egui::Context::wants_keyboard_input()` is false) and
the import dialog is closed. Digits typed into the import form's job name or running number never rate
an image.

## 5. Display

### 5.1 Contact sheet (Q3)

- The row under the thumbnail keeps the file name and gains the stars on its left: `★★★••` for three
  stars, `✕` for rejected, nothing when unrated. The file name is shortened to fit, as today.
- With a label, the row's background is the label colour (the Tauri colours: Red `#e5484d`, Yellow
  `#f5c400`, Green `#3fb950`, Blue `#4c8dff`, Purple `#a371f7`), with dark text where needed for
  contrast.
  - **Later changes:** the label colour is drawn as a rounded, inset bar (a pill) in the grid row
    and in the loupe badge (owner, 2026-11-09, T46). The text on every label pill and labelled
    rating badge is dark (`#181818`), at least 4.5:1 (WCAG AA) on all five colours: red 4.5, blue
    5.6, purple 5.3; before, red, blue and purple had light text below 4.5:1 (owner, 2026-11-13,
    in the review of T109).
- A rejected image's thumbnail is drawn dimmed.
- A cell whose write is still pending looks the same as a confirmed one (no spinner); a failure rolls
  it back (rule 4).

### 5.2 Loupe (Q4)

- A badge in the lower left corner of the image area, in the same style as the grid row (stars on the
  label colour). Hidden when the image has neither stars nor a label.
- The status line keeps the text form: "Betyg: 3 stjärnor, Röd", "Avvisad", "Ej betygsatt".

## 6. Dependencies

- No new crates and no `Cargo.toml` change. `pw-loupe` already depends on `pw-metadata`.
- The star and cross characters come from egui's bundled fonts (`default_fonts`); if a glyph is
  missing, the stars are drawn as shapes instead (decided in B3, no new font).

## 7. Behaviour requirements

Proposed as behaviour requirements now and targets after a first measurement, as in ADR 0011 §7:

| # | Requirement | Target |
|---|---|---|
| **R1** | A rating or label key updates the grid cell or loupe badge in the same frame; the UI never waits for ExifTool (G2) | Checked by hand; no file I/O on the UI thread |
| **R2** | Writes for one image reach the disk in key order and the last key wins, also when keys come faster than ExifTool | Tests with a fake worker; by hand: 1-2-3-4-5 quickly, then reopen the folder → 5 stars |
| **R3** | A failed write rolls the image back to its last confirmed value and names the file | Tests; by hand with a read-only sidecar |
| **R4** | Nothing that was shown as set is lost when the window is closed right after a key | By hand: rate, close at once, reopen → value kept (Q6) |
| **R5** | Stepping in the loupe and scrolling the grid keep the display frame rate while writes run (V1–V8 unchanged) | By hand while holding a rating key and stepping |
| **R6** | Values written by RawStride are read by Lightroom Classic and Photo Mechanic, and theirs by RawStride | By hand in B5 (also closes ADR 0004's open "clean-room level C check") |

Results (2026-11-10, from the log in `docs/tasks/done/log-2026-11-08-to-2026-11-13.md`):

| # | Result |
|---|---|
| **R1** | Checked by hand: the owner's T43 hand check (keys in the grid and the loupe, display) passed |
| **R2** | Tests with a fake worker (T42: coalescing, order across paths, retry on a full queue); checked by hand in T43 (values kept after reopening the folder) |
| **R3** | Tests (T42, T48, T49); checked by hand: a read-only sidecar keeps the old rating and gives "Kunde inte spara betyget för <fil>: filen är skrivskyddad" (T49) |
| **R4** | Checked by hand (T47): rate, close at once, reopen → value kept. The exit wait of T44 lists a write still pending after about 5 s, and a refused write with its Swedish reason, in "Betyg som inte sparades" |
| **R5** | Checked by hand in T43 (holding a digit while stepping keeps the loupe smooth) |
| **R6** | **Photo Mechanic: passed (owner, 2026-11-10)**, with one limit: Photo Mechanic does not show a reject (`xmp:Rating` −1), since its scale is 0–5; the label is shown. Stars and labels go both ways; Photo Mechanic writes the same `xmp:Rating` and `xmp:Label` with Adobe's label names. **Lightroom Classic: not checked** (not available) |

## 8. Testing and acceptance

### 8.1 Automated (`cargo test`)

- Key map: every ADR 0004 key, number pad digits, Shift+digit and Ctrl+Alt+digit give nothing, the
  loupe's `key_action` and `grid_key_action` are unchanged.
- `apply`: 0 clears stars and keeps the label; the same label again clears it; another label replaces
  it; stars keep the label; reject (Q5) toggles.
- Store and queue, with a fake worker: optimistic value; coalescing of unsent writes; order kept across
  paths; `QueueFull` retried next frame; `Stopped` fails and rolls back; failure with a newer write
  pending keeps the newer value; read merge (a read sent before a write does not overwrite it, a read
  sent after does); entries survive a tab switch; an unsupported file is refused without a write.
- `pw-metadata`: already covered by `tests/sidecar.rs` (ADR 0004 verification); no change.

### 8.2 By hand (owner)

- Rate and label in the grid and in the loupe, on ARW, RW2, DNG and a JPEG; reopen the folder and see
  the same values.
- Rate a DNG while it is shown in RAW mode; step away and back; the image and its thumbnail are right
  (§4.2 rule 7, §10).
- Hold a digit key while stepping through the loupe (R5); close the window right after a key (R4).
- Make a sidecar read-only and rate that image (R3).
- Type digits in the import dialog's fields: no image is rated (§4.3).
- Open the rated files in Lightroom Classic and Photo Mechanic, and the other way round (R6).

## 9. Work steps

| Step | Content | Result |
|---|---|---|
| **B0** | Accept this ADR; answer §11; accept R1–R6 | **Done 2026-11-09:** documentation commit |
| **B1** | `pw-loupe`: rating key map and `apply` (plain Rust, tests) | **Done:** T41 |
| **B2** | `pw-loupe`: rating store and write queue (§4.2), replacing the rating part of `MetadataCache`; Swedish status texts (plain Rust, tests with a fake worker) | **Done:** T42, T45 |
| **B3** | `pw-loupe`: draw the grid row and the loupe badge; wire the keys in grid and loupe with the focus rule (§4.3); failure messages in the status line | **Done:** T43, T46 |
| **B4** | `pw-loupe`: complete pending writes on exit (Q6) | **Done:** T44, T47 |
| **B5** | Acceptance by hand (§8.2), including the Lightroom/Photo Mechanic check | **Done** by the owner's hand checks; R6 with Photo Mechanic on 2026-11-10, Lightroom not checked (§7) |
| **B6** | Documentation: architecture.md, test inventory, ADR 0004 (open check closed), GR8 status | **Done:** T50 |

Steps B1–B4 are agent tasks T41–T44 (`docs/tasks/done/`). T47–T49 were found in the hand checks.

### 9.1 Findings during implementation

- **Common Controls 6 manifest (T47).** After T44, `pw-loupe.exe` exited at start (0xC0000139).
  Cargo merges the features of `rfd`, which is shared with the Tauri app's `tauri-plugin-dialog`, so
  `rfd` is built with `common-controls-v6`, and its message box needs Common Controls 6.
  `crates/pw-loupe/build.rs` now embeds the manifest. `check.ps1` cannot catch a binary that fails to
  load, because the test binaries never reference `on_exit`.
- **Refused writes (T48).** ExifTool answers "0 image files updated" when it refuses a write (for
  example a read-only sidecar). `pw-metadata` took that answer as success; it now reads the summary
  counts and reports the write as failed.
- **Swedish reasons (T49).** `pw-metadata` reports the kind of a refused write as `WriteError`
  (`ReadOnly`, `NotWritten`, `Other`); `pw-loupe` turns it into "filen är skrivskyddad" or "filen kunde
  inte skrivas" without comparing text.
- **Label name in the loupe badge (T43).** The badge also shows the Swedish label name; this is not in
  §5.2, and the owner chose to keep it.

## 10. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| A file shown in the loupe is open in our process while ExifTool rewrites it in place (DNG, JPEG) | On Windows the write can fail if the file is open without write sharing | Rust's `File` and the C runtime's `fopen` share read and write by default; checked in B5 with a DNG in RAW mode. A failure is rolled back and reported (R3), never silent |
| A preview read during an embedded write sees moved offsets (ADR 0004, Open) | One failed preview or thumbnail | The next request is correct; accepted in ADR 0004 |
| The worker's queue (8) is smaller than a burst of keys | Writes refused | The UI queue keeps them and retries each frame (§4.2 rule 2) |
| ExifTool hangs | Writes stay pending; exit waits | Exit waits at most a fixed time and then reports the files not written (Q6). A real ExifTool timeout stays "not for agents yet" |
| A folder read finishes after the user has already rated an image | The old value reappears | Read merge rule (§4.2 rule 5), with tests |

## 11. Questions and the owner's answers (2026-11-09)

The owner accepted every proposal below as written. Q5 has an added direction for later (§13).

| # | Question | Decision (the proposal, accepted) |
|---|---|---|
| **Q1** | Do filter and multi-selection belong to this milestone? | **No.** They become milestone 5. This milestone stays small; the write queue already allows several targets per key |
| **Q2** | Language of rating texts | **Swedish**, as in the Tauri app and the import dialog: "Betyg: 3 stjärnor, Röd", "Avvisad", "Ej betygsatt", label names Röd, Gul, Grön, Blå, Lila. The three existing English metadata notices ("Metadata loading…", "Ratings unsupported for this file", "Metadata error: …") are translated in B2 |
| **Q3** | How a grid cell shows ratings | Stars on the left of the file-name row, the row background in the label colour, rejected thumbnails dimmed (§5.1) |
| **Q4** | How the loupe shows ratings | A badge in the lower left corner plus the status-line text; the badge follows the I key (information on/off) |
| **Q5** | A key for "rejected"? ADR 0004 has none; the Tauri app can only show a reject written by another program | **X** sets rejected; X on a rejected image clears it (Lightroom's key). X is free in both the grid and the loupe. 0 also clears it, since it clears the stars. *Owner, 2026-11-09:* later, X is meant to move the image to `_rejected` instead (§13) |
| **Q6** | What happens with pending writes when the window closes | The window closes at once; the program waits up to **5 s** for the queue to empty, then exits. Writes that did not finish are listed in a message box before exit |
| **Q7** | Does a rating key in the grid move to the next image (auto-advance)? | **No**, as in the Tauri app. A setting can come later |

## 12. Consequences

- Rating and labelling work in RawStride the way they do in the Tauri app, closer to GR8. Filter and
  multi-selection (milestone 5), IPTC metadata and TIFF viewing remain before the switch.
- `pw-loupe` gains a tested rating store; `MetadataCache` keeps only the load states.
- `pw-metadata` is unchanged; ADR 0004's storage and key decisions are reused as they are.
- The reserved digit keys of ADR 0009 §3.3 get their planned meaning.

## 13. Later direction: moving rejected and picked images (owner, 2026-11-09)

Not part of this milestone; recorded so that this milestone does not block it.

- The owner's intention is that **X moves the image directly into a `_rejected` folder inside the same
  folder**, the way the owner uses FastRawViewer. Another key would move an image to `_picked` in the
  same way.
- This may become an **option in the menus** (a checkbox, as in FastRawViewer), so the user chooses
  between "X sets the rating rejected" (this milestone) and "X moves to `_rejected`".
- It needs its own ADR before any work: what moves with the image (the `.xmp` sidecar and companion
  files such as a JPEG of a RAW+JPEG pair), undo, name collisions in the target folder, what the tab
  and the contact sheet show after a move, and the key for `_picked`. FastRawViewer is studied only
  from its public documentation and by using it (clean-room levels C and D, `docs/clean-room.md`).
- Consequence for this milestone: the X key goes through the same `RatingKey::Reject` path as the
  other keys, so changing what it does later is a change in one place.

## 14. Photo Mechanic check (R6, 2026-11-10)

Test folder with RW2, DNG and JPEG files; values set in RawStride and read in Photo Mechanic
2026.6, and the other way round. The sidecars were compared afterwards (clean-room level C: only
files the programs write).

- **Same fields.** Photo Mechanic writes `xmp:Rating` and `xmp:Label` with Adobe's names
  ("Yellow"), so stars and labels are read both ways.
- **Reject.** RawStride writes −1 (Adobe's reject). Photo Mechanic does not show it. This supports
  the later direction in §13: moving rejected images to `_rejected` works in every program.
- **Sidecar name.** Photo Mechanic writes `<stem>.XMP` in upper case. Windows does not distinguish
  case, and RawStride reads and updates that file.
- **Photo Mechanic's own fields.** Photo Mechanic also writes `photomechanic:ColorClass` (a number).
  RawStride changes only `xmp:Label`, so after a change in RawStride the number is stale. Photo
  Mechanic showed the `xmp:Label` colour in the test, so this has no visible effect today. If it ever
  does, RawStride could also write `ColorClass`; that would need its own decision.

