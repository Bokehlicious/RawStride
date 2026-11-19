# Finished tasks

Reviewed and committed. Kept for history; a new task does not need them. The task text is as it
was when the task was done, so "this file", "below" and "the Log" in T1–T115 mean the old
`docs/agent-tasks.md` (split into this folder on 2026-11-13); its Log table is
[log-2026-11-08-to-2026-11-13.md](log-2026-11-08-to-2026-11-13.md).

| Task | Title |
|---|---|
| [T1](T001-fix-the-five-existing-clippy-warnings.md) | Fix the five existing clippy warnings |
| [T2](T002-pw-loupe-use-the-workspace-dependency-for-pw.md) | pw-loupe: use the workspace dependency for pw-metadata |
| [T3](T003-pw-loupe-typed-metadata-state-instead-of.md) | pw-loupe: typed metadata state instead of comparing display text |
| [T4](T004-ingest-summary-only-count-files-that-were.md) | Ingest summary: only count files that were copied or already present |
| [T5](T005-card-eject-in-pw-ingest-safer-powershell-call.md) | Card eject in pw-ingest: safer PowerShell call and a real wait |
| [T6](T006-real-file-tests-part-1-pw-metadata-pw-ingest-pw.md) | Real-file tests, part 1 (pw-metadata, pw-ingest, pw-libjpeg): never pass silently |
| [T7](T007-format-the-files-that-cargo-fmt-check-reports.md) | Format the files that `cargo fmt --check` reports |
| [T8](T008-move-reusable-ingest-helpers-from-src-tauri-to.md) | Move reusable ingest helpers from src-tauri to pw-ingest |
| [T9](T009-eject-polish-readable-exit-status-fast-timeout.md) | Eject polish: readable exit status, fast timeout test, no dead code off Windows |
| [T10](T010-card-eject-try-again-when-windows-is-still-busy.md) | Card eject: try again when Windows is still busy after the import |
| [T11](T011-card-eject-keep-powershell-running-until-the.md) | Card eject: keep PowerShell running until the card is gone |
| [T12](T012-real-file-tests-part-2-pw-raw-pw-viewcore.md) | Real-file tests, part 2 (pw-raw, pw-viewcore) |
| [T13](T013-real-file-tests-part-3-pw-libraw.md) | Real-file tests, part 3 (pw-libraw) |
| [T14](T014-real-file-tests-part-4-pw-rawview-with-libraw.md) | Real-file tests, part 4 (pw-rawview with LibRaw) |
| [T15](T015-a-script-that-runs-the-checks-and-writes-a.md) | A script that runs the checks and writes a readable report |
| [T16](T016-developer-setup-document-what-the-tests-need.md) | Developer setup: document what the tests need |
| [T17](T017-test-inventory.md) | Test inventory |
| [T18](T018-pw-ingest-split-paths-rs-and-restore-the.md) | pw-ingest: split `paths.rs` and restore the explanations |
| [T19](T019-pw-viewcore-run-the-not-a-raw-file-test-in.md) | pw-viewcore: run the "not a RAW file" test in every test run |
| [T20](T020-adr-0010-m1-move-card-detection-and-request.md) | ADR 0010 M1: move card detection and request validation to pw-ingest |
| [T21](T021-adr-0010-m2-folder-tabs-in-pw-loupe.md) | ADR 0010 M2: folder tabs in pw-loupe |
| [T22](T022-pw-ingest-tidy-the-import-api-before-the-egui.md) | pw-ingest: tidy the import API before the egui dialog uses it |
| [T23](T023-adr-0011-k1-thumbnail-extraction-in-pw.md) | ADR 0011 K1: thumbnail extraction in pw-thumbcache |
| [T24](T024-adr-0011-k2-thumbnail-worker-and-in-memory-cache.md) | ADR 0011 K2: thumbnail worker and in-memory cache |
| [T25](T025-adr-0011-k3-grid-model-in-pw-loupe.md) | ADR 0011 K3: grid model in pw-loupe |
| [T26](T026-adr-0011-k4a-pw-thumbcache-skip-duplicate.md) | ADR 0011 K4a: pw-thumbcache — skip duplicate requests, export the image type |
| [T27](T027-adr-0011-k4b-thumbnail-bookkeeping-for-the-grid.md) | ADR 0011 K4b: thumbnail bookkeeping for the grid (plain Rust) |
| [T28](T028-adr-0011-k4c-draw-the-contact-sheet.md) | ADR 0011 K4c: draw the contact sheet |
| [T29](T029-adr-0011-k4d-switch-between-grid-and-loupe-size.md) | ADR 0011 K4d: switch between grid and loupe, size control |
| [T30](T030-adr-0010-m4a-import-form-and-preview-summary.md) | ADR 0010 M4a: import form and preview summary (plain Rust) |
| [T31](T031-adr-0010-m4b-import-worker-background-thread.md) | ADR 0010 M4b: import worker (background thread) |
| [T32](T032-adr-0010-m4c-import-dialog-state-machine-plain.md) | ADR 0010 M4c: import dialog state machine (plain Rust) |
| [T33](T033-adr-0010-m4d-validate-the-import-form-on-the.md) | ADR 0010 M4d: validate the import form on the worker thread (G2) |
| [T34](T034-adr-0010-m5a-card-detection-on-the-worker.md) | ADR 0010 M5a: card detection on the worker, dialog keys and texts (plain Rust) |
| [T35](T035-adr-0010-m5b-draw-the-import-dialog-editing-and.md) | ADR 0010 M5b: draw the import dialog (editing and preview) |
| [T36](T036-adr-0010-m5c-run-the-import-and-open-the-result.md) | ADR 0010 M5c: run the import and open the result in a new tab |
| [T37](T037-adr-0010-m7a-backup-destination.md) | ADR 0010 M7a: backup destination |
| [T38](T038-adr-0010-m7b-job-name-and-first-running-number.md) | ADR 0010 M7b: job name and first running number |
| [T39](T039-adr-0010-m7c-iptc-template-on-the-new-copies.md) | ADR 0010 M7c: IPTC template on the new copies |
| [T40](T040-adr-0010-m7d-eject-the-card-after-the-import.md) | ADR 0010 M7d: eject the card after the import |
| [T41](T041-adr-0013-b1-rating-keys-applying-a-key-rating.md) | ADR 0013 B1: rating keys, applying a key, rating texts (plain Rust) |
| [T42](T042-adr-0013-b2-rating-store-and-write-queue-plain.md) | ADR 0013 B2: rating store and write queue (plain Rust) |
| [T43](T043-adr-0013-b3-rating-keys-and-display-in-the-grid.md) | ADR 0013 B3: rating keys and display in the grid and the loupe |
| [T44](T044-adr-0013-b4-finish-pending-rating-writes-on-exit.md) | ADR 0013 B4: finish pending rating writes on exit |
| [T45](T045-adr-0013-b2-follow-up-send-the-folder-read.md) | ADR 0013 B2 follow-up: send the folder read again when the metadata queue is full |
| [T46](T046-adr-0013-5-rounded-inset-label-bar-in-the-grid.md) | ADR 0013 §5: rounded, inset label bar in the grid and the loupe badge |
| [T47](T047-pw-loupe-does-not-start-since-t44-embed-the.md) | pw-loupe does not start since T44: embed the Common Controls 6 manifest |
| [T48](T048-pw-metadata-a-write-that-exiftool-refused-is.md) | pw-metadata: a write that ExifTool refused is reported as saved |
| [T49](T049-swedish-message-when-a-rating-cannot-be-saved.md) | Swedish message when a rating cannot be saved |
| [T50](T050-adr-0013-b6-documentation-after-milestone-4.md) | ADR 0013 B6: documentation after milestone 4 |
| [T51](T051-adr-0014-f1-the-filter-model-and-the-filter-of.md) | ADR 0014 F1: the filter model and the filter of a tab (plain Rust) |
| [T52](T052-adr-0014-f2-multi-selection-model-shift-in-the.md) | ADR 0014 F2: multi-selection model, Shift in the grid key map (plain Rust) |
| [T53](T053-adr-0014-f3-one-rating-key-on-several-images.md) | ADR 0014 F3: one rating key on several images, and the batch failure text (plain Rust) |
| [T54](T054-adr-0014-f4-pw-viewcore-keeps-its-caches-when.md) | ADR 0014 F4: pw-viewcore keeps its caches when the file list changes |
| [T55](T055-adr-0014-f5a-browse-logic-of-a-tab-under-a.md) | ADR 0014 F5a: browse logic of a tab under a filter (plain Rust) |
| [T56](T056-adr-0014-f5b-draw-the-filter-and-the-marking-in.md) | ADR 0014 F5b: draw the filter and the marking in the grid, wire clicks and keys |
| [T57](T057-adr-0014-f6-the-loupe-steps-and-prefetches.md) | ADR 0014 F6: the loupe steps and prefetches within the filter |
| [T58](T058-adr-0014-f8-documentation-after-milestone-5.md) | ADR 0014 F8: documentation after milestone 5 |
| [T59](T059-adr-0014-amendment-1-2-2-a-rating-removes-an.md) | ADR 0014 amendment 1, §2.2: a rating removes an image that stops matching at once |
| [T60](T060-adr-0014-amendment-1-2-1-the-star-condition.md) | ADR 0014 amendment 1, §2.1: the star condition ≥, =, ≤ |
| [T61](T061-adr-0014-amendment-2-filter-on-several-labels.md) | ADR 0014 amendment 2: filter on several labels at once |
| [T62](T062-adr-0009-amendment-1-a-held-step-key-in-the.md) | ADR 0009 amendment 1: a held step key in the loupe waits for the shown image |
| [T63](T063-adr-0015-d1-new-crate-pw-tiff-with-the-tiff.md) | ADR 0015 D1: new crate `pw-tiff` with the TIFF decoder from the Tauri app |
| [T64](T064-adr-0015-d2-the-loupe-core-opens-tiff-files.md) | ADR 0015 D2: the loupe core opens TIFF files |
| [T65](T065-adr-0015-d3-thumbnails-of-tiff-files.md) | ADR 0015 D3: thumbnails of TIFF files |
| [T66](T066-adr-0015-d4-rawstride-lists-tiff-files-and.md) | ADR 0015 D4: RawStride lists TIFF files and names them in the status line |
| [T67](T067-adr-0015-d6-d7-documentation-after-milestone-6.md) | ADR 0015 D6–D7: documentation after milestone 6 |
| [T68](T068-adr-0016-c1-new-crate-pw-colour-for-colour.md) | ADR 0016 C1: new crate `pw-colour` for colour conversion |
| [T69](T069-adr-0016-c2-rawstride-reads-the-monitor-profile.md) | ADR 0016 C2: RawStride reads the monitor profile at start and names it in the window title |
| [T70](T070-adr-0016-c3-tiff-pw-tiff-reads-the-embedded-icc.md) | ADR 0016 C3 (TIFF): `pw-tiff` reads the embedded ICC profile and converts 16-bit data before the reduction |
| [T71](T071-adr-0016-c3-jpeg-pw-libjpeg-returns-the-icc.md) | ADR 0016 C3 (JPEG): `pw-libjpeg` returns the ICC profile, `pw-raw` the Exif colour space |
| [T72](T072-adr-0016-c4-the-loupe-and-the-thumbnails.md) | ADR 0016 C4: the loupe and the thumbnails convert to the monitor profile |
| [T73](T073-adr-0016-5-the-loupe-says-when-an-embedded.md) | ADR 0016 §5: the loupe says when an embedded colour profile cannot be used |
| [T74](T074-adr-0016-c6-documentation-after-milestone-7.md) | ADR 0016 C6: documentation after milestone 7 (colour management) |
| [T75](T075-adr-0009-s6-acceptance-measurement-of-rawstride.md) | ADR 0009 S6: acceptance measurement of RawStride and a results report |
| [T76](T076-adr-0007-i1-the-iptc-property-table-generated.md) | ADR 0007 I1: the IPTC property table, generated from the TechReference |
| [T77](T077-adr-0007-i2-dates-and-times-truncated-dates-no.md) | ADR 0007 I2: dates and times — truncated dates, no invented time zone |
| [T78](T078-adr-0007-i3-the-value-model-values-origins-lang.md) | ADR 0007 I3: the value model — values, origins, Lang Alt, changes |
| [T79](T079-adr-0007-i4-iim-text-rules-blank-values-byte.md) | ADR 0007 I4: IIM text rules — blank values, byte limits, prefixes, character encodings |
| [T80](T080-adr-0007-i5a-the-iim-block-datasets-decoding.md) | ADR 0007 I5a: the IIM block — datasets, decoding, values |
| [T81](T081-adr-0007-i5b-reconciliation-iim-xmp-by-checksum.md) | ADR 0007 I5b: reconciliation — IIM/XMP by checksum, Exif/XMP, Creator, `Cleared` |
| [T82](T082-adr-0007-i6-the-reader-exiftool-output-into.md) | ADR 0007 I6: the reader — ExifTool output into readings, sidecar rules |
| [T83](T083-adr-0007-i7-the-write-plan-xmp-iim-exif-overlap.md) | ADR 0007 I7: the write plan — XMP, IIM, Exif overlap fields, conflicts |
| [T84](T084-adr-0007-i8a-the-file-snapshot-and-the.md) | ADR 0007 I8a: the file snapshot and the verification of a written copy |
| [T85](T085-adr-0007-i8b-windows-file-operations-and-the.md) | ADR 0007 I8b: Windows file operations and the write journal |
| [T86](T086-adr-0007-i8c-the-safe-write-transaction.md) | ADR 0007 I8c: the safe write transaction |
| [T87](T087-adr-0007-i9a-ratings-and-labels-through-the.md) | ADR 0007 I9a: ratings and labels through the write plan and the safe write |
| [T88](T088-adr-0007-i9b-the-ingest-template-with-the-modes.md) | ADR 0007 I9b: the ingest template with the modes Keep, Replace, Append, Prefix and Clear |
| [T89](T089-adr-0007-i9c-the-view-core-closes-the-raw.md) | ADR 0007 I9c: the view core closes the RAW sessions of a file before it is written |
| [T90](T090-adr-0007-i9d-the-real-image-check-previews-with.md) | ADR 0007 I9d: the real image check — previews with `pw-raw`, the DNG opened with LibRaw |
| [T91](T091-adr-0007-i9e-rawstride-writes-ratings-and-the.md) | ADR 0007 I9e: RawStride writes ratings and the ingest template safely |
| [T92](T092-adr-0007-i9f-a-current-iim-checksum-in-tiff-and.md) | ADR 0007 I9f: a current IIM checksum in TIFF and DNG |
| [T93](T093-adr-0007-i9g-the-iim-checksum-stays-current.md) | ADR 0007 I9g: the IIM checksum stays current when no XMP is left |
| [T94](T094-adr-0007-i10a-the-exiftool-version-is-checked.md) | ADR 0007 I10a: the ExifTool version is checked when the metadata worker starts |
| [T95](T095-adr-0007-i10b-the-reader-looks-up-the-two.md) | ADR 0007 I10b: the reader looks up the two sidecar names instead of listing the folder |
| [T96](T096-adr-0007-i10c-validation-warnings-for-edited.md) | ADR 0007 I10c: validation warnings for edited metadata (§11) |
| [T97](T097-adr-0007-i10d-edits-of-single-properties.md) | ADR 0007 I10d: edits of single properties written safely, with the write notices |
| [T98](T098-adr-0007-i10e-the-metadata-worker-reads-all.md) | ADR 0007 I10e: the metadata worker reads all metadata, writes edits and takes new settings |
| [T99](T099-adr-0007-i10f-rawstride-s-settings-file-create.md) | ADR 0007 I10f: RawStride's settings file (create IIM, fallback encoding) |
| [T100](T100-adr-0007-i10g-the-import-dialog-chooses-the.md) | ADR 0007 I10g: the import dialog chooses the mode of each template field, and asks before a Clear |
| [T101](T101-adr-0007-i10h-rawstride-tells-the-user-at-start.md) | ADR 0007 I10h: RawStride tells the user at start when ExifTool is missing or too old |
| [T102](T102-adr-0007-i10i-the-texts-of-the-info-panel.md) | ADR 0007 I10i: the texts of the info panel (values, conflicts, notices) |
| [T103](T103-adr-0007-i10j-the-info-panel-in-rawstride.md) | ADR 0007 I10j: the info panel in RawStride |
| [T104](T104-adr-0007-i10k-the-editor-s-form-with-its-fields.md) | ADR 0007 I10k: the editor's form with its fields, conflict choices, edits and warnings |
| [T105](T105-adr-0007-i10l-the-editor-window-in-rawstride.md) | ADR 0007 I10l: the editor window in RawStride, saved through the worker |
| [T106](T106-adr-0007-i10m-the-settings-window-create-iim.md) | ADR 0007 I10m: the settings window (create IIM, fallback encoding) |
| [T107](T107-adr-0007-i10n-one-metadata-window-locked-to-its.md) | ADR 0007 I10n: one metadata window locked to its image (D16) |
| [T108](T108-adr-0007-i10o-keywords-and-creator-on-one-line.md) | ADR 0007 I10o: keywords and creator on one line, the reason of a refused write (owner's check of T107) |
| [T109](T109-adr-0017-u1-the-shell-menu-bar-command-bar.md) | ADR 0017 U1: the shell – menu bar, command bar, status bar, side columns, themes |
| [T110](T110-adr-0017-u2-the-inspector-info-exif-summary-and.md) | ADR 0017 U2: the inspector – Info (Exif summary) and Metadata, `I`, sections remembered |
| [T111](T111-adr-0017-u3-the-preview-as-a-full-screen-mode.md) | ADR 0017 U3: the preview as a full-screen mode, its right column and the filmstrip |
| [T112](T112-adr-0017-u12-first-part-previous-next-and-spara.md) | ADR 0017 U12, first part: previous/next and "spara och nästa" in the metadata window |
| [T113](T113-adr-0007-4-2-the-camera-s-rating-0-inside-a-raw.md) | ADR 0007 §4.2: the camera's rating 0 inside a RAW file counts as no rating |
| [T114](T114-names-yrkestitel-and-kontaktuppgifter-no-origin.md) | Names "Yrkestitel" and "Kontaktuppgifter", no origin line under read-only values |
| [T115](T115-latency-ps1-read-the-screen-with-dxgi-desktop.md) | `latency.ps1`: read the screen with DXGI Desktop Duplication, so the full-screen preview can be measured |
| [T116](T116-move-the-loupes-ui-texts-into-text-modules.md) | Move pw-loupe's remaining UI texts into text modules |
| [T117](T117-translate-the-loupes-ui-texts-to-english.md) | Translate pw-loupe's UI texts to English |
| [T118](T118-english-messages-in-the-lower-crates.md) | English messages in the lower crates |
| [T119](T119-same-folder-one-tab.md) | The same folder opens in one tab, however its path is written |
| [T121](T121-one-check-for-swedish-letters-and-test-names.md) | One check for Swedish letters; test names that say English |
| [T122](T122-robust-free-space-test.md) | A free-space test that does not fail by chance |
| [T123](T123-metadata-window-every-simple-field.md) | ADR 0017 U10, first part: every simple field of the metadata window, in sections |
| [T124](T124-rating-and-label-in-the-metadata-window.md) | ADR 0017 U10c: Rating and Label in the metadata window |
| [T125](T125-copy-and-paste-all-fields.md) | ADR 0017 U12b: Copy and paste all fields between images |
| [T126](T126-apply-to-all-marked.md) | ADR 0017 U12c: Apply to all marked |
| [T127](T127-level-c-real-file-tests.md) | ADR 0007 I12: real-file tests with the level-C samples |
| [T128](T128-licence-section-plus-choice-lists.md) | ADR 0017 U10d with ADR 0007 I11: the Licence (PLUS) section with fixed choice lists |
| [T129](T129-iim-value-over-the-limit-is-no-conflict.md) | ADR 0007 §5.1: an IIM value equal to the whole XMP text is no conflict |
| [T130](T130-hide-empty-structure-fields.md) | Empty structure fields are not shown as values |
| [T131](T131-metadata-window-centred-on-image-area.md) | The metadata window opens in the middle of the image area |
| [T132](T132-keep-the-sidecar-name.md) | A write keeps the sidecar's name as it is on disk |
| [T133](T133-milestone-8-documentation.md) | ADR 0007 I13: documentation after milestone 8 |
| [T134](T134-hierarchical-keywords-and-deprecated-iim-kept.md) | ADR 0007 §12: hierarchical keywords and deprecated IIM datasets kept after a write |
| [T135](T135-library-folders-favourites-recent.md) | ADR 0017 U4: the Library: folder tree, favourites and recent folders |
| [T136](T136-locations-as-lists-of-structures.md) | ADR 0017 U11a, first part: Location Created and Location Shown in the metadata window |
| [T137](T137-cargo-audit-in-the-check.md) | `cargo audit` in the check script |
| [T138](T138-sort-order.md) | ADR 0017 U14: sort order in the command bar and the contact sheet |
| [T139](T139-genre-and-plus-structures.md) | ADR 0017 U11a, second part: Genre and the PLUS structures in the metadata window |
| [T120](T120-exiftool-from-the-program-folder.md) | ExifTool from the program folder first |
| [T140](T140-tasks-area.md) | ADR 0017 U5: the Tasks area: import progress, stop, pending writes and errors |
| [T141](T141-recent-and-own-values.md) | ADR 0017 U12a: recent values and the user's own values per field in the metadata window |
| [T142](T142-histogram-and-exposure-statistics.md) | ADR 0017 U7, first part: the RAW histogram and exposure statistics |
| [T143](T143-persons-artwork-products-contributors.md) | ADR 0017 U11, last part: persons with details, artwork, products, contributors and the other structures |
| [T144](T144-inspector-histogram.md) | ADR 0017 U7, the inspector's Histogram: statistics of its own, and the 100 % view first |
| [T145](T145-histogram-kept-after-own-writes.md) | ADR 0017 U7, the inspector's Histogram kept after RawStride's own metadata writes |
| [T146](T146-clipping-map-of-raw-data.md) | ADR 0017 U7b: the clipping map of the RAW data |
| [T147](T147-clipping-map-in-the-view-core.md) | ADR 0017 U7b: the clipping map in the view core |
| [T148](T148-clipping-overlay-in-the-preview.md) | ADR 0017 U7b: the clipping keys, menu items and section |
| [T149](T149-clipping-marks-drawn-on-the-image.md) | ADR 0017 U7b: the clipping marks drawn on the image |
| [T150](T150-locked-view-in-the-view-core.md) | ADR 0017 U6: a view set from outside in the view core |
| [T151](T151-compare-model.md) | ADR 0017 U6: the compare model, its settings and texts |
| [T152](T152-second-view-core.md) | ADR 0017 U6: the second view core, and the image area drawn by one function |
| [T153](T153-compare-in-the-preview.md) | ADR 0017 U6: two images in the preview |
| [T154](T154-locked-zoom-in-compare.md) | ADR 0017 U6: locked zoom and position in the compare |
| [T155](T155-arrows-in-the-proportional-font.md) | The arrows drawn as boxes: Hack as the last font of normal text |
| [T156](T156-eject-a-usb-fixed-drive.md) | Eject a card reader that Windows sees as a fixed USB disk |
| [T157](T157-incremental-ingest-register.md) | Incremental ingest: the register on the card (pw-ingest) |
| [T162](T162-fat32-daylight-saving-shift.md) | A FAT32 card read one hour off after a change of the clocks (pw-ingest) |
| [T159](T159-raf-reader-in-pw-raw.md) | Fujifilm RAF in pw-raw: previews, size and Exif |
| [T158](T158-incremental-ingest-in-the-dialog.md) | Incremental ingest in the import dialog (pw-loupe) |
| [T160](T160-raf-listed-everywhere.md) | Fujifilm RAF listed: viewer, import and sidecar |
| [T164](T164-same-names-in-favourites-and-recent.md) | Folders with the same name told apart in Favourites and Recent (pw-loupe) |
| [T163](T163-open-every-imported-folder.md) | After an import, every folder that got images opens in its own tab (pw-loupe) |
| [T161](T161-import-dialog-finds-new-cards.md) | The import dialog finds a card inserted while it is open |
| [T165](T165-compare-after-a-step-that-did-not-happen.md) | The compare after a step that did not happen (pw-loupe) |
| [T166](T166-mark-a-card-already-imported.md) | Mark a card whose images are at the destination already (pw-loupe) |
| [T167](T167-focus-map-in-the-view-core.md) | ADR 0017 U9a, first part: the focus map in the view core (pw-viewcore) |
| [T168](T168-focus-marking-key-and-drawing.md) | ADR 0017 U9a, second part: the focus marking key, menu item and drawing (pw-loupe) |
| [T169](T169-move-an-image-with-its-companions.md) | ADR 0017 U9b, first part: moving an image with its companion files (pw-metadata) |
| [T170](T170-move-request-in-the-metadata-worker.md) | ADR 0017 U9b, second part: the move as a request to the metadata worker (pw-metadata) |
| [T171](T171-move-settings.md) | ADR 0017 U9b, third part: the two move settings (pw-loupe) |
| [T172](T172-move-rules-and-undo-list.md) | ADR 0017 U9b, fourth part: the move rules, the undo list, the tab's list and the texts (pw-loupe) |
| [T173](T173-move-events-in-the-metadata-cache.md) | ADR 0017 U9b, fifth part: the move requests and answers in the shell's metadata part (pw-loupe) |
| [T174](T174-move-keys-menu-and-undo.md) | ADR 0017 U9b, last part: the keys, the menu items, the moves and the undo in the shell (pw-loupe) |
| [T175](T175-other-products-named-by-the-clean-room-rule.md) | Other products named only as the clean-room rule allows (docs) |
| [T176](T176-focus-map-time-on-real-files.md) | ADR 0017 U9a: the focus map's time, and whether it slows stepping and Z (pw-viewcore tests) |
| [T177](T177-release-before-a-move-without-the-write-stamp.md) | ADR 0017 U9b: the release before a move notes no write stamp (pw-metadata, pw-imagecheck, pw-loupe) |
| [T178](T178-move-waits-for-the-write-answer.md) | ADR 0017 U9b: a move waits for its rating write's answer, and an image is not moved twice (pw-loupe) |
| [T179](T179-add-and-remove-list-entries-in-an-edit.md) | ADR 0017 U12d: an edit that adds and removes entries of a list (pw-metadata) |
| [T180](T180-apply-to-marked-adds-and-removes-list-entries.md) | ADR 0017 U12d: "Apply to {n} Marked" adds and removes list entries (pw-loupe) |
| [T181](T181-nothing-saved-text-on-a-field-error.md) | The metadata window's status line says when nothing was saved because of a field error (pw-loupe) |
| [T182](T182-x-trans-statistics-gaps.md) | RAW statistics on X-Trans: channel shares, renders and a file cut short (pw-libraw tests) |
| [T183](T183-tabs-on-rejected-and-picked-and-undo-while-moving.md) | ADR 0017 U9b: a tab open on `_rejected` or `_picked` follows a move and an undo; Ctrl+Z while a move runs says so (pw-loupe) |
| [T184](T184-release-for-move-a-required-method.md) | `ImageCheck::release_for_move` a required method (pw-metadata, pw-imagecheck) |
| [T185](T185-delete-group-to-recycle-bin.md) | ADR 0018 B1: an image's files to the Recycle Bin, with the drive and the files checked first (pw-metadata) |
| [T186](T186-delete-requests-in-the-metadata-worker.md) | ADR 0018 B2: preparing and deleting an image as requests to the metadata worker (pw-metadata) |
| [T187](T187-delete-rules-and-texts.md) | ADR 0018 B3, first part: the delete's plan, batch and texts in the shell (pw-loupe) |
| [T188](T188-delete-requests-and-answers-in-the-metadata-cache.md) | ADR 0018 B3, second part: the delete requests and answers in the shell's metadata part (pw-loupe) |
| [T189](T189-delete-key-menu-window-and-wiring.md) | ADR 0018 B4: the Delete key, the menu item, the confirmation window and the deletes in the shell (pw-loupe) |
| [T190](T190-delete-rejected-images.md) | ADR 0018 B5: "Delete Rejected Images" in the active tab's folder (pw-loupe) |
| [T191](T191-remove-the-unused-cleared-path.md) | ADR 0007 Q2 (revision 19): the unused `Cleared` path removed (pw-metadata, pw-loupe) |
| [T192](T192-exif-imageeditor-and-contributor.md) | ADR 0007 §6.6: Exif ImageEditor ↔ the Contributor whose role is content editor (pw-metadata) |
| [T193](T193-lists-with-qualifiers-not-written.md) | ADR 0007 §8: a list whose items have qualifiers is shown with a notice and never written (pw-metadata, pw-loupe) |
| [T194](T194-batch-line-for-a-qualified-list.md) | ADR 0007 §8: the batch names a list with qualifiers that was not written in another image (pw-loupe) |
| [T195](T195-scene-subject-and-genre-codes-checked.md) | ADR 0007 §11: Scene Code, Subject Code and Genre checked against IPTC NewsCodes, warnings only (pw-metadata, pw-loupe) |
| [T196](T196-camera-gps-fills-location-created.md) | ADR 0007 §6.7: camera GPS fills the coordinates of Location Created, never written (pw-metadata) |
| [T197](T197-region-render-on-raf.md) | ADR 0027 W0: the region render on RAF, an X-Trans region processed in a wider box (pw-libraw, C++ shim) |
| [T198](T198-batch-lines-for-every-notice.md) | ADR 0017 U12d-Q10: the batch names every write notice of the other images, the match over the notices without a `_` arm (pw-loupe) |
| [T199](T199-checksum-notice-names-both-reasons.md) | ADR 0007 §8: the notice that no new IIM checksum was written gives a reason that holds for an unresolved conflict and a held list with qualifiers (pw-loupe) |
| [T200](T200-external-program-list.md) | ADR 0019 E1: the list of external programs in `settings.txt` and its section "External Programs" in the Settings window (pw-loupe) |
| [T201](T201-open-in-program-plan-wait-and-texts.md) | ADR 0019 E2, first part: the command line's length, the plan (start, ask, too long), the wait for the images' writes, the start of the program and the texts of "Open in {name}" (pw-loupe) |
| [T202](T202-open-in-program-menu-key-and-wiring.md) | ADR 0019 E2, second part: Image ▸ "Open in {name}" per program or "Open in Another Program…", Ctrl+E, the question above 100 images, the wait for the images' writes and the start in the shell (pw-loupe) |
| [T203](T203-show-in-explorer.md) | ADR 0019 E3: Image ▸ "Show in File Explorer" for the current image, and the owner's hand check of E2 and E3 (pw-loupe) |
