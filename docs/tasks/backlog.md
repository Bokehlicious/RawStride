# Not for agents yet

These need design decisions, new dependencies or platform-specific code, and stay with the owner
and reviewer:

- A timeout when ExifTool hangs (process supervision across threads). Today only the shutdown has a
  timeout (`SHUTDOWN_TIMEOUT` in `pw-metadata`'s `exiftool.rs`).
- ADR 0007 (IPTC metadata): milestone 8 is done (I1–I13, 2026-11-14; what is left is in ADR 0007
  §18). Also later:
  editing a
  list whose items have qualifiers with the qualifiers kept (§8; T193 only keeps such a list
  unwritten; needs RawStride to edit the XMP packet itself, since ExifTool 13.59 cannot change or
  remove a qualified item). Also from T193's chain review (2026-11-18): the import's
  template drops a change to such a list without a notice (`apply_template_safely` returns no
  plan notices), and the editor gives no warning before OK that the list will not be written.
  Leftovers from the review of T193 (2026-11-18): `write_plan::plan` itself does not refuse a
  changed list with qualifiers (only its callers in `edit.rs` put it back; `write_rating_safely`
  writes no list); recent values (`main.rs` `recorded`) also record a keyword that was put back;
  the batch's "Written" count also counts an image whose write was `Unchanged`; an `xml:lang` on a
  list item is not in ExifTool 13.59's JSON (so not marked) and is kept by a whole-list write, but
  also given to the new items; T193's hand check (owner, 2026-11-19) did not try a JPEG with IIM,
  a list with qualifiers and an IIM/XMP conflict (no such file at hand), so the held list's
  `DigestNotWritten` is checked only by tests.
  **Sorted after T194 (owner, 2026-11-19):** the misleading text of `DigestNotWritten` is T199, and
  the batch dropping another image's `Truncated`, `IimConvertedToUtf8` and `DigestNotWritten`
  (with the `match`'s `_ => {}` arm, from the review of T194) is T198. Stay, each for a reason:
  the template's missing notice waits for templates in full (ADR 0017 U13); the warning before OK
  is not needed while the info panel's notice and the write notice tell the user; `plan` not
  refusing, because every caller today puts the list back (taken up when a new caller of `plan`
  comes); the recent values, because a value put back is harmless to offer again; the "Written"
  count, because ADR 0017 U12d-Q8 already decides that an unchanged image counts as written; the
  `xml:lang`, because ExifTool 13.59 does not report it and it belongs with editing such lists;
  the hand check of the held list's `DigestNotWritten`, done when such a file is at hand.
  from §10, left out of T88: template sub-fields of lists of
  structures (Location Created, Image Creator, Copyright Owner, Licensor), variables (`{date}` …),
  template files in XMP with the modes (where the modes are stored is decided with U13). Left out of I10: editing several
  images at once and lists of structures are now ADR 0017 U12 and U11.
- **Future improvement (owner, 2026-11-12, after D11):** Swedish names for **every** IPTC property
  and structure field in T102's name table, so the Swedish info panel and editor show no English
  IPTC names. Today D11 names about 35 of them; the rest fall back to IPTC's English name. Needs
  the owner's approval of the full list.
- **RAF previews** (review of T159/T160, 2026-11-16): a RAF has only a 160 px thumbnail and a
  4416 px JPEG, so `pw-thumbcache` decodes the large JPEG for each contact-sheet thumbnail. If
  T160's hand check finds the contact sheet slow, a task decides between a scaled decode of the
  large JPEG and a cached mid-size preview. T160's hand check (owner, 2026-11-17) had only four
  RAF files and showed no problem; too few to judge, so this waits for a folder with many RAF
  files.
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
- **Eject through the device: known limits** (independent review of T156, 2026-11-16; none of
  them affects the owner's readers; **kept** at the sort of 2026-11-17): `reader_device` ejects the disk's direct parent, so a reader
  that is a composite USB device (`USB\VID_…&MI_00`) would be refused; walking up with
  `CM_Get_Parent` to the nearest node with `DN_REMOVABLE` would fix it. `on_usb` uses a buffer of
  exactly `STORAGE_DEVICE_DESCRIPTOR`'s size; a driver that answers `ERROR_MORE_DATA` would make
  the reader count as not USB (not ejected, safe). `CR_FAILURE` (19) is reused for "no matching
  disk" and reads like Windows error 19 in a bug report. The veto-name length is passed as
  `MAX_PATH` instead of the buffer's own length.
- ~~**CI before the repository becomes public**~~ (done: T220, public-release plan C1, and T222, P3, 2026-11-20) (owner, 2026-11-14): a GitHub Actions workflow
  (`.github/workflows/ci.yml`) on `windows-latest` that installs the pinned Rust toolchain, runs
  `vcpkg install` with vcpkg's binary cache (LibRaw built from source takes long), puts ExifTool
  (Windows package, 12.67 or newer) on `PATH`, and runs `.\tools\check.ps1` without `-Ignored`;
  `test-output.txt` kept as an artifact. Wait while the repository is private: Windows minutes
  count double against the free quota, and `check.ps1` already runs before every commit. While
  private, run it only by hand (`workflow_dispatch`) and on pull requests to `main`; on every push
  once public. The first run also tests `docs/building.md` on a clean machine.
- **Review the git history before the repository becomes public** (owner, 2026-11-14): go
  through every commit on every branch to be published (not only the current files) for things
  that should not be public: personal data (names, e-mail addresses, paths with user names),
  keys, tokens or passwords, private test images and their metadata, copyrighted reference specs
  (for example the IPTC files kept outside the repository), and large binaries. Decide per finding
  whether to remove it from the history (a history rewrite of all branches, which changes every
  commit hash and needs the owner's approval) or to leave it; a tool such as `gitleaks` for the
  secrets part would be a new dependency. Do this before the CI item above makes the repository
  public. **Done as public-release plan H1 (2026-11-19)**; the rewrite is H2 there.
- **Linux and macOS ports** (owner, 2026-11-17: only if there is interest once the repository is
  public; the owner uses only Windows). Survey of the code, 2026-11-17, nothing built off Windows:
  - *A small README note when public*: done (public-release plan R8, 2026-11-19), in the
    README's "Building" section. Keep putting Windows code behind `#[cfg(windows)]` with a `not(windows)` branch, as now.
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
    5 shortcuts use `modifiers.ctrl` where macOS wants Cmd (`modifiers.command`).
  - *GUI*: eframe's `wayland`/`x11` and rfd's `xdg-portal`/`gtk3` are off (GUI_REQUIREMENTS §6) and
    one of each is needed on Linux. glow works on macOS (OpenGL 4.1, deprecated); GUI_REQUIREMENTS
    §7's timings are Windows-only measurements.
  - *Distribution*: `docs/distribution.md` (the retired Tauri app's folder) is Windows only, and
    RawStride's own folder (ADR 0002 §4) is not made yet. macOS needs an
    `.app`, signing and notarization (Apple Developer account), `libomp.dylib` and a Mac for the
    hand checks; Linux a tarball or AppImage. ExifTool shipping (ADR 0002 §2.5) per platform.
  - *Size (estimate)*: Linux about 1–1.5 weeks (build, Unix fileops and settings folder, tests on
    Unix and CI, eject and monitor profile); macOS about as much again, mostly packaging. Ongoing
    cost: every later change to files, cards or windows tested on three platforms.
- **A second language: a text catalog with keys** (owner, 2026-11-14). Only when a second language
  is wanted (GR14: no language choice for now). Done already by T116–T121: every text the user
  sees is made by a function in a text module (rule 13), whole sentences from one template, the
  tests check the texts through those functions, and one test keeps Swedish letters out of the
  code. Left: one catalog with keys in place of the text modules; typed kinds instead of finished
  English sentences from the lower crates (`pw-ingest`'s form checks and eject results, the error
  texts of `pw-metadata`, `pw-viewcore`, `pw-thumbcache`); the tests then check the key or the
  catalog, no weaker than today. Needs an ADR first: a library (Fluent, rust-i18n; a new
  dependency, the owner's approval) or our own simple table. The Swedish names of the item above
  would become the second language's catalog.
- **Smaller functions, not important** (owner, 2026-11-17, after ADR 0018–0025): each needs an
  ADR with the owner's answers before any task. Choosing images on the card before an import;
  importing from several cards at once; spell checking in the metadata window; a custom sort order
  (dragging images into an order of the user's own, beside U14's keys); printing and slide shows;
  uploading. Deliberately left out, not to be taken up: video (ADR 0005 choice 5) and deleting
  from the card (ADR 0005 choice 3, ADR 0018 §2.6).
- **`cargo audit` warns of unmaintained crates** (`tools/check.ps1`; updated 2026-11-19, seen in
  T213's run): `zbus_names` 4.3.5, `zvariant` 5.15.1 and `zvariant_derive` 5.15.1 are marked
  unmaintained (RUSTSEC-2026-0370, -0371, -0372). Warnings, not vulnerabilities, so the check
  passes. They come only through `eframe` 0.37 → `egui-winit` → `accesskit_winit` →
  `accesskit_unix` (the Linux screen-reader bridge, D-Bus): in `Cargo.lock` but not built on
  Windows (`cargo tree --offline -i zvariant` finds nothing without `--target all`). Nothing to
  do on our side but wait for an `eframe` / `accesskit` release that moves off them, and take it
  in a normal egui update; until then the warning shows in the public CI log (plan
  `public-release.md` §3). The earlier `chrono` warning (RUSTSEC-2026-054356, 2026-11-17) is gone:
  `chrono` is no longer in `Cargo.lock` (`cargo tree --offline --target all -i chrono` finds no
  package, 2026-11-19).
- **A filter bar of toggle buttons instead of drop-down lists** (owner idea, 2026-11-10, after T61;
  ADR 0014 §15): a row "0★ 1★ 2★ 3★ 4★ 5★" and a row of colour swatches, each clicked on and off,
  with a check box per row that turns the row on or off. Needs an amendment of ADR 0014 first: the
  star part would become a set of star values instead of the ≥ / = / ≤ condition (T60).
- **Before the repository is made public** (survey, 2026-11-17; the README section "How RawStride
  is developed" and general antivirus wording in the agent rules were done then). The licence,
  CONTRIBUTING and the dependency survey are in place, and no secrets were found in the files or
  the history. **Moved to `plans/public-release.md` (accepted 2026-11-19, PR-Q1–PR-Q10)**, which
  now holds every point of the two surveys (2026-11-17 and 2026-11-19): the e-mail address (H2,
  H3), the trademark search (PR-Q3, done), the small fixes (R2, R3, R7; `docs/agent-tasks.md` is
  already a stub), the choices (R5, R6, R8, C1; issue templates after the switch), the GitHub
  settings (P1), the free test files (R9), the measurement tools (R4), the old name in `vcpkg.json`
  (R2) and the local folders (R1). Struck here at R8, 2026-11-19. One point stays here:
  - *Licence notice in the program*: GPL-3.0 §0 and §5(d) ask an interactive program to show
    "Appropriate Legal Notices" (copyright, no warranty, where the licence is), usually in an
    About box. `pw-loupe` has none, and ADR 0002 §4 does not list it. Needed before the first
    binary release, not before the repository is public; an ADR 0002 §4 line first (plan §4).
    Decided (owner, 2026-11-20): ADR 0031 RL-Q9, the "About RawStride" window, step RL1; the
    ADR 0002 §4 line is written.
  - *IPTC's terms for the generated tables* (public-release plan H1, 2026-11-19):
    `crates/pw-metadata/src/iptc_table.rs` (from `tools/iptc-table.ps1`) and `vocabulary.rs` are
    generated from IPTC's TechReference and the NewsCodes / PLUS vocabularies (names, ids and
    codes only); the source and its SHA-256 are named, but no licence or attribution. Check IPTC's
    terms (believed to be CC BY 4.0, not checked) and add the attribution they ask for, in the
    files or `THIRD-PARTY-NOTICES.txt`; at the latest before the first binary release.
- **Known limits of `relocate::companions`** (`/code-review` of T169, 2026-11-17; accepted, the
  moves are Windows only; **kept** at the sort of 2026-11-17 until a Linux or macOS port):
  - *Case-sensitive file systems:* a file whose name differs from the image's only in case is
    skipped as if it were the image, so it does not move with the group.
  - *Links and read errors:* a sidecar that is a symbolic link or junction is not listed (it
    stays behind); one entry that cannot be read fails the whole listing.
- **Leftovers from the review of T174** (`/code-review`, 2026-11-17; findings 1–3 are T178; the
  tab on `_rejected` and Ctrl+Z are T183; the rest **kept** at the sort of 2026-11-17:
  tidying the user does not see, and rare races a move's undo puts right):
  - *The undo's state lives in `main.rs`* (`MoveRun::Undo`, `UndoImage`) without unit tests,
    beside `Batch` in `moves.rs`; an `UndoRun` in `moves.rs` would make it testable. Also
    small: `forget_sort` after an undo clears every tab's sort, not only the listed tabs';
    `send_moves` builds the extension list each frame; the `tab.folder == None` branch in
    `undo_done` cannot run; `replace_tab_files` gives every tab of the active folder
    `ratings_ready` (their files and ratings are the active tab's).
  - *Left by T178* (its cold read): P then X within a frame moves the image to `_picked`
    though rejected (the Pick batch holds it); a RAW skipped (its reject write failed) while its
    JPG partner is sent goes along with the JPG to `_rejected`; a rating write whose answer
    never comes keeps its image's move waiting. From its review: a second P or X on an image
    whose move failed in the running batch is left out until that batch is done (`contains`
    holds it, since a failed answer may be a JPG its RAW partner moved along).
  - *Left by T183* (its cold read, 2026-11-17): a tab on `_rejected` or `_picked` made active
    while a batch runs is loaded again when the batch is done (its position, filter and
    inspector reset); a tab open on a `_rejected` folder that was deleted gives a listing error
    after every batch; an undo that partly fails is not pushed back, so its failed images can no
    longer be undone. From its review (`/code-review`, 2026-11-17): a move or a move back that
    fails and whose rollback also fails (files left in `_rejected` or `_picked`) relists neither
    that folder's tab nor the source tab when no other image of the folder succeeded
    (`relist_tabs` takes only the moved and restored images); the active `_rejected` tab loaded
    again (`load_active_tab`, `begin_load`) while a rating write of one of its images is in
    flight may show that rating as unread for a moment; `undo_done` forgets and follows every
    tab's sort twice when a `_rejected` or `_picked` tab is open, every finished batch with such a
    tab open re-sorts every tab (modification times read again), and `relist_tabs` resolves each
    folder with `same_folder` twice (its pre-check and `tabs_of_folder_mut`).
- **Leftovers from the review of T177** (`/code-review`, 2026-11-17; accepted, nothing breaks
  today; the required method is T184; the design below **kept** at the sort of
  2026-11-17, for when `ImageCheck` is next changed):
  - *A cleaner design:* the stamp is part of the write, not of the release. A write-only hook
    (for example `ImageCheck::before_write`, called only by the safe write) would let `release`
    be the same for writes and moves, and `with_move_release` and the second closure could go.
- **Leftovers from the review of T170** (`/code-review`, 2026-11-17; the histogram stamp is
  T177; the reopening **kept** at the sort of 2026-11-17, not seen in the owner's hand check of
  U9b; the text-only error was moved to ADR 0018 B2 and came back here: the owner decided
  (2026-11-17, ADR 0018 B-Q17) that only the delete's answer gets a structured error now):
  - *A view core may open the file again between the release and the move:* the worker releases
    the image files one by one and then moves them; a render or prefetch in between opens a new
    session, and the move fails with "open in another program". The same gap exists before a
    safe write. If T174's hand check sees it, a task holds the view cores' reopening of the
    group's files until the move's answer.
  - *The move's answer is text:* `Event::Moved` and `Event::MovedBack` carry `MoveError` as a
    `String`, so its `left` (files that could not be moved back) is lost for the undo list.
    Kept (ADR 0018 B-Q17); the delete's answer has a structured `DeleteError` (T186) to copy
    when the moves are next changed.

- **Leftovers from the review of T179** (reviewer and `/code-review` twice, 2026-11-17;
  accepted, none is reached by T180's use of `Edit::Entries`; the arm in `BatchEdits::lists` was
  T180's; **kept** at the sort after U12d, 2026-11-17, as each item says):
  - *`same_value` compares list entries on the IIM prediction:* removing a keyword over 64 bytes
    and adding it cut to 64 bytes is no change and is lost without a notice (as for `Set`). Kept:
    rare (a keyword over 64 bytes) and it is all writing, not U12d; a task if it is seen in use.
  - *Four blank tests:* `apply_entries` uses `trim()` for `add`, `Metadata::set` spaces only
    (`is_empty_text`), `iim::is_blank` spaces and NUL, `template.rs` its own `is_blank`; a
    NUL-only or tab-only entry is treated differently on each path. Kept with the tidying below.
  - *Tidying:* the append rule repeats `template.rs`'s list Append (a shared helper);
    `entries_left_alone` works out afterwards what `apply_entries` skipped (one source would
    keep them in step); the separate empty-result branch beside the `Empty` fallback;
    `contains` in loops (quadratic, small at today's list sizes). Kept until `edit.rs` or
    `template.rs` is next changed (for example U13, templates); none is reached from the window,
    whose list text is trimmed and has no empty entries.
  - *From the chain review of T179 and T180* (owner, ADR 0017 U12d-Q8): an entry with a comma
    ("Lund, Sweden") is split by the window's list text, so removing it removes nothing on the
    other images while the confirmation names it; a failed file of a batch can only be retried by
    making the change again. Kept: known limits in ADR 0017; the comma needs another way to edit
    lists (since T108); a task if either is seen in use.

- **A small fault: "Apply to {n} Marked" greyed without saying why** (owner's hand check of T181,
  2026-11-17): with a field error the button is disabled (`apply_allowed`, U12c-Q2) and nothing
  says why; the error is shown under its field. Nothing can go wrong, since nothing is written
  while a field has an error. A text on hover over the disabled button ("Correct the error in
  …", as `EditorText::NotSaved` names the items) would say why. Not important.

- **Known limits of the delete to the Recycle Bin** (T189's brief and its reviews, 2026-11-18;
  accepted when T189 was done; **kept** at the sorts after B4 and B5, 2026-11-18, as each item
  says: none was seen in the hand checks of B4 and B5, so each becomes a task only if it is
  noticed in use; the struck ones are in ADR 0018 §2.7):
  - *The worker:* if the metadata worker stops after it took a request, its answer never comes
    and the delete stays busy, which also holds moves back (the moves have the same gap). Kept:
    not seen; the same gap as the moves, and it belongs with the ExifTool timeout above.
  - *What is shown:* a held Delete key repeats "A delete is still running"; nothing is shown while
    the prepare runs; between the first answer and the end of the batch the deleted images are
    still in the list, and a rating key on one fails (as for the moves). Kept: not seen in the
    hand check of B4; a task if it is noticed in use.
  - *Paths:* a JPG partner left out of the plan is not waited for, so a rating write still queued
    for it fails after the delete (noise, not loss); a folder read already sent may put a deleted
    path back into the metadata cache (the tab's list is the source); a `_rejected` folder reached
    under another spelling is not matched by `same_path`, so a later Ctrl+Z on it fails safely.
    Kept: rare and harmless; a task if it is noticed in use.
  - *Other windows:* while a delete runs, the metadata window can be opened on an image being
    deleted, and its save then fails. While the confirmation waits, `same_folder` (which resolves
    the folder on disk) is asked several times a frame; the drive was accepted by T185's check, so
    it is a fixed drive. Kept: safe and not seen; a task if it is noticed.
  - *Not hand-checked:* a network folder refused with "the drive has no Recycle Bin" (the owner
    had no network drive at the hand check of B4, 2026-11-18). T185's decision tests cover
    `DRIVE_REMOTE` and a volume without a name. Kept until a network drive is at hand (not
    checked at B5 either, 2026-11-18).
  - *A drive without Recycle Bin settings* (T223, 2026-11-20): a fixed drive whose
    `Explorer\BitBucket\Volume\{GUID}` key does not exist is refused as "settings unreadable"
    (ADR 0018 B-Q14), so every delete there fails. GitHub's runner had no key for `D:`; when
    Windows creates the key is not known, so a user's second drive may lack it too. Whether a
    missing key should count as Windows' defaults (bin on, default size) is the owner's decision
    for ADR 0018. Kept: not seen on the owner's machine (the keys exist for every local fixed
    drive, 2026-11-17); a task if it is noticed in use.

- **Known limits of opening images in another program** (T201's brief, 2026-11-19; accepted when
  T201 was done; **kept** at the sort after ADR 0019 E1–E3, 2026-11-19 (owner): none was seen in
  the hand check of E1–E3, so each becomes a task only if it is noticed in use; the struck ones are
  listed at the end of this file):
  - *The length:* `command_line_length` follows `std`'s quoting on Windows as it is today; a later
    `std` that quotes otherwise is caught only by the start's error (the start then fails with
    Windows' reason).
  - *`.bat` and `.cmd`:* a batch file set up as a program is measured as an `.exe`; `std` starts
    it through `cmd.exe` and may refuse some arguments, so its start may fail with `Failed`.
  - *A request that waits* (T202's brief, accepted when T202 was done): quitting while a request
    waits drops it (nothing is started or said); the wait has no time limit, so a worker that
    never answers a write, or a user who keeps rating the targets, holds the start back (the
    status line keeps "Waiting for writes …"); an image moved or deleted while its request waits
    is left out by its old path, not followed to its new place; a program renamed or removed in
    Settings while a request waits is still started by its old name and path; a request waits
    only for writes already queued, so a rating key pressed on one of its images after the start
    is not seen by the program until it reads the file again (ADR 0019 §2.7).
  - *What is shown* (T202's independent review, 2026-11-19): while a request waits only for a
    move or a delete (its writes done), the status line says nothing; when a new request
    replaces one that waited, the old "Waiting for writes …" stays until the start; while a
    request waits behind a delete confirmation left open, a frame is drawn every 50 ms; a
    repeated Ctrl+E on a missing program adds one Tasks line per press.
  - *Settings and the system:* after Save, a program whose path is not UTF-8 is used until a
    restart although T200 does not write it to the file; the Settings window has no scrolling,
    so on a very small screen its section "External Programs" may be cut off; the menu checks each program's path (`is_file`)
    every frame while the Image menu is open, so a program on a network share that is gone may
    slow the open menu (T202's independent review).
  - *Show in File Explorer* (T203's brief and its independent review, accepted when T203 was
    done): `resolve_paths` joins the working folder without normalising, so a tab opened on a
    `\\?\` path or a path with `..` makes File Explorer open another folder (it gives no
    error), and a path over 260 characters may do the same; it does not wait
    for a move, so right after X with "Move Rejected and Picked Images to Subfolders" on it may
    open the old folder (or say the image is no longer in its folder once the move is done).

- **Leftovers from the review of T197** (reviewer and an independent review agent, 2026-11-19;
  none is caused by T197, which avoids the first for X-Trans by refusing a box under 512 px before
  LibRaw is called):
  - *A failed LibRaw call kills the session:* LibRaw's exception handler calls `recycle()` on an
    error such as `IO_CORRUPT`, so the unpacked RAW data is gone and every later call on that
    `pw_libraw::Session` fails ("Out of order call of libraw function"), Bayer files too;
    `statistics` then gives `Ok(None)` without an error. A fix would reopen the session or report
    it as dead.
  - *Panic paths in `pw-libraw/src/lib.rs` from before T197* (AGENTS.md §3): `r.x + r.w` and
    `r.x + r.w + margin` in `render_region_with` can overflow `u32` (a panic in a debug build; in
    release the shim's bounds check and the box check refuse the result), and `Rgb8::crop` is a
    public function with an `assert!`.
  - *The cost of an X-Trans region:* the shim's box reaches up to 1487 px plus the region plus
    64 px on each axis, about 0.26–0.45 s for a 1024 px tile on `_DSF4243.RAF` against 1.55 s for
    the whole image (T197's measurements). It matters only if the loupe ever renders many small
    tiles of a RAF.

Removed on 2026-11-13 because they are done or planned elsewhere: ADR 0010 steps M3–M7 (done,
T30–T40); ADR 0011 step K4 (done, T26–T29); moving rejected or picked images to `_rejected` /
`_picked` (ADR 0013 §13), now ADR 0017 U9 (§2.6, U-Q6); the import dialog fields for title and description in the Tauri
app's `ui/` (owner: dropped, the Tauri app is retired at the switch, GR8). Removed on 2026-11-14:
"Same folder, two tabs" (done, T119); suppressing the Windows "no disk in drive" dialog (done, T135: `SetErrorMode` at start). Removed on 2026-11-17: "Mark a card imported before the
register" (now T166) and "The compare after a move that did not happen" (now T165); "The clean-room policy and the documents
disagree" (the owner's rule in `clean-room.md`, "Naming other products"; the rewording is T175).
Struck at the backlog sort of 2026-11-17 (owner): the measurement of the 100 % view first (T144,
U7c-Q5; the step is done and the gain small) and the large-folder limit of `relocate::companions`
(not seen in the hand check of U9b).
Struck at the backlog sort after U12d, 2026-11-17 (owner): from T179's leftovers, the same entry
in `add` and `remove` (T180 never sends both) and `ConflictKept` named when the `Entries` would
have changed nothing (accepted, ADR 0017 U12d-Q8).
Written as tasks after the backlog sort of 2026-11-17: a wrong Artwork Date Created (T181), RAW
statistics on X-Trans (T182; T146 had tested the X-Trans files, T182 covers what it left), a tab on
`_rejected` or `_picked` and Ctrl+Z while a move runs (T183), `release_for_move` a required
method (T184).
Struck at the backlog sort after B4, 2026-11-18 (owner; written into ADR 0018 §2.7 as accepted
known limits): from the delete's known limits, quitting while a delete runs (the images not yet
sent stay, nothing is lost) and the view sessions released by a delete refused at the worker
(the image opens again, seen in the hand check of B4).
Struck at the backlog sort after B5, 2026-11-18 (owner; written into ADR 0018 §2.7 as accepted
known limits): T190's leftovers, a partner un-rejected after its image's delete was sent, the
group gathered again at delete time, and a tab closed while the batch runs.
Moved to an ADR on 2026-11-18: "The folder follows changes on disk" (ADR 0026, accepted by the
owner the same day, with the two ADR 0017 §7.2 wishes "Load a folder again" and "Images shown
while importing"; steps F1–F4 in `plans/adr-0018-0025-steps.md`).
Written as a task on 2026-11-18: "A region render fails on every RAF" (T182's finding) is T197,
ADR 0027 step W0, moved forward by the owner to order 1b in `plans/adr-0018-0025-steps.md`.
Struck at the backlog sort after ADR 0019 E1–E3, 2026-11-19 (owner): from the known limits of
opening images in another program, Windows' reason in `start_failed_text` given in Windows'
language (the system's own text, accepted), and "Show in File Explorer" reached only from the
contact sheet since the preview draws no menu bar (covered by "A right-click menu on images").
Moved to an ADR on 2026-11-19: "A right-click menu on images" (ADR 0019 addition 1, §2.9, accepted
by the owner the same day; step E4 in `plans/adr-0018-0025-steps.md`, before ADR 0024 O1).
- **RawStride's own writes and a check by modification time** (chain review of T204, 2026-11-19):
  since T204 a file RawStride writes gets the time of the write (ADR 0007 §9.4). ADR 0026's folder
  watcher and ADR 0030's change check must tell RawStride's own writes from outside changes (as
  the histogram cache does with its stamp before and after a write, U7c-Q7); taken up in those
  ADRs' steps. **Stays (owner, 2026-11-19, after P0b's hand check):** it belongs to those steps,
  none of which has started.
- **Known limits of T205** (independent review, 2026-11-19; accepted when T205 was done):
  a keyword that itself holds `|` is copied into a flat `lr:hierarchicalSubject`, which then
  reads as nested and is kept unchanged from then on (none of 68,647 keywords in ADR 0030's K0
  sample holds `|`); flat entries of `lr:hierarchicalSubject` that are not among the keywords
  are replaced when the keywords are written (ADR 0007 §8, Q15, as decided);
  `XMP-microsoft:Rating` and `XMP-microsoft:RatingPercent` (Windows Explorer's) are not in
  ADR 0007 §6.3 and stay as they are when RawStride writes a rating. **Stays (owner, 2026-11-19,
  after P0b's hand check):** no keyword with `|` in the K0 sample, the replacement is the Q15 rule,
  and the Microsoft fields are taken up if Windows Explorer is seen showing a stale rating.
- **Leftover of T211** (2026-11-19): `pw_metadata::Error::ReadOnly` and `Error::NotWritten` are
  no longer built by production code (only the removed `refused_write_error` built them);
  `From<Error> for WriteError` still maps them. They are public, so no warning; remove them when
  `Error` is next changed, or keep them.
- **Leftovers of T221** (the right-click menu on images, ADR 0019 E4; review, 2026-11-19; accepted
  when T221 was done; **kept** at the sort after ADR 0019 E4, 2026-11-19 (owner): none was seen in
  the hand check of E4, so each becomes a task only if it is noticed in use):
  - *Keys while the menu is open:* the keys keep acting while the right-click menu is open, as
    with the menu bar's menus: Escape closes it and also clears the marking, the arrows move the
    current image under it.
  - *Opening it from the keyboard* (Shift+F10 or the menu key): not built.
  - *The first frame:* in the frame of the right-click the menu is greyed by the image that was
    current before (the click is applied after the contact sheet is drawn); nothing can be chosen
    in that frame.
  - (The filmstrip has no menu by the owner's choice, ADR 0019 §2.9; the template submenu waits
    for ADR 0017 U13.)
