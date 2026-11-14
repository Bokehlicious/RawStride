# T138. ADR 0017 U14: sort order in the command bar and the contact sheet — Open

- **Can start now.** The owner approved the proposals of U14-Q1–U14-Q7 (at the end of this task)
  on 2026-11-14; they are written into the points below and into ADR 0017 §6.1.
- **May run beside T139** (the PLUS structures in the metadata window). Both change
  `crates/pw-loupe/src/main.rs`, in different functions: this task the command bar, the menu
  items of Visa, the start and the per-frame follow-up of the sort (point 5); T139 only the
  metadata window. Neither touches the other's functions. Both may add terms to
  `docs/glossary.md`: add rows, never rewrite another row. **Run `.\tools\check.ps1` only when the
  other task's agent is not running it** (both build in the same `target` folder).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/sort.rs` (new: the sort keys and the
  ordering as plain Rust, with its tests); `crates/pw-loupe/src/tabs.rs` and
  `crates/pw-loupe/src/browse.rs` (reordering a tab's files, keeping the current image and the
  marking); `crates/pw-loupe/src/lib.rs` (the module line, `list_folder` only if needed);
  `crates/pw-loupe/src/shell.rs` and `crates/pw-loupe/src/shell_text.rs` (the menu items and their
  texts, rule 13); `crates/pw-loupe/src/settings.rs` (the remembered sort order);
  `crates/pw-loupe/src/main.rs` (only the parts named above); `docs/glossary.md`;
  `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.1 and step U14: today a folder is always in file-name order (`list_folder`,
  ignoring case). When culling a shoot from two cameras, or a burst renamed by the camera, the
  owner needs the capture order, and when picking, the best-rated first.
- **What:**
  1. **The sort keys** (U14-Q1): **File Name** (today's order, the default), **Capture Time**
     (Exif DateTimeOriginal), **Modified** (the file's modification time), **File Type** (the
     extension, then the name) and **Rating** (stars, then label in the order of the label keys;
     no rating last), each **ascending or descending** (U14-Q2). Ties are always broken by the file
     name, ignoring case, so a RAW and its JPEG stay next to each other when their key is equal.
  2. **One order for all tabs** (U14-Q3), remembered in `settings.txt`
     (`sort = name|capture|modified|type|rating`, `sort_descending = true|false`); every opened
     folder uses it; changing it reorders every open tab.
  3. **Keys that must be read first** (U14-Q4): Capture Time is read with `pw_raw::inspect` (as the
     inspector's Info, U2-Q1) on a background thread for every file of the tab, the visible
     images first; Rating uses the ratings the tab already reads for the filter (ADR 0014 Q7). Until
     every key of a tab is read, the tab keeps its order and the status bar shows "Sorting by
     capture time: {i} of {n}"; then the tab is reordered once. A file whose date cannot be read
     or has none sorts after those with a date (ascending and descending). The capture times are
     kept per tab while it is open.
  4. **No reordering while working** (U14-Q5): a rating key, a write or a refreshed folder does not
     move an image in the current order. The order is applied when a sort is chosen, when a folder
     is opened (or opened again), and when a key read of point 3 completes. So a rated image stays
     where it is while culling.
  5. **Reordering keeps the place** (U14-Q6): the current image stays current and visible (the
     contact sheet scrolls to it, as a key step does), the marking is kept, the filter is applied
     again to the new order. The view core gets the new file list as a filter does (`set_files`,
     ADR 0014), so decoded images are kept.
  6. **Where** (U14-Q7): in the command bar a drop-down "Sort: {key}" with the five keys and a
     button "↑"/"↓" for the direction (its tooltip "Ascending"/"Descending"); in Visa a submenu
     "Sort By" with the five keys (the chosen one checked) and "Descending" as a check item. No
     key for it now (U8 may add one).
- **Tests to add** (`sort.rs`, `tabs.rs`/`browse.rs`, `settings.rs`): each key ascending and
  descending on a built list, ties by name, missing capture time last both ways, the rating order
  with labels and no rating; reordering keeps the current image and the marking and reapplies the
  filter; a rating change does not reorder; a tab waiting for its keys keeps its order and reports
  "{i} of {n}"; the setting saved and loaded, an unknown value warned and ignored; the texts
  through their functions.
- **Not in this task:** a custom (dragged) order, sorting by other metadata (title, city …), a
  sort per tab, sorting the folder tree, a key for the sort.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (with T139's files if it runs at the same time).

## Questions (the reviewer's proposals; written into the points above)

| # | Question | Proposal |
|---|---|---|
| U14-Q1 | Which sort keys? | File Name (default), Capture Time, Modified, File Type, Rating (stars, then label); ties by file name |
| U14-Q2 | Direction | Ascending or descending for every key, one button beside the drop-down |
| U14-Q3 | Per tab or for all tabs? | One order for all tabs, remembered in `settings.txt` |
| U14-Q4 | Capture times are not read today; how are they read? | `pw_raw::inspect` on a background thread, visible images first; the tab keeps its order and shows "{i} of {n}" until all are read, then is reordered once; no date sorts last |
| U14-Q5 | Does a rating key move the image when sorted by rating? | No: the order is applied only when a sort is chosen, a folder is opened, or a key read completes, so nothing jumps while culling |
| U14-Q6 | What happens to the current image and the marking? | Both kept; the contact sheet scrolls to the current image; the filter is applied again |
| U14-Q7 | Where is it chosen, and names? | Command bar: "Sort: {key}" drop-down and an ↑/↓ button; Visa ▸ "Sort By" with the keys and "Descending"; English names (GR14) in the glossary; no key yet |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Blocked | Added; waits for U14-Q1–U14-Q7 |
| 2026-11-14 | Open | The owner approved U14-Q1–U14-Q7 as proposed |
