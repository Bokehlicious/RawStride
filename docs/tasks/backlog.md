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
  Leftovers from the review of T193 (2026-11-18): the text of `DigestNotWritten` ("because a
  conflict is unresolved") is misleading when the reason is a held list with qualifiers;
  `write_plan::plan` itself does not refuse a changed list with qualifiers (only its callers in
  `edit.rs` put it back; `write_rating_safely` writes no list); recent values (`main.rs`
  `recorded`) also record a keyword that was put back; the batch's "Written" count also counts an
  image whose write was `Unchanged`; an `xml:lang` on a list item is not in ExifTool 13.59's JSON
  (so not marked) and is kept by a whole-list write, but also given to the new items; T193's hand
  check (owner, 2026-11-19) did not try a JPEG with IIM, a list with qualifiers and an IIM/XMP
  conflict (no such file at hand), so the held list's `DigestNotWritten` is checked only by tests;
  Leftovers from the review of T194 (2026-11-19): the batch (`metadata.rs` `batch_written`) lists
  only `ConflictKept` and `QualifiersKept` of an image other than the window's; its `Truncated`,
  `IimConvertedToUtf8` and `DigestNotWritten` are dropped, so the user does not learn that a field
  was truncated in another image, and the `match` ends in `_ => {}`, so a new notice kind would be
  dropped without a decision (an exhaustive `match` would force one);
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
- **CI before the repository becomes public** (owner, 2026-11-14): a GitHub Actions workflow
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
  public.
- **Linux and macOS ports** (owner, 2026-11-17: only if there is interest once the repository is
  public; the owner uses only Windows). Survey of the code, 2026-11-17, nothing built off Windows:
  - *A small README note when public*: Windows only today, an issue where people can show interest.
    Keep putting Windows code behind `#[cfg(windows)]` with a `not(windows)` branch, as now.
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
  - *Distribution*: `docs/distribution.md` and `package-dist.ps1` are Windows only. macOS needs an
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
- **`chrono` is unmaintained** (`cargo audit` in `tools/check.ps1`, seen 2026-11-17):
  RUSTSEC-2026-054356 marks `chrono` 0.4.45 as unmaintained. It is a warning, not a vulnerability,
  so the check still passes. Find which crates use it (`cargo tree --offline -i chrono`) and
  whether a maintained replacement fits; a change of dependency needs a task of its own.
- **A filter bar of toggle buttons instead of drop-down lists** (owner idea, 2026-11-10, after T61;
  ADR 0014 §15): a row "0★ 1★ 2★ 3★ 4★ 5★" and a row of colour swatches, each clicked on and off,
  with a check box per row that turns the row on or off. Needs an amendment of ADR 0014 first: the
  star part would become a set of star values instead of the ≥ / = / ≤ condition (T60).
- **Before the repository is made public** (survey, 2026-11-17; the README section "How RawStride
  is developed" and general antivirus wording in the agent rules were done then). The licence,
  CONTRIBUTING and the dependency survey are in place, and no secrets were found in the files or
  the history. Left, for the owner to decide:
  - *The owner's e-mail address* is in all commits. Keep it, or rewrite the history of both
    branches to the GitHub noreply address (`git filter-repo --mailmap`). A rewrite changes every
    commit hash (check the documents' references to hashes first); push it to a new, empty
    `RawStride` repository after renaming the old one (kept private and archived), and turn on
    "Block command line pushes that expose my email".
  - *A trademark search* for the name RawStride (ADR 0012, ADR 0002 §4).
  - *Small fixes*: `repository = ""` in `Cargo.toml`; a path with the owner's user name in
    `done/T115-…`; `docs/agent-tasks.md` (about 700 KB, replaced by `docs/tasks/`) archived or
    removed; the done tasks' logs read once with public eyes.
  - *Choices*: the owner's name as test data in `pw-metadata`; the one-off `tools/m2-*.cmd`
    scripts with paths to the test-data folder; `.github/` with CI, issue templates and a
    `SECURITY.md` (optional); CONTRIBUTING asks contributors for `Signed-off-by`, which the
    owner's own commits do not carry.
  - *Check on GitHub*: the repository's visibility and settings today (not checked, `gh` is not
    installed).
  - A binary release also needs what ADR 0002 §4 lists (distribution folder,
    `THIRD-PARTY-NOTICES.txt`, ExifTool).
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
