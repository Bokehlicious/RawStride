# T174. ADR 0017 U9b, last part: the keys, the menu items, the moves and the undo in the shell — Open

- **Chain:** T169 → T170 → T171 → T172 → T173 → T174. The owner's answers U9-Q5–U9-Q10 are in
  ADR 0017 §6.1.
- **Why:** ADR 0017 §2.6, step U9b. T169–T173 give the moves, the settings, the rules, the undo
  list and the answers. This task wires them to the user: the setting in the Image menu, X and P
  moving images, Ctrl+Z moving them back, and the tabs and the view cores following.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/lib.rs` (the keys),
  `crates/pw-loupe/src/shell.rs` (three commands), `crates/pw-loupe/src/shell_text.rs` (their
  texts), `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/metadata.rs` (only point 8),
  `docs/test-inventory.md`, and this task's own file
  (status and log).
- **What:**
  1. **Commands** (`shell.rs`, `shell_text.rs`): three `Command` variants, with their arms in
     `acts_in` (`Where::Both`), `command_text`, `command_enabled`, `command_checked` and
     `run_command`:
     - `Command::MovePicked` "Move to Picked" (P), in the Image menu right after the reject item;
       greyed while the setting is off (U9-Q6);
     - `Command::MoveToSubfolders` "Move Rejected and Picked Images to Subfolders", right after
       it, a check box ticked when on (U9-Q5); a change is saved at once, as `set_sort` saves;
     - `Command::UndoMove` "Undo Move" (Ctrl+Z), the first item of the Edit menu, followed by a
       separator before today's items; greyed while the undo list is empty or a batch or an undo
       runs (U9-Q8).

     Extend the menu-order tests (`shell.rs`, `shell_text.rs`) with the new items.
  2. **Keys** (`lib.rs`): P (no modifier) and Ctrl+Z give the two commands in the contact sheet
     and the preview. `key_action` refuses Ctrl and `grid_key_action` takes only Ctrl+A, so add a
     key function for them (or `ShellAction` variants) read where the other shell commands' keys
     are read, with the same guards (no dialog or metadata window open, not while egui wants the
     keyboard, `egui_wants_keyboard_input`), and teach the test helper `command_of_key` the same
     order. Neither is a key of another key map.
  3. **X with the setting on** (`main.rs`): the key and the menu both end in `rate()`. Before
     `rate` sets the ratings, it takes the target paths (as `set_ratings` will get them), resolved
     to absolute paths with `resolve_paths`, and each one's `current_rating` (T173). After the
     ratings are set, a T172 `Batch` is made of those images whose new rating is rejected and
     whose `target_folder(.., Reject)` is `Some`, each with its rating before the key as
     `restore`. An X that clears a reject moves nothing. **P with the setting on**: the same
     targets (the marked images in the contact sheet, otherwise the current one; ADR 0014 §2.7),
     resolved, with `target_folder(.., Pick)` and `restore: None`; no rating changes. P with the
     setting off does nothing. `rate` hands what the batch needs back to its callers without
     changing what it does today.
  4. **Sending** (every frame, right after the metadata cache's `poll`, where the rating writes
     are pumped): one batch or one undo at a time (a later key's batch waits behind it).
     `to_send(is_queued)` gives the images; each is sent with `move_image(image, target,
     SUPPORTED_EXTENSIONS as Vec<String>, settings' companion_suffixes)` and marked `sent` when the
     worker took it; `QueueFull` leaves it for the next frame; `Stopped` answers it at once as
     failed with `worker_stopped_text` (exported by T172). The request ids sent are kept, and an
     answer whose id is not the running batch's or undo's is ignored.
  5. **Moved** (`take_moves`): `Moved` answers go to the running batch. When it is `done`:
     - for each folder of the moved images, every tab of that folder (`tabs_of_folder_mut`) gets
       `replace_files` without the moved paths (every `from` of the moves; a tab's paths may be
       relative, so they are compared after `resolve_paths`, and the list given is the tab's own
       paths). The active tab is found by comparing its folder with `same_folder`; it gets the
       real `ratings_ready`, the others `false` (as `Tabs::follow_sort` does). For the active tab
       the view cores and the thumbnails follow as after a reorder (`loupe_follows_visible`, the
       thumbnail reset of `active_reordered`); the compare closes by itself when its held image
       left (`follow_compare`);
     - the step is pushed on the undo list;
     - failures are shown in the status line (`self.message`) with `move_failed_text` for one and
       `moves_failed_text` for more, and go to the Tasks error list as rating write failures do;
       a failed image keeps its new rating and stays in the list.
  6. **Undo** (Ctrl+Z and the menu item): `pop` a step; nothing to pop shows
     `nothing_to_undo_text`. Each `MovedImage` is sent with `move_back(moved)` (`QueueFull` and
     `Stopped` as in point 4); the step is the running undo until every `MovedBack` of it has
     come. Then:
     - the images moved back that have a `restore` get `restore_rating(image, restore)` (the undo
       of an X, U-Q6);
     - for each folder of the images, every tab of that folder gets the folder listed again
       (`list_folder(tab's folder)`) through `replace_files`, with `current` the entry of that
       list whose resolved path is the first restored image; then `forget_sort` (it clears every
       tab's sort; accepted, each is sorted again when shown) so the order is applied again
       (`follow_sort`); the active tab's view cores and thumbnails follow as in point 5;
     - when the active tab is of that folder and a restored image is not in the metadata cache
       (`has_image`, T173; the tab was loaded again since the move), the active tab's ratings are
       loaded again as when the tab is opened;
     - failures are shown with `undo_failed_text` (and in the Tasks error list); a failed image
       stays where it is, and the step is not pushed back. Otherwise `undone_text(count)` is shown,
       after all the steps above, so nothing overwrites it.
  7. Answers that come while the metadata window is open are handled as at any other time (the
     keys cannot start a move while it is open). Nothing else changes. A file that another program
     holds open fails with T169's reason and nothing of that image is moved (ADR 0017 §2.6).
     Known limit (review of T170): between the worker's release and the move a view core may
     open the file again (a render or prefetch of a near image); the move then fails with that
     reason. Nothing here prevents it; the hand check shows whether it happens (`backlog.md`,
     "Leftovers from the review of T170").
  8. **`crates/pw-loupe/src/metadata.rs`** (only this; added to the files by the review of T173):
     T173's functions carry `#[cfg_attr(not(test), allow(dead_code))]` until this task uses
     them. Remove the attribute (and the comment line "T174 … uses these; until then only the
     tests do" above each group) from `MetadataSubmit::move_image`, `MetadataSubmit::move_back`,
     `take_moves`, `is_queued`, `current_rating`, `restore_rating` and `has_image`. One the
     program still does not use keeps its attribute, and the report says which and why.
- **Tests to add:** `lib.rs`: P and Ctrl+Z give their commands in both views with exactly those
  modifiers and are in no other key map. `shell.rs`, `shell_text.rs`: the order and texts of the
  new items (extending the existing lists). The wiring in `main.rs` has no unit test (`Loupe`
  cannot be built in a test); the owner's hand check covers it.
- **Not in this task:** a Settings page for the suffixes (U8 or U10b, U9-Q10); an undo for
  ratings or edits; remembering the undo list after a restart.
- **Rules:** AGENTS.md §3. Because this is the last task of a long chain in `main.rs`, it gets an
  independent code review before the owner's approval. `docs/tasks/README.md` rule 13 (T172's
  texts, and this task's menu texts in `shell_text.rs`, which match T172's glossary rows).
- **Hand check by the owner** (ADR 0017 §7), for the whole step U9b, on a copy of R2's folder
  (`RAW\RAW-DxO`): with the setting on, X moves an ARW with its `.ARW.dop` and `.XMP` into
  `_rejected` and it leaves the contact sheet; DxO PhotoLab still shows its edits there; P moves
  one to `_picked` without a rating change; several marked images move with one key; Ctrl+Z moves
  the last batch back and restores the ratings; inside `_rejected` X only sets the rating; with
  the file open in another program the move fails, the message names it, and nothing moved; with
  the setting off X only sets the rating and P does nothing.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added for ADR 0017 U9b; rewritten after the cold read of the first T171–T172 (`rate()` gives the targets, absolute paths, every tab of the folder, the message set last, one batch or undo at a time); waits for T173 |
| 2026-11-17 | Blocked | Cold read: the command arms named (`acts_in`, `command_text`, `command_enabled`, `command_checked`, `run_command`) and the menu places; a key function for P and Ctrl+Z with the usual guards; `QueueFull`/`Stopped` for `move_back`, answers of other ids ignored; the active tab by `same_folder`, `ratings_ready` false for the others; the undo's `current` in the tab's own form; `forget_sort` clears every tab (accepted); the ratings after an undo through `has_image` and a reload; failures also in the Tasks error list; an independent code review |
| 2026-11-17 | Blocked | Review of T170: also waits for T177 (a move's release notes no histogram stamp); the reopening between release and move named as a known limit in point 7 |
| 2026-11-17 | Blocked | Review of T172: a finished batch's `moved` can hold an image with no moves of its own (a partner moved by another image), so its rating is restored too; its `move_back` of an empty list answers `Ok` at once and it counts in `undone_text` |
| 2026-11-17 | Blocked | Review of T173: point 8 added (the `allow(dead_code)` of T173's seven functions removed once they are used); `metadata.rs` added to the files for it only |
| 2026-11-17 | Open | T173 and T177 are done |
