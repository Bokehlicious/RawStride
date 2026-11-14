# T135. ADR 0017 U4: the Library: folder tree, favourites and recent folders — Open

- **Can start now.** The owner approved the proposals of U4-Q1–U4-Q7 (at the end of this task) on
  2026-11-14, including the two `windows-sys` features of U4-Q1; they are written into the points
  below and into ADR 0017 §6.1.
- **May run beside T136** (the metadata window). Both change `crates/pw-loupe/src/main.rs`, in
  different functions: this task only the left column in `draw_columns`, the field holding the
  library's state in `struct Loupe`, `open` (point 5) and the drive-check call at start (point 1);
  T136 only the metadata window. Neither touches the other's functions. Both may add terms to
  `docs/glossary.md`: add rows, never rewrite another row. **Run `.\tools\check.ps1` only when the
  other task's agent is not running it** (both build in the same `target` folder).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/library.rs` (new: the tree, favourites
  and recent folders as plain Rust, with its tests); `crates/pw-loupe/src/library_text.rs` (new:
  its texts, rule 13); `crates/pw-loupe/src/lib.rs` (the two `pub mod` lines);
  `crates/pw-loupe/src/main.rs` (only the parts named above); `crates/pw-loupe/src/settings.rs`
  (favourites, recent folders, the sections' states); `crates/pw-loupe/src/shell_text.rs` (only
  `LibraryNote`, which goes away); `crates/pw-loupe/Cargo.toml` (two `windows-sys` features, U4-Q1,
  only if approved); `docs/glossary.md`; `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.1: the library on the left with a folder tree of the computer's drives,
  favourites and recently opened folders; clicking a folder opens it in a tab as a contact sheet
  (GR13). Today the left column holds only the line "Open a folder with Ctrl+O or drag it to the
  window." (U1-Q3).
- **What:**
  1. **Drives** (U4-Q1): the drive letters come from `GetLogicalDrives`, each with its kind from
     `GetDriveTypeW` and its volume label from `GetVolumeInformationW` (`windows-sys`, feature
     `Win32_Storage_FileSystem`). At start the program calls
     `SetErrorMode(SEM_FAILCRITICALERRORS)` (feature `Win32_System_Diagnostics_Debug`), so an empty
     card reader or DVD drive gives no Windows "There is no disk in the drive" dialog; such a drive
     is shown as "{letter}: (no disk)" and cannot be expanded. Drives are read at start and again
     when the user presses "Refresh" (or F5 while the library has the pointer); nothing is polled.
     The Windows calls sit in one small `cfg(windows)` function; the rest of `library.rs` is plain
     Rust that takes the drive list as data, so it is tested without real drives.
  2. **The tree** (U4-Q2): drives first, as "{label} ({letter}:)" or "{letter}:" without a label;
     a folder's subfolders are read only when it is expanded (one `read_dir`, on the UI thread for
     now; a slow network folder is a known limitation, named in the report), sorted by name
     without regard to case, hidden and system folders left out, no files. An expand that fails
     (no access, gone) shows one greyed line with the reason under that folder. A click on a folder
     opens it (point 5); the triangle only expands or collapses. The active tab's folder is
     highlighted when it is visible in the tree; the tree does not expand by itself (U4-Q5).
  3. **Favourites** (U4-Q3): "Add to Favourites" in the right-click menu of a folder in the tree,
     of a folder tab and in the Arkiv menu (the active tab's folder); "Remove from Favourites" in
     the right-click menu of a favourite. Shown in the order they were added, by folder name with
     the full path as tooltip; a favourite that no longer exists is greyed with the tooltip
     "Folder not found" and can only be removed. A folder is a favourite once (compared as tabs
     compare folders, `tabs.rs`, GR12).
  4. **Recent folders** (U4-Q4): the last 10 folders opened in any way (tree, favourites, Ctrl+O,
     drop, import, command line), most recent first, a folder once; "Clear Recent Folders" in the
     section's right-click menu. Shown and greyed like favourites.
  5. **Opening** (U4-Q6): a click on a folder in the tree, a favourite or a recent folder opens it as
     Ctrl+O does (`open(folder, true)`: a new tab, or the tab that already shows it, GR12; as a
     contact sheet, GR13). `open` records every successfully opened folder in the recent list.
     Not while a dialog holds the program (as `open_folder_dialog`).
  6. **The column** (U4-Q7): under the heading "Library" three collapsible sections **Folders**,
     **Favourites** and **Recent**, each in its own scroll area when long, their states remembered;
     "Tasks" stays below as today. The line `LibraryNote` goes away; an empty Favourites section
     says "Right-click a folder to add it.", an empty Recent section "No folders opened yet.".
  7. **Settings** (`settings.txt`, U4-Q3, U4-Q4): one line per favourite `favourite = <path>` and
     per recent folder `recent = <path>`, in their order; the sections' states as the other
     sections' are. A path that cannot be read back is skipped with the settings file's usual
     warning. `Settings` is `Copy` today; keep the lists outside it (for example a separate
     `Library` value loaded and saved with it) rather than changing every caller.
- **Tests to add** (in `library.rs` and `settings.rs`): the tree from a given drive list (labels,
  no disk, order); subfolders sorted without case, hidden and system folders left out (built in a
  temporary folder; hidden set with the Windows attribute in a `cfg(windows)` test); an expand that
  fails gives its line; favourites added once, removed, kept in order, a missing one greyed; recent
  folders capped at 10, most recent first, a reopened folder moved to the top; favourites and
  recent folders saved and loaded, an unreadable line skipped; the texts through their functions.
- **Not in this task:** the tasks area (U5), watching folders for changes, a tree that follows the
  active tab by expanding, drag-and-drop in the library, network locations other than mapped
  drives, renaming or creating folders.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency (`windows-sys` is already
  `pw-loupe`'s; only the two features of point 1, U4-Q1).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (with T136's files if it runs at the same time).

## Questions (the reviewer's proposals; written into the points above)

| # | Question | Proposal |
|---|---|---|
| U4-Q1 | How are the drives found without the Windows "no disk" dialog for empty card readers (the backlog item "Suppressing the Windows 'no disk in drive' dialog")? | Two more features of the existing `windows-sys` =0.61.2 in `pw-loupe`: `Win32_Storage_FileSystem` (drive list, kind, label) and `Win32_System_Diagnostics_Debug` (`SetErrorMode` at start). No new package; `Cargo.lock` unchanged. This also solves that backlog item for the whole program |
| U4-Q2 | What does the tree show? | Drives, then folders only; hidden and system folders left out; sorted by name; read when expanded, never in the background |
| U4-Q3 | How are favourites added, removed and stored? | Right-click "Add to Favourites" on a tree folder or a tab, and in Arkiv; right-click "Remove from Favourites"; order as added; `favourite = <path>` lines in `settings.txt` |
| U4-Q4 | How many recent folders, and from where? | 10, from every way of opening a folder, most recent first; "Clear Recent Folders" |
| U4-Q5 | Does the tree follow the active tab? | It highlights the active tab's folder when it is visible, but does not expand by itself (less movement in the column) |
| U4-Q6 | Does a click open a new tab or replace the active tab? | A new tab, or the tab that already shows the folder, as Ctrl+O and a drop do today (GR12) |
| U4-Q7 | Names and layout of the column | "Library" with the sections Folders, Favourites and Recent (English, GR14; new glossary terms), each collapsible and remembered; Tasks below as today |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Blocked | Added; waits for U4-Q1–U4-Q7 |
| 2026-11-14 | Open | The owner approved U4-Q1–U4-Q7 as proposed |
