# T200. The list of external programs and its Settings section (ADR 0019 E1) — Done

- **Why:** ADR 0019 (accepted by the owner on 2026-11-17, moved forward as order 1c on 2026-11-19
  in `plans/adr-0018-0025-steps.md`) lets the user open the chosen images in another program with
  one key. Its first step, E1, is the list of those programs: up to five, each a name and the path
  to its `.exe`, kept in `settings.txt` and set up in a section "External Programs" of the Settings
  window (ADR 0019 §2.1, E-Q1, E-Q2). The menu items, Ctrl+E and the start itself are step E2, a
  later task; nothing in this task starts a program.
- **Crates:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/settings.rs` (the type, the field of `Settings`, reading and writing the
    lines, the tidying of names, and their tests in its `tests` module)
  - `crates/pw-loupe/src/shell_text.rs` (the new `SettingsText` variants, one text function with a
    number, and their test)
  - `crates/pw-loupe/src/lib.rs` (only the `pub use settings::{…}` and `pub use shell_text::{…}`
    lists: `settings` and `shell_text` are private modules, so `main.rs` sees only what is
    re-exported there)
  - `crates/pw-loupe/src/main.rs` (only its `use pw_loupe::{…}` list, `draw_settings_window` and
    one new helper beside `pick_folder` that opens the file dialog for a program;
    `save_settings_window` only if point 6 shows it is needed)
  - `docs/glossary.md` (the new terms, §6)
  - `docs/test-inventory.md` (counts and the description of the `pw-loupe` tests)
- **What:**
  1. **The type.** In `settings.rs`:
     - `pub struct ExternalProgram { pub name: String, pub path: PathBuf }` with
       `Clone, Debug, PartialEq, Eq`, and `pub const MAX_EXTERNAL_PROGRAMS: usize = 5;`
     - a new field of `Settings` (not of `ShellSettings`):
       `pub external_programs: Vec<ExternalProgram>`, in the list's order, the first the default
       program (E-Q1); empty by default. It must be in `Settings` itself because
       `save_settings_window` (`main.rs`) takes only the theme from the window's `ShellSettings`
       and keeps everything else of `ShellSettings` as it was.
     - The test `every_shell_key_round_trips_through_save_and_load` (`settings.rs`) builds a
       `Settings` literal with all three present fields and no `..`; it will not compile with the
       new field. Add `..Settings::default()` to that literal and change nothing else in the test
       (allowed by this task, an exception to rule 5; the test checks exactly what it did before).
  2. **Reading.** In `load_settings_and_library`, a new key `external_program` (a `const` beside
     the others). Its value is `<name> | <path>`:
     - the value holds exactly one `|`; the name is the text before it and the path the text after
       it, each trimmed;
     - the name is not empty; the path is a full path without a line break (the same test as
       `parse_folder`; the file need not exist: a program whose `.exe` is gone is greyed in the
       menu by E2, E-Q9);
     - a good line is appended to `external_programs`, so the file's order is the list's order;
     - a line that is not good (no `|`, two or more `|`, an empty name, an empty or relative path)
       is ignored and gives one problem worded as the `favourite`/`recent` problem in that
       function: `settings file {path}, line {number}: bad value "{value}" for external_program
       (expected a name, '|' and a full path to a program), line ignored`;
     - a good line when five programs are already read is ignored and gives one problem:
       `settings file {path}, line {number}: more than 5 external programs, line ignored` (the 5
       from `MAX_EXTERNAL_PROGRAMS`). A bad line is reported as bad whatever its place, so the
       value is checked before the count.
  3. **Writing.** In `save_settings_with_library`, one line per program,
     `external_program = <name> | <path>`, in the list's order, written after the lines of
     `shell_lines` and before the library's `favourite` and `recent` lines. A program that would
     not be read back as it is (its name empty, with spaces at either end, a `|` or a line break;
     its path not UTF-8 or not passing the test of point 2) is left out, as `library_lines` leaves
     out such a folder; of the programs left, the first five are written. With an empty list no line is written, so the file
     of `saved_file_has_the_comment_line_and_every_key` is unchanged.
  4. **Tidying names.** A function in `settings.rs`,
     `pub fn tidied_programs(programs: &[ExternalProgram]) -> Vec<ExternalProgram>`, used by the
     Settings window when it is saved (point 6): each name has every `|` and line break removed
     and is trimmed; a name that is then empty becomes the file name of its path without the
     extension (`Path::file_stem`, lossy), and, if that is empty too, the program is dropped; at
     most `MAX_EXTERNAL_PROGRAMS` are kept. The paths are not changed.
  5. **The section "External Programs"** in `draw_settings_window`, after the theme and before the
     Save and Cancel buttons (E-Q2):
     - a heading "External Programs";
     - one row per program in the list's order, each inside `ui.push_id(index, …)`: a one-line
       text field for the name, the path in small text under it, a button "Move Up" (disabled on
       the first row) and a button "Remove". The window is `.resizable(false)` and sizes itself to
       its content, so give it a fixed width (for example `.default_width(…)` with `.max_width(…)`
       of the same value, a `const` beside the code) so that a long path wraps instead of
       widening the window;
     - with no program, the line "No programs added." in place of the rows;
     - under the rows a button "Add…". It opens the Windows file dialog (`rfd::FileDialog`, the
       new helper beside `pick_folder`) with the title "Choose a Program" and one filter,
       "Programs", for `exe`. A chosen file is appended with the file's name without `.exe` as its
       name. With five programs "Add…" is disabled (`ui.add_enabled`) and its tooltip, shown with
       `on_disabled_hover_text`, says "At most 5 programs"; a cancelled dialog changes nothing.
       The same path may be added twice (E1 does not refuse it).
     - Move Up swaps the row with the one above it; Remove takes the row away. The rows are drawn
       with `iter_mut().enumerate()` (the name fields edit in place); a click on Move Up, Remove
       or Add… is kept in a local and applied after the loop, not while the list is borrowed.
       Nothing is saved or
       applied until "Save"; "Cancel" leaves the list as it was, as for the window's other
       choices.
  6. **Saving.** When "Save" is clicked, the window's list is passed through `tidied_programs`
     before the settings are applied and written, so a name typed empty or with a `|` never
     reaches `settings.txt`. Check that `save_settings_window` keeps `external_programs` from the
     window's settings (it does if the field is in `Settings`, point 1, since it is built with
     `..chosen`); change it only if it does not.
  7. **Texts (rule 13).** Every new text comes from `shell_text.rs`: new `SettingsText` variants
     for "External Programs", "Add…", "Move Up", "Remove", "No programs added.",
     "Choose a Program" and "Programs", and a function `external_program_limit_text(n: usize)`
     that returns "At most 1 program" for 1 and "At most {n} programs" otherwise (called with
     `MAX_EXTERNAL_PROGRAMS`). No string literal shown to the user in `draw_settings_window`. The
     problems of point 2 stay as `format!` texts in `settings.rs`, as the other settings problems
     are.
  8. **Glossary.** Add to `docs/glossary.md` §6 the terms of point 7 that it lacks ("Remove" is
     there), each with "added by T200" and a short note (where it is shown); name them in the
     report.
- **Tests to add:**
  - In `settings.rs`'s `tests`:
    - the list is empty by default, and a file without `external_program` lines gives an empty
      list and no problem;
    - lists of one and of five programs round-trip through save and load in their order (names
      with spaces inside and paths with spaces, for example `Capture One` and
      `C:\Program Files\Capture One\CaptureOne.exe`);
    - the saved file of one program has its line after `companion_suffixes = …` and before a
      `favourite = …` line (save with a `SavedLibrary` that has one favourite);
    - each bad value of point 2 (no `|`, two `|`, empty name, empty path, relative path) gives one
      problem on its line, and a good line after it is still read;
    - a sixth good line gives one problem on its line and the first five are kept;
    - a program that would not be read back (a name with a `|`, a name with a space at the end) is
      left out when saving, and the others are written;
    - `tidied_programs`: `|` removed and the name trimmed; an empty name becomes the file's name
      without the extension; a program with an empty name and no file stem is dropped; seven
      programs become five; the paths are unchanged.
  - In `shell_text.rs`: the new texts in `the_settings_window_has_the_texts_of_the_task` (or a new
    test beside it), and `external_program_limit_text` for 1 and 5.
  - The Settings window itself is egui drawing and is not unit-tested; its behaviour is checked by
    the owner after E2 (ADR 0019 §4: the hand check comes after E2 and E3).
- **Not in this task:** the Image menu's "Open in {name}" and "Open in Another Program…", Ctrl+E,
  greying a program whose `.exe` is gone, opening the Settings window at this section, the targets,
  the wait for writes, the start, its limits and errors (all E2); "Show in Explorer" (E3). No ADR,
  no requirement document and no other file under `docs/tasks/` changes.
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13 for every text the user
  sees); no new dependency (`rfd` is already used); no `.unwrap()`, `.expect()` or unchecked
  indexing outside tests (Move Up and Remove work on indices: use `swap` and `remove` only on
  indices checked against the list's length, or `get`).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Open | Added (ADR 0019 E1, order 1c). Cold read (fresh agent, 2026-11-19): `lib.rs` added to the files (the private modules' re-exports, and `main.rs`'s `use` list); the `Settings` literal without `..` in `every_shell_key_round_trips_through_save_and_load` may get `..Settings::default()`; the problems' wording given in full, the bad-value check before the count, the limit on saving applied after leaving out unreadable programs; the window given a fixed width, rows in `push_id`, the disabled Add… tooltip with `on_disabled_hover_text`, clicks applied after the row loop |
| 2026-11-19 | Ready for review | Task agent: points 1–8 and the tests done in the named files (`settings.rs`, `shell_text.rs`, `lib.rs`, `main.rs`, `docs/glossary.md`, `docs/test-inventory.md`); 8 tests added (7 in `settings.rs`, 1 in `shell_text.rs`); `save_settings_window` unchanged (it keeps `external_programs` through `..chosen`); `.\tools\check.ps1` all OK, 1940 passed, 76 ignored, 0 failed, no clippy warning. Glossary §6: External Programs, Add…, Move Up, No programs added., Choose a Program, Programs, At most {n} programs. The window's width is 440 points (`SETTINGS_WINDOW_WIDTH` in `draw_settings_window`) |
| 2026-11-19 | Done | Review (reviewer, 2026-11-19): every point 1–8 and every test of the task checked against the diff; reading checks the value before the count, saving leaves out a program that would not be read back and then keeps five, `tidied_programs` as point 4, the window's clicks applied after the row loop with checked indices, no user-visible literal in `draw_settings_window`; the implementation agrees with ADR 0019 §2.1. `.\tools\check.ps1` run again by the reviewer: all OK, 1940 passed, 76 ignored, 0 failed, no clippy warning. No independent code review (one crate, no safe write, no C++ shim). Nothing left over. The owner's hand check comes after E2 and E3 (ADR 0019 §4) |
