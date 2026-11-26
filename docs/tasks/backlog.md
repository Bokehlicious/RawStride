# Backlog

Ideas and work that wait for the owner's decision, an ADR or a new dependency before any task is
written. Not for agents.

- Accepted limits and leftovers ("a task only if it is seen") are in
  [`known-limits.md`](known-limits.md), by area.
- Leftovers that a planned step must take up are in that step's notes in `plans/`.
- An item that becomes a task, an ADR or a step, or is dropped, is removed from this file; git
  history and `done/` keep it. Nothing is struck and left here.

Reorganised on 2026-11-21 (owner): the earlier file mixed these three kinds and kept done items.

## Waiting for a decision or an ADR

- **The camera's brightness worked out from the embedded JPEG** (W5's hand check, 2026-11-26;
  ADR 0027 §4.4): the file's own value (DNG `BaselineExposure`, the Fujifilm value) exists only
  in some formats and matches the camera's JPEG only roughly. An offset worked out by comparing
  the embedded JPEG's histogram with the RAW view's would serve every format (ARW and RW2
  included). A new design with its own questions (which part of the histogram, the JPEG's tone
  curve, the cost per image); an ADR question before any task.
- **The auto exposure rule's target** (W5's hand check, 2026-11-26; ADR 0027 §3.4, §4.4): the
  owner may want the auto-exposed view a little brighter than the 99.9th percentile 1/3 EV
  below clipping; to be tuned after T338–T340, when the owner has used auto exposure at start
  for a while.
- **Fine details stand still while stepping quickly** (T328's hand check, 2026-11-25): with
  fine details on, quick steps leave the main image standing still until the user pauses; with
  them off the image changes almost at once. The same before T328 (a build without it), so not
  caused by the stop. Likely cause: the new image's fine render starts as soon as it is opened
  and takes its only session and every core, while the neighbours are asked for only after the
  dwell. A possible fix: start the fine render only after the dwell. The owner (2026-11-25) will
  hardly use fine details and thinks what was wanted was rather a mask that shows the fine
  details; a question for the owner before any task (ADR 0027 §4.2 said no more work on fine
  details unless seen in use).
- **Steps that stall at 51.2 % with the clipping marks on** (T328's latency run, 2026-11-25): in
  `s6\a1` at 51.2 %, steps 2 and 6–10 of 13 changed nothing on the screen within 8 s (about
  2.8 s of CPU and 270 MB each), with and without sharpening, and the same with the view core
  from before T328 and `pw-libraw` from before T327; with `clip_highlights` and `clip_shadows`
  off every step was seen (4.8–5.3 ms). T326's run earlier that day saw every step, the owner
  says with clipping on, though its CPU per step (250–650 ms) matches the run without; not
  explained. The owner sees no stall stepping by hand. T329's run (2026-11-25, same folder, with
  T329) saw every step at 51.2 % and 64 % with clipping on (5.2–7.2 ms). A question of its own
  (`latency.ps1` with clipping on and off, the clipping job's place in `pick`).
- **TIFF files stutter when dragged between 50 % and 100 %** (T329's hand check, 2026-11-25):
  TIFF files in a folder viewed in RAW mode stutter badly when the user drags them, also after
  the view has stood still for 1 s, while RAW files drag smoothly then. Not known whether it was
  so before T329 (the first drag stood still on every image then). Not looked into; the pan view
  (`State::pan_view`) has no limit on the source, so a TIFF's pan view (its decode or size) or
  its ring are where to look first. A question of its own (which levels, how large the files,
  JPEG mode too, a build before T329).

- **Custom buttons in the template window's questions** (found at T293's review, 2026-11-23):
  `rfd` is built without its `common-controls-v6` feature, so "Discard the changes to the
  template?" and "{file} already exists. Replace it?" show OK and Cancel instead of Discard
  / Replace and Cancel; both answers work. Turning the feature on changes `pw-loupe`'s
  `Cargo.toml`, and the test binaries, which get no Common Controls 6 manifest, would then
  import `TaskDialogIndirect` and fail at start as before T47. Needs the owner's decision
  (keep OK/Cancel, or a task that turns it on and gives the tests the manifest). **Kept** at the
  sort after U13c, 2026-11-23 (owner): OK/Cancel for now; a later task would take every Windows
  question of the program, not only the template window's.
- **Swedish names for every IPTC property** (owner, 2026-11-12, after ADR 0007 D11): every
  property and structure field in T102's name table, so the Swedish info panel and editor show no
  English IPTC names. Today D11 names about 35 of them; the rest fall back to IPTC's English name.
  Needs the owner's approval of the full list; would become the second language's catalog below.
- **A second language: a text catalog with keys** (owner, 2026-11-14). Only when a second language
  is wanted (GR14: no language choice for now). Done already by T116–T121: every text the user
  sees is made by a function in a text module (rule 13), whole sentences from one template, the
  tests check the texts through those functions, and one test keeps Swedish letters out of the
  code. Left: one catalog with keys in place of the text modules; typed kinds instead of finished
  English sentences from the lower crates (`pw-ingest`'s form checks and eject results, the error
  texts of `pw-metadata`, `pw-viewcore`, `pw-thumbcache`); the tests then check the key or the
  catalog, no weaker than today. Needs an ADR first: a library (Fluent, rust-i18n; a new
  dependency, the owner's approval) or our own simple table.
- **Editing a list whose items have qualifiers, with the qualifiers kept** (ADR 0007 §8; T193 only
  keeps such a list unwritten). Needs RawStride to edit the XMP packet itself, since ExifTool
  13.59 cannot change or remove a qualified item. The limits until then are in
  `known-limits.md`, "Lists with qualifiers".
- **A filter bar of toggle buttons instead of drop-down lists** (owner idea, 2026-11-10, after T61;
  ADR 0014 §15): a row "0★ 1★ 2★ 3★ 4★ 5★" and a row of colour swatches, each clicked on and off,
  with a check box per row that turns the row on or off. Needs an amendment of ADR 0014 first: the
  star part would become a set of star values instead of the ≥ / = / ≤ condition (T60).
- **Value columns over the catalog** (owner, 2026-11-26, during S4's hand check; ADR 0030 K5,
  ADR 0022): columns side by side, for example Capture Date, Camera, Lens and File Type, each
  listing the catalog's values with the number of images for each and an "All" row at the top
  (the number of values and of images). Several values picked in one column mean "or", picks in
  different columns "and", and a pick narrows the other columns' counts. Capture Date as a tree
  of years, months and days. **Only in the catalog, not in a folder tab** (owner, 2026-11-26): a
  folder holds one shoot or day, so its columns would hold one or two values each, and the
  "Filter…" window's "is one of" rows cover a folder; over the catalog the values are in the
  database, so the counts are cheap queries. To build on the filter's model (a column as an "is
  one of" row) so a folder version stays possible if it is ever wanted. Open questions, settled
  with K5's questions: where the columns are shown (a panel above the contact sheet or the
  "Filter…" window); which fields get a column, and whether the user picks them; whether the
  columns and the window's rows are one filter; how picks change the other columns' counts; the
  date tree.
- **Rotate items in the right-click menu** (cold reads of T235–T237, 2026-11-20): ADR 0024 names
  only the Image menu, and the right-click menu's items and order are ADR 0019's (E-Q17). Needs an
  ADR addition if the owner wants them.
- **Smaller functions, not important** (owner, 2026-11-17, after ADR 0018–0025): each needs an
  ADR with the owner's answers before any task. Choosing images on the card before an import;
  importing from several cards at once; spell checking in the metadata window; a custom sort order
  (dragging images into an order of the user's own, beside U14's keys); printing and slide shows;
  uploading. Deliberately left out, not to be taken up: video (ADR 0005 choice 5) and deleting
  from the card (ADR 0005 choice 3, ADR 0018 §2.6).

## Technical work

- **A stop that LibRaw feels sooner: `setCancelFlag()`** (T327, 2026-11-25; ADR 0027 W5-Q1): the
  progress handler is asked only between LibRaw's stages, so a stop takes 220–350 ms on a
  whole-image PPG render of an ARW and up to 1.3 s on an X-Trans RAF (its demosaic has no ask).
  LibRaw's `setCancelFlag()` is checked inside some of its loops (`checkCancel`, `libraw.h`) and
  might stop sooner; it most likely still frees the RAW data (the same exception). The owner's
  question if a stop is felt late; T328's hand check (2026-11-25) did not show one apart from
  fine details standing still (above).
- **A setting for the number of ExifTool write processes** (T268, ADR 0007 §9.6 point 2: "may
  become a setting later"): the app uses four (`WRITE_PROCESSES` in `pw-loupe`'s `main.rs`,
  the owner, 2026-11-22, after T267); a memory card, a network share and a hard disk were not
  measured. A task if a user needs fewer or the owner asks.
- **The shipped ExifTool at least `MIN_EXIFTOOL_VERSION`** (chain review of T270–T272,
  2026-11-22): nothing checks that the version `tools/package-dist.ps1` ships (and CI pins) is not
  below the app's lowest version. A guard in the script or a test; a tooling change, so a task.
- **The comment of `examples/digest_and_restart.rs`** (T272's review, 2026-11-22): its module
  comment in `pw-metadata` still says in the present tense that the safe write makes a second
  call for the checksum and restarts after a TIFF write; both are dropped since T272 (ADR 0007
  §9.7). Comment only; fix it with the next task that touches the example.
- **The work folder of `examples/check_timing.rs` and `examples/dng_damage.rs`** (T273's review,
  2026-11-22): both make a `--work` folder before they check that it is not a source file's
  folder or inside one, so a refused folder is left behind, empty, where the sources are. Check
  first (on the parent of a folder that does not exist yet), then make it; with the next task
  that touches either example.
- **The wrapper `Timed` of `examples/write_timing.rs`** (T275's review, 2026-11-22): it
  forwards only `check_copy`, not `ImageCheck::check_copy_with`, so the example times a DNG
  write with LibRaw's check, which the app leaves out when the original has an image data
  hash (ADR 0007 §9.7 point 4). Forward `check_copy_with` (as `tests/real_files.rs` does)
  with the next task that touches the example or measures the write again.
- **A timeout when ExifTool hangs** (process supervision across threads). Today only the shutdown
  has a timeout (`SHUTDOWN_TIMEOUT` in `pw-metadata`'s `exiftool.rs`). A worker that never answers
  also holds a delete, a move and an "Open in" request (`known-limits.md`).
- **RAF previews** (review of T159/T160, 2026-11-16): a RAF has only a 160 px thumbnail and a
  4416 px JPEG, so `pw-thumbcache` decodes the large JPEG for each contact-sheet thumbnail. If the
  contact sheet is found slow, a task decides between a scaled decode of the large JPEG and a
  cached mid-size preview. T160's hand check (owner, 2026-11-17) had only four RAF files and showed
  no problem; waits for a folder with many RAF files.
- **ORF and CR2 in `pw-raw`** (survey, 2026-11-17; owner: no need now, the cameras are gone).
  Both are listed in `SUPPORTED_EXTENSIONS` and LibRaw 0.22.3 renders both, but `pw-raw` falls
  short. Sample files: the owner's five ORF (OM-1, E-M1 Mark II) and three CR2 (EOS-1D X Mark II)
  in the `RAW-ny` subfolder of the test-data folder, kept out of `PW_TESTDATA` until fixed.
  - *ORF is refused*: `inspect_tiff` accepts only TIFF magic 42 and 0x55; Olympus uses 0x4F52
    (`IIRO`, older 0x5352). Accepting the magic is not enough: IFD0 is the uncompressed raw data
    and the only large preview sits in the Olympus maker note (ExifIFD → MakerNote 0x927C →
    CameraSettings 0x2020 → PreviewImageStart/Length 0x0101/0x0102, offsets from the maker-note
    start), which `pw-raw` does not read today.
  - *CR2 shows only the 160 px thumbnail*: IFD0 holds a 5472 × 3648 JPEG as a strip with
    Compression 6 but no PhotometricInterpretation tag, and `inspect_tiff` requires YCbCr for strip
    previews. A small change; the candidate is validated as a JPEG anyway.
  - Then: fixed cases in `pw-raw/tests/real_files.rs`, the files moved into `PW_TESTDATA`, and
    `orf`/`cr2` in the LibRaw tests' extension filters.
  - Later formats (CR3 above all; also NRW, RWL, 3FR, IIQ) need sample files; raw.pixls.us has
    them (mostly CC0, check each file's licence) and LibRaw already renders them.
- **Linux and macOS ports** (owner, 2026-11-17: only if there is interest now that the repository
  is public; the owner uses only Windows; the README's "Building" section says so). Keep putting
  Windows code behind `#[cfg(windows)]` with a `not(windows)` branch, as now. Survey of the code,
  2026-11-17, nothing built off Windows:
  - *Build*: `rust-toolchain.toml` lists only the MSVC target; `.cargo/config.toml`,
    `pw-libraw/build.rs` and `pw-libjpeg/build.rs` assume the `x64-windows-static-md` triplet and
    `*.lib` names (`lib*.a` on Unix; `ws2_32` Windows only). LibRaw's OpenMP (ADR 0006): `libgomp`
    on Linux; Apple clang has none, so `libomp` or an ADR 0006 exception on macOS. `pwlr.cpp` takes
    a `wchar_t*` path and the Rust side sends UTF-16; on Unix `wchar_t` is 32 bits and LibRaw's
    wide-path stream is Windows only, so a UTF-8 `char*` path is needed or no RAW opens.
    `tools/check.ps1` and the `.cmd` scripts are Windows only.
  - *The safe write does not work off Windows*: `pw-metadata`'s `fileops.rs` returns `Unsupported`
    for `free_space`, `is_read_only_volume`, `is_locked`, `replace` and `move_new`, so nothing can
    be saved. Unix versions: `statvfs`, `rename` + `fsync` of the folder, `renameat2`
    (`RENAME_NOREPLACE`) / `renamex_np` (`RENAME_EXCL`). `is_locked` has no Unix equivalent
    (advisory locks only): an ADR 0007 decision. Likely a new dependency (`libc` or `rustix`).
    Six test files in `pw-metadata/tests/` are `#![cfg(windows)]`.
  - *Worse off Windows today*: settings, journal and field values need `LOCALAPPDATA` (nothing is
    saved without it; XDG / `~/Library/Application Support`); the drive tree is empty; eject is
    Windows only (`diskutil`, `udisksctl`); the monitor profile falls back to sRGB (ColorSync;
    colord or X11 `_ICC_PROFILE`; ADR 0016); `local_offset_seconds` in `tasks.rs` is always UTC;
    5 shortcuts use `modifiers.ctrl` where macOS wants Cmd (`modifiers.command`). The moves'
    limits on case-sensitive file systems are in `known-limits.md`.
  - *GUI*: eframe's `wayland`/`x11` and rfd's `xdg-portal`/`gtk3` are off (GUI_REQUIREMENTS §6) and
    one of each is needed on Linux. glow works on macOS (OpenGL 4.1, deprecated); GUI_REQUIREMENTS
    §7's timings are Windows-only measurements.
  - *Distribution*: `docs/distribution.md` (RawStride's portable zip, T228) is Windows only. macOS
    needs an `.app`, signing and notarization (Apple Developer account), `libomp.dylib` and a Mac
    for the hand checks; Linux a tarball or AppImage. ExifTool shipping (ADR 0002 §2.5) per
    platform.
  - *Size (estimate)*: Linux about 1–1.5 weeks (build, Unix fileops and settings folder, tests on
    Unix and CI, eject and monitor profile); macOS about as much again, mostly packaging. Ongoing
    cost: every later change to files, cards or windows tested on three platforms.

## Watched

- **`cargo audit` warns of unmaintained crates** (`tools/check.ps1`; updated 2026-11-19, seen in
  T213's run): `zbus_names` 4.3.5, `zvariant` 5.15.1 and `zvariant_derive` 5.15.1 are marked
  unmaintained (RUSTSEC-2026-0370, -0371, -0372). Warnings, not vulnerabilities, so the check
  passes. They come only through `eframe` 0.37 → `egui-winit` → `accesskit_winit` →
  `accesskit_unix` (the Linux screen-reader bridge, D-Bus): in `Cargo.lock` but not built on
  Windows (`cargo tree --offline -i zvariant` finds nothing without `--target all`). Nothing to
  do on our side but wait for an `eframe` / `accesskit` release that moves off them, and take it
  in a normal egui update; until then the warning shows in the public CI log (plan
  `public-release.md` §3).

## Reserved for outside contributors

GitHub issues marked `good first issue` (owner, 2026-11-20); no agent task, and a task in the same
code leaves them alone.

- [#2](https://github.com/Bokehlicious/RawStride/issues/2): the metadata window's "Apply to {n}
  Marked" is greyed by a field error without saying why (owner's hand check of T181, 2026-11-17);
  a text on hover over the disabled button.
- [#3](https://github.com/Bokehlicious/RawStride/issues/3): `pw_metadata::Error::ReadOnly` and
  `Error::NotWritten` are no longer built by production code (since T211); remove them.
- [#4](https://github.com/Bokehlicious/RawStride/issues/4): a new program icon; changing it is
  replacing `crates/pw-loupe/assets/rawstride.ico` and `rawstride-128.png`, no code change (T226
  brought back the old Tauri app's icon from M1).
