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
   The syntax is our own, not Photo Mechanic's.
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
4. **IPTC template on ingest:** creator, copyright (+ `xmpRights:Marked`), credit, e-mail,
   telephone, web. IPTC Core in XMP, stored like ratings (ADR 0004): sidecar for RAW, embedded
   for DNG/JPEG/TIFF. Only non-empty fields are written; written to the new copies only.
5. **No videos.** Ingest copies still images only (and their sidecars); `.MP4`, Sony
   `PRIVATE/M4ROOT` and audio notes stay on the card. This is a decision, not a gap.

## Dialog (src-tauri/src/ingest.rs, ui/web/ingest.js)

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
