# T203. ADR 0019 E3: "Show in Explorer", and the owner's hand check of E2 and E3 (pw-loupe) — Blocked: waits for T202

- **Chain:** T200 (E1, done) → T201, T202 (E2) → T203 (E3). The owner's answers are E-Q1–E-Q14 in
  ADR 0019 §3.
- **Why:** ADR 0019 §2.8, step E3: Image ▸ "Show in Explorer", without a key (E-Q11), opens the
  folder of the current image in Windows' file manager with the file selected. ADR 0019 §4 puts
  the owner's hand check after E2 and E3; it is written here and covers both.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/external.rs` (point 2),
  `crates/pw-loupe/src/external_text.rs` (point 3), `crates/pw-loupe/src/lib.rs` (only the
  exports of points 2 and 3), `crates/pw-loupe/src/shell.rs` (one command),
  `crates/pw-loupe/src/shell_text.rs` (its text), `crates/pw-loupe/src/main.rs`,
  `docs/glossary.md`, `docs/test-inventory.md`, and this task's own file (status and log).
  **Starts after T202 is committed.**
- **What:**
  1. **Command** (`shell.rs`): `Command::ShowInExplorer`, no key, in the Image menu right after
     `MenuEntry::ExternalPrograms` (T202) and before the separator that comes before "Delete to
     Recycle Bin". Its text in `command_text` (`shell_text.rs`) is "Show in Explorer" (ADR 0019
     §2.8); `acts_in` gives `Where::Both` through its default arm. `command_text` and
     `run_command` match every command and need the new arm; `command_enabled` has a `_` arm, so
     the greying of point 4 is an explicit arm. Three existing tests list the menus and are
     extended with the new item (allowed by this task, an exception to rule 5: an item is added,
     nothing else of what they check changes): `the_delete_items_are_the_last_items_of_the_image_menu`
     (`shell.rs`, about line 1038, as T202 left it: its `checked_sub(6)` becomes
     `checked_sub(7)`, its expected slice and its message and doc comment get the new item),
     `every_menu_item_text_in_menu_order`
     ("Show in Explorer" in its place, `shell_text.rs` about line 453) and
     `every_menu_key_text_in_menu_order` (a `None` in its place, about line 515).
  2. **The start** (`external.rs`):
     - `pub fn explorer_select_argument(path: &Path) -> OsString`: `/select,"<path>"`, built by
       pushing `/select,"`, `path.as_os_str()` and `"` onto an `OsString` (lossless; a Windows
       path holds no `"`);
     - `pub fn show_in_explorer(path: &Path) -> Result<(), String>`: one
       `std::process::Command` of `explorer.exe` in the folder named by the environment variable
       `SystemRoot` (`Path::new(&root).join("explorer.exe")`), or plain `explorer.exe` when it is
       not set, with that argument passed as it is, not quoted
       again (`std::os::windows::process::CommandExt::raw_arg`, under `#[cfg(windows)]`; on other
       platforms it returns `Err` with a short reason and starts nothing, since RawStride runs on
       Windows); the child is dropped at once, never waited for (Explorer's exit code is not a
       success signal); a spawn error is `Err(error.to_string())`.
  3. **Text** (`external_text.rs`, rule 13): `explorer_failed_text(reason: &str)`: "Explorer
     could not be started: {reason}" (no full stop after `{reason}`, as T201's texts), and
     `explorer_missing_text(name: &str)`: "{name} is no longer in its folder." (`{name}` the
     file name).
  4. **Enabled and run** (`main.rs`, `command_enabled` and `run_command`): greyed while the
     active tab shows no current image (`tab.position().is_some()`). It acts on the **current
     image only**, also in the contact sheet with several images marked (ADR 0019 §2.8: "the
     folder of the current image"): the tab's `files.get(tab.current)`, resolved with
     `resolve_paths` (an error goes to the status line, `self.message`). A file that is no longer
     a file (`is_file`, moved or deleted since the tab was read) gives
     `explorer_missing_text(name)` in the status line and starts nothing, since Explorer would
     silently open another folder. `show_in_explorer`'s error goes to the status line and the
     Tasks error list (`self.tasks.add_error(…, ClockTime::now())`) with `explorer_failed_text`.
     Nothing is shown when it starts.
  5. **Glossary** (`docs/glossary.md` §3): "Show in Explorer" (the Image menu's item, no key; ADR
     0019 E3, E-Q11; added by T203). The glossary's row "Utforskaren | File Explorer" (§5, T118)
     names Windows' program; the menu item keeps the ADR's shorter "Explorer" (see the log).
- **Tests to add:** `external.rs`: `explorer_select_argument` for a path without and with a
  space and with a non-ASCII letter (no test starts Explorer). `external_text.rs`:
  `explorer_failed_text` and `explorer_missing_text`. `shell.rs`, `shell_text.rs`: the Image menu's order with the new item,
  its text, and that it has no key.
- **Known limits** (to `backlog.md` when this task is Done): `resolve_paths` joins the working
  folder without normalising, so a tab opened on a `\\?\` path or a path with `..` makes
  Explorer open another folder (Explorer gives no error); a path over 260 characters may do the
  same. The preview draws no menu bar and the item has no key (E-Q11), so it is reached only
  from the contact sheet.
- **Not in this task:** a key for it (E-Q11: none); several images at once.
- **Rules:** `AGENTS.md` §3; `docs/tasks/README.md` rule 13. The task starts a process from
  `main.rs` and touches the menu, so it gets an independent code review before the owner's
  approval, as T202.
- **Hand check by the owner** (ADR 0019 §4, after E2 and E3), with the owner's RAW editor set up
  as the first program in Settings ▸ External Programs and a second program added:
  - Settings: Add… picks an `.exe` and its name appears without `.exe`; the name can be changed;
    Move Up and Remove work; Cancel keeps the old list; Save keeps the new one after a restart;
  - the Image menu shows "Open in {name}" for each program in the list's order, the first with
    Ctrl+E; with an empty list one item "Open in Another Program…", and it and Ctrl+E open the
    Settings window, also from the preview;
  - Ctrl+E on one image in the contact sheet and in the preview opens it in the RAW editor, and
    the status line says "1 image opened in {name}.";
  - a program that cannot start (for example a renamed `.txt` set up as a program): the reason
    in the status line and the Tasks list;
  - several marked images in the contact sheet go in one start; in the preview only the current
    one goes;
  - an image rated just before Ctrl+E: the rating is in its sidecar when the RAW editor opens it
    ("Waiting for writes …" may flash in the status line); the same after "Apply to All Marked"
    on several marked images followed at once by Ctrl+E;
  - an image rejected with X while "Move Rejected and Picked Images to Subfolders" is on, then
    Ctrl+E at once: it is not opened from its old place, and the Tasks list names it;
  - a program whose `.exe` was renamed or removed: its menu item is greyed, and Ctrl+E (when it
    is the first) says it does not exist, in the status line and the Tasks list;
  - more than 100 marked images: the question "Open {n} images in {name}?", Enter and Escape
    cancel, Tab to Open and Enter opens;
  - "Show in Explorer" opens the image's folder with the file selected, also for a path with
    spaces and with å/ä/ö; with several images marked it shows the current one; it is greyed
    in an empty tab;
  - not checked by hand: the refusal of a command line longer than Windows allows (tests cover
    it), a second Ctrl+E replacing a request that waits (too short to see), and quitting while
    a request waits (the program is then not started, nothing is said; a known limit of T202).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows no change from this
  task outside the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (ADR 0019 E3, order 1c), with T201 and T202 as one chain; waits for T202. Cold read (fresh agent, 2026-11-19): the three menu tests named and allowed to grow; the argument built as an `OsString`; Explorer from `SystemRoot`; a file no longer there gives `explorer_missing_text` instead of Explorer opening another folder; `\\?\` and `..` paths as known limits; the hand check widened. Chain review: see T201's log. Open question to the owner: the menu text "Show in Explorer" (the ADR) or "Show in File Explorer" (the glossary's name of Windows' program, §5) |
