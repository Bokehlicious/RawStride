# T110. ADR 0017 U2: the inspector – Info (Exif summary) and Metadata, `I`, sections remembered — Done

- **Can start now.** The owner approved the proposals of U2-Q1–U2-Q5 (at the end of this task) on
  2026-11-13; they are written into the points below. Not at the same time as any other task that
  changes `main.rs`.
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/lib.rs` (`Action::ToggleInfo`
  and the doc comment of `key_action` only); `crates/pw-loupe/src/metadata.rs` (the inspector's
  read, point 4); new `crates/pw-loupe/src/inspector.rs` (the Exif summary, its reader and the
  inspector's state; pure functions where possible); new `crates/pw-loupe/src/inspector_text.rs`
  (every new text of this task, rule 13); `crates/pw-loupe/src/shell.rs` and
  `crates/pw-loupe/src/shell_text.rs` (the Visa menu, point 6, and the removed placeholder line);
  `crates/pw-loupe/src/settings.rs` (the new keys, point 7); `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.1 (Inspector), §2.3 (metadata read-only next to the image), §2.4 (`I`) and
  §7 U2, the second step of stage A (§7.1). T109 made the right column with the heading
  "Inspektör" and a placeholder line; this task fills it. **The only new function is the Exif
  summary;** the metadata values are the ones the `M` window already shows.
- **Already decided (do not change):** the right column **starts collapsed** (owner, 2026-11-13,
  T109: default `right_column = collapsed`; a saved `right_column = open` still opens it). Nothing
  in this task opens the column at start or on its own.
- **Not in this task:** the histogram section (U7); the preview's right column with Info and Zoom
  (U3: in U2 the loupe stays in the central area, and the right column is the same in the contact
  sheet and the loupe); editing in the inspector (editing stays in the `M` window, §2.3);
  previous/next in the `M` window (the first part of U12, its own task); the `M` window itself is
  unchanged.
- **What:**
  1. **The current image** (ADR 0017 §2.1, §2.2): in the loupe the shown image; in the contact
     sheet the current cell (the one with the frame), also when several images are marked (the
     inspector shows the current image, not the selection). No current image (an empty tab):
     `InspectorText::NoImage`. At the top of the inspector the file name of the current image
     (wrapped, never cut).
  2. **Info section (new; U2-Q1, U2-Q2):** an Exif summary of the current image, read with
     `pw_raw::inspect` for **every file type RawStride lists** (`pw_raw::SUPPORTED_EXTENSIONS`:
     ARW, RW2, DNG, NEF, CR2, ORF, PEF, SRW, JPEG and TIFF); no ExifTool. `pw-raw` reads the
     values from the TIFF/Exif IFDs of RAW and TIFF files, from the embedded JPEG's Exif where a
     RAW keeps it only there (RW2), and from APP1 Exif in a JPEG. Rows, in this order, each only
     when the file has the value:
     - **Kamera:** make and model; the make is left out when the model already begins with it
       (case-insensitive; "Sony ILCE-7RM5", not "SONY ILCE-7RM5" twice);
     - **Objektiv:** the lens model;
     - **Brännvidd:** `35 mm`, `24,5 mm` (decimal comma, at most one decimal, `,0` dropped);
     - **Bländare:** `f/2,8`, `f/8`;
     - **Exponeringstid:** `1/250 s`, `0,5 s`, `2 s` (the rounding of `ExposureTime`'s `Display`
       in `pw-raw`, with a decimal comma; `pw-raw` is not changed);
     - **ISO:** `ISO 400`;
     - **Tagen:** DateTimeOriginal as `2026-10-30 14:03:12` (a value that is not
       `YYYY:MM:DD HH:MM:SS` is shown as stored).

     A file with none of the values shows `InspectorText::NoExif`; a read error shows
     `InspectorText::ExifError(reason)`; while reading `InspectorText::ExifLoading`. The labels
     are not IPTC properties and do not come from `property_label`; they are made in
     `inspector_text.rs` (U2-Q4). The formatting is pure functions in `inspector.rs` from
     `pw_raw::ImageInfo` to a list of rows, tested without files.
  3. **The Exif read** must not slow stepping (V1–V8): it runs on a background thread (one thread
     owned by the reader, `std::sync::mpsc`; no new dependency), with **the rules of T103**: a read
     is asked for only while the right column is open and the Info section is open, and after the
     image has been the current one for **200 ms**; at most **one read in flight**; when the image
     changes while a read runs, the newest image is asked for when it comes back; a result for
     another image is dropped. The result of the shown image is kept until the image changes; a
     loaded folder change empties it. The reader is behind a small trait (as `MetadataSubmit`), so
     the tests use a fake.
  4. **Metadata section:** every value the image has, with its origin, read-only, **as the `M`
     window shows them**: the values (the 22 fields of D12 and the rest), the conflicts (open and
     decided; only when there are any, in the warning colour) and the notices (only when there are
     any), built with T102's `info_panel` and headed by `PanelText::Values`, `Conflicts` and
     `Notices` (as T103). `PanelText::Loading` while loading, `PanelText::ReadError` on an error,
     `UNSUPPORTED_TEXT` for a file kind without metadata support. Values and labels wrap; no fixed
     text width.
     **The read** goes through `MetadataWorker::read_metadata` (T98) with **the rules of T103**
     (ADR 0017 §2.1), as its own state in `metadata.rs` beside the `M` window's read (T107), which
     is unchanged: only while the right column and the Metadata section are open; after 200 ms on
     the same image; at most one in flight; the newest image asked for after it; stale results
     dropped; read again after `RatingWritten` or `EditsWritten` for the image (so a save in the
     `M` window shows at once); emptied by a loaded folder change and pruned as the window's read
     is. While the `M` window is open the inspector keeps showing its image and asks for no new
     read until the window closes.
  5. **`I` shows and hides the inspector** (U2-Q3, ADR 0017 §2.4 and §4): `Action::ToggleInfo`
     changes meaning in the contact sheet and the loupe: the right column open ↔ collapsed
     (the same function as Visa ▸ Inspektör, point 6). When the side columns are hidden with `H`,
     `I` brings both back and opens the right column. **Today's meaning of `I` ends:** the status
     line is always shown in full, and the rating badge and the RAW/JPEG control in the loupe are
     always shown (`show_info` is removed). `I` still does nothing while a text field has the
     keyboard or a modal window is open. The doc comment of `key_action` says the new meaning.
  6. **Visa menu** (`shell.rs`): the check items "Information (I)" and "Höger kolumn" become
     **one** check item "Inspektör (I)", checked when the right column is open (U2-Q3). The menu
     test of T109 (every key text gives its command through the key functions) still passes.
     "Vänster kolumn" and "Sidokolumner (H)" are unchanged. The placeholder line under "Inspektör"
     (`ShellText::InspectorNote`) is removed with its test line.
  7. **Sections** (§2.1, §2.5): Info and Metadata are collapsible headers
     (`egui::CollapsingHeader` or the same look as T109's column headers), Info above Metadata, the
     whole column in one scroll area. Their state is remembered in `settings.txt` (D10) with the
     rules of T109's keys (an unknown value keeps the default and gives one problem naming the
     line; a file without the keys gives the defaults and no problem; every save writes all keys):
     `inspector_info` and `inspector_metadata` (`open`, `collapsed`), **both `open` by default**
     (U2-Q5). Saved at exit as T109's layout. A collapsed section asks for no read.
- **Rules:** rules 3–7 and 10–13. No `Cargo.toml` change (`pw-raw` and `pw-metadata` are already
  dependencies of `pw-loupe`). `pw-raw`, `pw-metadata` and the `M` window's behaviour are not
  changed. Texts in `inspector_text.rs`; `PanelText`, `panel_text`, `info_panel`,
  `property_label` and `UNSUPPORTED_TEXT` are reused, not copied. V1–V8 apply: stepping in the loupe
  with the inspector open must not be slower than with it collapsed.
- **Tests to add:** the Exif rows from `ImageInfo` (every row; the make left out when the model
  begins with it, and kept when not; decimal comma in focal length, aperture and exposure time;
  `1/250 s`, `0,5 s`, `2 s`; the date reformatted, and a malformed date shown as stored; missing
  values give no row; no values at all gives `NoExif`); the Exif reader with a fake (no read
  before 200 ms, none while the column or the section is collapsed, one in flight, the newest image
  after it, a stale result dropped, emptied by a folder change); the inspector's metadata read in
  `metadata.rs` with the existing fake submitter (the same five rules, a new read after
  `RatingWritten` and `EditsWritten` of the image, none while the `M` window is open, and the
  window's read unaffected); `I` in `key_action` (with and without Ctrl; Ctrl+I stays import);
  the menu table with "Inspektör" and without "Information" and "Höger kolumn"; `settings.rs`: the
  two keys round-trip, a bad value of each gives its default and one problem naming the line, a
  file without them gives `open` with no problem; one test of every text of `inspector_text`
  (as the test of `shell_text`).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: `I` and
  Visa ▸ Inspektör open and collapse the inspector in the contact sheet and the loupe, and `I`
  brings it back after `H`; Info on an ARW, an RW2, a DNG, a JPEG and a TIFF (the values against
  ExifTool or Photo Mechanic); Metadata as in the `M` window on a JPEG, a DNG and an ARW with a
  sidecar, and a conflict (`konflikt-stad.jpg`); a save in the `M` window shown in the inspector at
  once; holding an arrow key through a folder queues no reads (the inspector catches up when the
  key is released); after a restart the right column and the two sections are as they were;
  `latency.ps1` stepping in the loupe with the inspector open as in T75.
- **Questions (answered by the owner, 2026-11-13: every proposal approved):**
  - **U2-Q1** The source of Info (point 2): `pw_raw::inspect` for every file type, on a background
    thread, without ExifTool? (ExifTool would give more fields but is slower and shares the worker
    with the metadata reads.)
  - **U2-Q2** The rows and their format (point 2): the ADR's seven values; the camera without a
    repeated make; a decimal comma; the date as `2026-10-30 14:03:12`; a missing value gives no
    row. Exposure compensation (which `pw-raw` also reads) and the image size are left out of U2.
  - **U2-Q3** `I` and the menu (points 5 and 6): `I` opens and collapses the right column (also
    from `H`'s hidden state); "Information" and "Höger kolumn" become one item "Inspektör"; the
    status line, the rating badge and the RAW/JPEG control are always shown, so today's `I` ends
    already in U2 (the ADR's §4 places it with the preview).
  - **U2-Q4** Rule 13 says property names come only from `property_label`. The Exif labels
    (Kamera, Objektiv, …) are not IPTC properties: may they be made in `inspector_text.rs`?
  - **U2-Q5** Both sections open by default (point 7)? The column itself starts collapsed (T109).
