# ADR 0005: Ingest engine

- Status: accepted (the owner chose the proposals below, 2026-11-04)
- Date: 2026-11-04

## Context

Ingest (copying a card to disk, renaming, backing up) is half of the product's purpose
("inbyggd ingest"), and `docs/architecture.md` already assigns it to `pw-ingest` with two goals:
every copy verified, the source card never modified.

## Decision (implemented)

1. **Three steps: scan → plan → execute.** The plan is computed without writing anything, so the
   UI can show exactly what will happen (names, conflicts, bytes) before the user starts.
2. **Shots, not files.** Files in the same folder with the same stem (RAW + JPEG + `.xmp`) form a
   shot and get one capture time, one running number and one new name, so pairs stay paired.
   A sidecar without its image is not copied.
3. **Capture time** comes from EXIF `DateTimeOriginal` through `pw-raw` (the same fast reader as
   the viewer, no ExifTool round trip per file). Fallback: the file date, in UTC.
4. **Templates** with `{year} {year2} {month} {day} {hour} {minute} {second} {date} {time}
   {make} {model} {name} {seq} {seq:N} {job}`, for the folder (may contain `/`) and the file name.
   Values are made Windows-safe (illegal characters, trailing dots, `CON`/`COM1` …).
   The syntax is RawStride's own.
5. **Never overwrite.**
   - A destination file with the same size and modification time counts as already ingested and
     is skipped. Re-running an interrupted ingest copies only what is missing.
   - A different file with the same name gives the whole shot a suffix (`_1`, `_2`, …). This also
     covers two cameras that both write `IMG_0001`.
6. **Verified copy.** Each file is written to `name.pwpart`, flushed (`sync_all`), given the
   source's modification time, read back and compared with a hash of what was read from the
   source, and only then renamed. A failure or cancel leaves no final-named file.
   The source is opened read-only and read once.
7. **Backup destination** gets the same structure and names. Each destination is checked on its
   own, so an existing primary copy plus an empty backup copies to the backup only.
8. **No new dependencies.** The hash is std's SipHash (`DefaultHasher`); dates are computed by hand.

## Measured (sandbox, release build)

| What | Result |
|---|---|
| Scan of 4 RAWs incl. EXIF | < 1 ms |
| 302 MB (4 RAWs) to two destinations | 2.1 s, ~144 MB/s written incl. read-back |
| SipHash throughput | ~1.35 GB/s |

The sandbox disk is not the PC. Measure on thanos with a real card before tuning:
`cargo run --release -p pw-ingest --example ingest -- <card> <dest> --go`.

## Known limits

- The read-back may come from the OS file cache, so it proves that the bytes written match the
  bytes read, but not that the disk stored them. An unbuffered read-back (Windows
  `FILE_FLAG_NO_BUFFERING`) would make it stronger, at the cost of speed.
- Sequential copy, one file at a time. With CFexpress (~800 MB/s) a pipelined reader and writer
  and a faster hash (xxh3 or BLAKE3 crate) may be needed. Measure first.
- Videos and audio notes are never copied (see choice 5).

## Owner's choices (2026-11-04)

1. **Default templates:** folder `{year}/{date} {job}`, file name `{name}` (the camera's name).
2. **Recognising earlier ingests:** at the destination, by name and modification time; for images
   also by size, except DNG/JPEG/TIFF, whose size changes when ratings or IPTC are written into
   them (the date is kept). An existing sidecar counts as ours when the shot's images are already
   there, because it is edited after ingest. A log or the catalog may come later.
3. **Never delete from the card.** Ejecting is an option in the dialog, done only after a
   complete ingest (Windows "Eject" verb through PowerShell, then a check that the card is gone).
   **Changed by the owner (2026-11-16, T156):** a CFexpress reader on USB (seen: "ABT CFX Card
   Reader") is a *fixed* disk to Windows and has no "Eject" verb, so the eject always failed. A
   removable drive keeps the "Eject" verb; a fixed drive on USB is ejected as "Safely Remove
   Hardware" does (`CM_Request_Device_EjectW` on the USB device that holds the disk); any other
   drive is not ejected. The owner approved `windows-sys` 0.61.2 (already in the workspace) as a
   dependency of `pw-ingest` for this.
4. **IPTC template on ingest:** creator, copyright (+ `xmpRights:Marked`), credit, e-mail,
   telephone, web. IPTC Core in XMP, stored like ratings (ADR 0004): sidecar for RAW, embedded
   for DNG/JPEG/TIFF. Only non-empty fields are written; written to the new copies only.
   In RawStride the template has the modes of ADR 0007 §10 (Keep, Replace, Append, Prefix, Clear;
   T88, T100) and is written through the safe write (T91); the Tauri app is unchanged (D7).
5. **No videos.** Ingest copies still images only (and their sidecars); `.MP4`, Sony
   `PRIVATE/M4ROOT` and audio notes stay on the card. This is a decision, not a gap.
6. **Incremental ingest (owner, 2026-11-16; T157, T158).** An option "copy new photos only"
   skips what an earlier import already took from the same card, whatever the destination and
   the names were then.
   - **The register is on the card:** RawStride's own text file `.rawstride-ingest.txt` in the
     root of the source's drive (UTF-8, a version line, then one line per file: path below the
     root, size, modification time). It follows the card between computers and destinations, and
     formatting the card in the camera clears it. Photo Mechanic's file on the card is never read
     (clean room).
   - **Written as the import goes:** a line is added after each file is copied and verified to the
     destination and the backup; files already at the destination are added at the start. An
     interrupted import therefore remembers how far it came.
   - **The option is on by default** and remembered in the settings. Files in the register (same
     path, size and time) are left out before the plan, so they get no running number, and the
     preview and the summary count them as "imported earlier".
   - **A card that cannot be written** (a locked SD card): the import runs anyway and the summary
     says that the card could not be marked. There is no register on the computer instead.
   - **The same time (owner, 2026-11-16; T162):** for the register and for the destination check
     of point 5 above, a modification time exactly one hour (3600 s) earlier or later also counts
     as the same. FAT32 keeps local time, and Windows converts it with today's daylight saving
     time, so after a change of the clocks every file on such a card reads one hour off; without
     this, the whole card would be imported again (with `_1` names in the old destination).
   - **A card whose images are all at the destination already (owner, 2026-11-17; T166):** for
     example after an import made before the register existed. With "copy new photos only" on, a
     register that is not foreign or unreadable, nothing to copy and at least one file already at the
     destination, the dialog's start button reads "Mark Card as Imported" and the run adds those
     files to the register without copying anything. Eject after import applies as for any
     import; the summary is the usual one, and no tab opens, as no folder got new images.
   - **What changes in choice 3:** RawStride now writes one file of its own to the card. It still
     never changes or deletes an image or any other file there. The check at the destination
     (choice 2) stays.

## Dialog (src-tauri/src/ingest.rs, ui/web/ingest.js)

> **2026-11-07:** this section describes the dialog in the current Tauri app. The GUI moves to
> eframe + egui (`docs/GUI_REQUIREMENTS.md`). The behaviour below is kept, and the dialog is rebuilt
> in egui (GR8). Settings then move from the WebView's local storage to a settings file.

- Opened with **Importera…** or Ctrl+I. A card (drive with `DCIM`) is selected automatically;
  any folder can be chosen instead.
- The plan is recomputed 300 ms after each change (the scan of the card is cached; ↻ reads it
  again). The dialog shows counts, bytes, conflicts and the first 300 lines of the plan. Template
  errors mark the field.
- The copy runs in a background thread (`ingest-progress`, `ingest-done` events). The dialog can
  be hidden while it runs, and a cancel button stops it between chunks.
- At the end: the result (speed, failures, IPTC errors, eject) and **Öppna i visaren**, which
  opens the destination folder with the most files.
- Settings, including the IPTC fields, are remembered in the WebView's local storage. The job name
  is kept only for the session. A real settings file comes with `pw-core` config.
- Tests: `ui/tests/ingest-dialog.test.js` (27 checks against a mocked backend),
  `crates/pw-metadata` `iptc_template_in_sidecar_and_embedded`,
  `crates/pw-ingest` `files_edited_after_ingest_still_count_as_ingested`.

## Dialog in the egui program (`pw-loupe`, ADR 0010, 2026-11-09)

The same behaviour, rebuilt in egui (GR8), accepted by the owner with a real card on 2026-11-09.

- Opened with **Importera…** in the tab bar or Ctrl+I. Detected cards are buttons; "Välj…" opens the
  Windows folder dialog (`rfd`), starting in the typed folder.
- Fields: source, destination, **Backup** (checkbox and folder), folder and file-name templates,
  **Jobb** and **Första nr**, the **IPTC-mall** (Fotograf, Copyright, Credit, E-post, Telefon, Webb)
  and **Mata ut kortet när importen är klar**. Errors are shown under the field they belong to.
- Validation, scan, plan, copy, IPTC writing (ExifTool) and eject all run on one import worker thread
  (`crates/pw-loupe/src/import_worker.rs`); the window never reads the disk for the dialog. Messages
  and texts are those of the Tauri dialog (`pw-ingest` and `import_text.rs`).
- The preview follows 300 ms after the last change; the card is scanned again only when the source
  changes or with **Sök igen**, and always right before copying.
- While copying: progress, **Avbryt**, and **Dölj** to keep using the grid and loupe (the status line
  shows the progress; Ctrl+I shows the dialog again). The card is ejected only after a complete import;
  tabs showing the card are emptied first.
- At the end, every destination folder that received a copied file opens automatically in its own
  tab as a contact sheet, the newest one active (GR12, GR13; owner, 2026-11-16, T163, ADR 0010
  "Open the result"); no tab opens when nothing was copied.
- Settings are kept for the session only (ADR 0010 Q5); a settings file is a later decision.
- Code: `import.rs` (form and validation), `import_worker.rs`, `import_dialog.rs` (state machine),
  `import_text.rs`, `tabs.rs` (`clear_inside`), drawing in `main.rs`. Tests: the `import*` and `tabs`
  unit tests of `pw-loupe` (see `docs/test-inventory.md`).
