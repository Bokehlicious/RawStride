# T202. ADR 0019 E2, second part: the menu items, Ctrl+E, the question and the start in the shell (pw-loupe) — Blocked: waits for T201

- **Chain:** T200 (E1, done) → T201, T202 (E2) → T203 (E3). The owner's answers are E-Q1–E-Q14 in
  ADR 0019 §3. The owner's hand check comes after E3 (ADR 0019 §4) and is written in T203.
- **Why:** ADR 0019 §2.2–§2.6, step E2. T200 gives the program list, T201 the plan, the wait, the
  start and the texts. This task wires them to the user: Image ▸ "Open in {name}" per program,
  "Open in Another Program…" with an empty list, Ctrl+E, the question above 100 images, the wait
  for the images' writes, the start and its errors.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/shell.rs` (two commands and one menu
  entry), `crates/pw-loupe/src/shell_text.rs` (their texts), `crates/pw-loupe/src/lib.rs` (the
  key), `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/metadata.rs` (only point 9),
  `docs/test-inventory.md`, and this task's own file (status and log). **Starts after T201 is
  committed.**
- **What:**
  1. **Commands and the menu entry** (`shell.rs`):
     - `Command::OpenIn(u8)`: open the targets in the program at that index of
       `Settings::external_programs` (0 is the first, the default program, E-Q1), and
       `Command::OpenInAnother`: open the Settings window (E-Q3). Both act in `Where::Both`
       (the default arm of `acts_in`).
     - `MenuEntry::ExternalPrograms`: drawn from the settings' list, as `MenuEntry::Filter` and
       `MenuEntry::Sort` are drawn from their own data (point 4). In the Image menu it comes
       after `item(Command::ToggleMode, …)` and a separator, followed by a new separator and
       then the two delete items, so the menu reads: …, ToggleMode, separator, the programs,
       separator, Delete to Recycle Bin, Delete Rejected Images. T203 adds "Show in Explorer"
       after the programs, before that separator.
     - `menu_items()` skips `MenuEntry::ExternalPrograms` (as `Filter` and `Sort`): its items and
       Ctrl+E are not in the static table.
     - `pub const OPEN_IN_KEY: KeyChord` = Ctrl+E (no Shift, no Alt), the key the first
       program's item (or "Open in Another Program…") shows on its right; exported in `lib.rs`'s
       `pub use shell::{…}` (`main.rs` is a separate binary crate).
     - The test `the_delete_items_are_the_last_items_of_the_image_menu` (`shell.rs` about line
       1036) checks the last four entries from `ToggleMode`; with the new entry it must take the
       last six (`checked_sub(6)`), its expected slice gets `MenuEntry::ExternalPrograms` and a
       separator after ToggleMode's separator, and its doc comment says so. This change is
       allowed by this task (an exception to rule 5: what it checks is the same, the menu
       grew). `every_menu_item_text_in_menu_order` and `every_menu_key_text_in_menu_order`
       (`shell_text.rs`) do not change, since `menu_items()` skips the new entry.
  2. **Texts** (`shell_text.rs`, rule 13): `command_text(Command::OpenIn(index))` is T201's
     `program_number_text(index as usize + 1)` (it is never shown in the menu, which draws
     `open_in_text(name)`; it exists because `command_text` takes no name), and
     `command_text(Command::OpenInAnother)` is T201's `open_in_another_text()`. A new test in
     `shell_text.rs` checks both (one `OpenIn(0)` and one `OpenIn(4)`).
  3. **The key** (`lib.rs`): `pub fn open_in_key_action(key: egui::Key, ctrl: bool, shift: bool,
     alt: bool) -> bool`: `egui::Key::E` with Ctrl and no other modifier. Read in `main.rs` right
     after the Delete key (about line 5860), with the same guards (not while
     `ctx.egui_wants_keyboard_input()`, nothing while `dialog_open()`) and **not on a repeat** (a
     held Ctrl+E must not start the program again each repeat); it runs `OpenIn(0)` when
     the list has a program, otherwise `OpenInAnother`, through `run_command`. Teach the test
     helper `command_of_key` (`lib.rs` tests, about line 1015) Ctrl+E as `Command::OpenIn(0)`,
     after the Delete key. E without Ctrl stays the focus marking: `preview_key_action` and
     `key_action` refuse Ctrl+E today; a test says so.
  4. **The menu entry** (`main.rs`, in `draw_menu_bar`, a new arm for
     `MenuEntry::ExternalPrograms` calling a new `external_programs_menu(&self, ui, commands)`):
     - with an empty list: one item `open_in_another_text()` with `key_text(OPEN_IN_KEY)` on its
       right, enabled as `command_enabled(Command::OpenInAnother)`, pushing that command;
     - otherwise one item per program in the list's order, `open_in_text(&program.name)`, the
       first with `key_text(OPEN_IN_KEY)` on its right; enabled when
       `command_enabled(Command::OpenIn(index))` **and** `program.path.is_file()` (a program
       whose `.exe` is gone is greyed, E-Q9; the check runs only while the menu is open); a
       click pushes `Command::OpenIn(index)`. The index is at most 4 (`MAX_EXTERNAL_PROGRAMS`),
       so `u8::try_from(index)` never fails; a failure skips the item, no panic. No string
       literal in the drawing (rule 13).
  5. **Enabled and run** (`main.rs`, `command_enabled` and `run_command`):
     - `OpenIn(index)`: enabled while the active tab shows a current image
       (`tab.position().is_some()`, as the rating items) and `index` is within the list. The
       missing `.exe` is not checked here: the key then says why (point 6), the menu greys it
       (point 4). Runs `open_in(index)`.
     - `OpenInAnother`: always enabled; runs `open_settings_window()` (the Settings window has
       no scrolling, so its section "External Programs" is in view when it opens; nothing more is
       needed for "at External Programs", ADR 0019 §2.2).
  6. **Starting a request** (`main.rs`, a new `open_in(&mut self, index: usize)`):
     - the program is `self.settings.external_programs.get(index)` cloned; none, nothing;
     - the targets are `key_targets()` (in the contact sheet the marked images, otherwise the
       current one; in the preview only the current one, E-Q4, as Delete), resolved with
       `resolve_paths`; an error goes to the status line (`self.message`); no targets, nothing.
       Only the image files as the tab lists them, no partner or sidecar (E-Q5);
     - the program's path not a file (`is_file`): the status line shows
       `program_missing_text(name, &path.display().to_string())` and the same text goes to the Tasks error list
       (`self.tasks.add_error(…, ClockTime::now())`, as a failed move's), nothing else (E-Q9);
     - otherwise `plan_open(program, images)`: `TooLong { count, fit }` → the status line shows
       `too_long_text(count, name, fit)`, nothing started (E-Q8); `Ask(request)` → the step
       `self.open` (a new `OpenStep` field in `Loupe`) becomes `Confirming(request)` and a
       `bool` asks the Cancel button for the focus on its first frame (as
       `delete_focus_cancel`); `Start(request)` → `Waiting(request)`, which **replaces** a request
       that was waiting (ADR 0019 §2.4: the earlier one is dropped, not started), and, when the
       request is not ready at once (`ready(|path| self.metadata.write_pending(path))`), the
       status line shows `waiting_for_writes_text()` (set once here, not every frame).
     - **A request that waits:** `Ask` replaces it too (the user gave a new command; when the
       question is then cancelled, neither is started). A command that ends in no new request
       (no targets, a path error, a missing program, `TooLong`) leaves a waiting request as it
       was. The waiting request holds its own copy of the program, so a program renamed or
       removed in Settings meanwhile is still started by its old name and path (Known limits).
     - Whenever the step goes to `Idle` without a start (Cancel, or a request replaced and then
       cancelled) and `self.message` still holds `waiting_for_writes_text()`, the message is
       cleared, so "Waiting for writes …" never stays behind.
  7. **The question above 100 images** (`main.rs`, a new `draw_open_window(&mut self, ctx)`
     called beside `draw_delete_window`, about line 8147), drawn as the delete's confirmation is
     (T189 point 4), as an `egui::Modal` with its own id:
     - content: `open_many_question_text(request.images.len(), &request.program.name)`, buttons
       `open_button_text()` and `open_cancel_text()`, each with a fixed id; the window's width a
       `const` as `DELETE_WINDOW_WIDTH`;
     - **Cancel is the default** (ADR 0019 §2.5): Cancel's button gets `request_focus()` on the
       first frame only; the Open button's id is kept (as `delete_button`) for the keys;
     - **keys:** a branch at the top of the key loop, beside the delete's (about line 5756),
       takes every key while the question is shown: Escape cancels; Enter cancels unless the
       Open button has the focus (then egui clicks it); every other key does nothing;
     - Cancel, Escape or a click outside (`should_close`) → `Idle`, nothing started; Open →
       `Waiting(request)`, and the status line shows `waiting_for_writes_text()` when it is not
       ready at once, as point 6;
     - **one dialog at a time:** the question is shown when the step is `Confirming` and neither
       the metadata window, the Settings window nor a shown import dialog is open (none can open
       while it is shown, since `dialog_open()` is true; the check is for the frame it begins).
       This guard is written out on its own (a new `open_window_shown(&self)`); it must not call
       `other_dialog_open()` or `dialog_open()`, which are true while the question itself is
       `Confirming`, or the question would never show.
       `dialog_open()` and `other_dialog_open()` both return true while the open step is
       `Confirming`, so the delete's confirmation waits behind it (a delete that was preparing
       when Ctrl+E was pressed) and the menus are greyed. Every other place that guards on
       `delete_window_shown()` or `DeleteStep::Confirming` as a modal (the file drop, about line
       5666: `&& !matches!(self.open, OpenStep::Confirming(_))` beside the delete's; the wheel,
       about line 5993: `&& !self.open_window_shown()`) guards on the open question too. Unlike the delete's,
       the question does not depend on the active tab (the request holds its own paths), and
       the bool that asks Cancel for the focus is kept until the window is first drawn (commands
       run after the draw calls, so the question shows on the next frame).
  8. **The follow-up** (every frame, right after `follow_deletes`, a new `follow_open(&mut self,
     ctx)`): while the step is `Waiting(request)`:
     - it also waits while a move or an undo runs or waits (`self.moves.busy()`) or the delete
       step is not `Idle`: those wait for the same rating writes and run earlier in the frame
       (`follow_moves`, `follow_deletes`), so without this a file could be moved or deleted in
       the very frame the program is given its old path;
     - when not waiting so and `request.ready(|path| self.metadata.write_pending(path))`: take
       the request (the step is `Idle`); the images that are no longer files (`is_file`: moved
       or deleted while the request waited) are left out, each named in the Tasks error list
       with `image_gone_text(file name)` (T201); with none left the status line shows
       `nothing_to_open_text()` and nothing is started; otherwise `start_program` with the
       request of the images left. `Ok` → the status line shows `opened_text(images left,
       name)`; `Err(Missing)` → `program_missing_text` (the `.exe` was
       removed while the request waited) and `Err(Failed(reason))` → `start_failed_text(name,
       reason)`, each in the status line **and** the Tasks error list (E-Q9). A write that
       failed meanwhile was reported by its own path, and the program is started anyway (ADR
       0019 §2.4);
     - otherwise a frame is asked for (`ctx.request_repaint_after(MOVE_POLL_INTERVAL)`).
     Nothing is started from anywhere else; RawStride does not wait for or watch the program
     (E-Q7). The start is not held back by an open dialog (the metadata or Settings window): the
     request was given before it opened. It is not held back by a running move or delete either
     (Known limits).
  9. **`crates/pw-loupe/src/metadata.rs`** (only this): remove T201's
     `#[cfg_attr(not(test), allow(dead_code))]` and its comment from `write_pending`.
- **Tests to add:** `lib.rs`: Ctrl+E gives `open_in_key_action`, not E alone, not Ctrl+Shift+E or
  Ctrl+Alt+E; Ctrl+E is in no other key map (`key_action`, `grid_key_action`,
  `preview_key_action`, `shell_key_action`, `move_key_action`, `rating_key_action`,
  `tab_action`, `opens_import`). `shell.rs`: the Image menu's entries in order with
  `ExternalPrograms` and its separator, checked by the rewritten
  `the_delete_items_are_the_last_items_of_the_image_menu` of point 1 (no separate order test);
  `menu_items()` has no `OpenIn` and no `OpenInAnother`;
  `OPEN_IN_KEY` is Ctrl+E. `shell_text.rs`: the two command texts, and `key_text(OPEN_IN_KEY)` is
  "Ctrl+E". The wiring in `main.rs` has no unit test (`Loupe` cannot be built in a test); the
  owner's hand check after T203 covers it.
- **Known limits** (to `backlog.md` when this task is Done): quitting while a request waits drops
  it (the program is not started, nothing said); the wait has no time limit, so a worker that
  never answers a write, or a user who keeps rating the targets, holds the start back (the
  status line keeps "Waiting for writes …"); an image moved or deleted while its request waits
  is left out by its old path (point 8), not followed to its new place; after Save, a program
  whose path is not UTF-8 is used until a restart although T200 does not write it to the file;
  a request waits only for writes already queued when it is checked, so a
  rating key pressed on one of its images after the start is not seen by the other program
  until it reads the file again (ADR 0019 §2.7); a program renamed or removed in Settings while
  a request waits is still started by its old name and path; the Settings window has no
  scrolling, so on a very small screen its section "External Programs" may be cut off; Windows'
  reason in `start_failed_text` is in Windows' language. The agent does not edit `backlog.md`;
  the reviewer moves these there.
- **Not in this task:** "Show in Explorer" (T203); Windows' default program (E-Q10); anything of
  ADR 0019 §5.
- **Rules:** `AGENTS.md` §3; `docs/tasks/README.md` rule 13 (the texts are T201's and this task's
  `command_text` arms; no string literal in the drawing code). The program is started while the
  user keeps working, and the task wires a modal and keys in `main.rs`, so it gets an
  independent code review before the owner's approval.
- **Done when:** `.\tools\check.ps1` is all OK with no clippy warnings and `git diff --stat`
  shows no change from this task outside the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (ADR 0019 E2, order 1c), with T201 and T203 as one chain; waits for T201. Cold read (fresh agent, 2026-11-19): `OPEN_IN_KEY` exported; the delete-items test rewritten for six entries (allowed); a new test for the two command texts; the path given with `display()`; Ctrl+E not on a repeat; an `Ask` replaces a waiting request, a command with no new request leaves it; "Waiting for writes …" cleared when nothing is started; the question independent of the tab and its focus bool kept to the first drawn frame; the start not held back by a dialog; new known limits; the agent does not edit `backlog.md`. Chain review: see T201's log |
