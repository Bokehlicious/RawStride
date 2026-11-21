# T244. ADR 0017 U15a: the metadata window steps only between the marked images (pw-loupe) — Open

- **Why:** ADR 0017 §2.3.1 as amended by the owner on 2026-11-21 (addition U15, U15-Q9): with the
  metadata window's image marked and at least two images marked, "Previous", "Next" and "Save
  and Next" (also as keys) step only between the marked images, in the contact sheet's order
  (after the filter), however the window was opened (`M`, the menus, the right-click menu, and
  the thumbnail button of T245). Otherwise they step between all the shown images, as today.
  Today `window_neighbour` (`main.rs`) always steps over `tab.visible`, so with ten images marked
  among a hundred the user steps through the ninety that are not marked too. First task of the
  chain T244 → T245 (plan `adr-0018-0025-steps.md`, order 2a, row U15; U15-Q8). No hand check of
  its own: the owner's hand check of U15 follows T245.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/browse.rs` (point 1 and its tests)
  - `crates/pw-loupe/src/lib.rs` (point 2 only: the re-export)
  - `crates/pw-loupe/src/main.rs` (point 3 only: `window_neighbour` and its doc comment)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **`metadata_steps(visible: &[usize], marked: &[usize], current: usize) -> Vec<usize>`**
     (`browse.rs`, right after `metadata_neighbour`): the list the metadata window on file index
     `current` steps over. `marked` is the marked images in the shown order, as
     `Selection::marked_in(visible)` gives them. When `marked` has two or more entries and
     `current` is one of them, it returns `marked`; otherwise it returns `visible`. Both are
     returned as they are, in their order (the contact sheet's order after the filter). The count
     is of every marked image, kinds without metadata support included (as `marked_subjects`
     counts them); `metadata_neighbour` skips those kinds when it steps. Its doc comment names
     ADR 0017 §2.3.1 as amended 2026-11-21 (U15-Q9, T244) and says that the rule holds however
     the window was opened. It also says that the rule differs on purpose from the preview's
     "only the marked" (`Tab::preview_list`, which does not need the current image marked): the
     window steps between the marked only when its own image is one of them (U15-Q9).
     `metadata_neighbour` itself is not changed, except that its doc comment's "Föregående" and
     "Nästa" become the buttons' English names "Previous" and "Next".
  2. **Re-export** (`lib.rs`): add `metadata_steps` to `pub use browse::{…}` beside
     `metadata_neighbour`.
  3. **`window_neighbour`** (`main.rs`): build the step list with
     `metadata_steps(&tab.visible, &tab.selection.marked_in(&tab.visible), current)` and give that
     list to `metadata_neighbour` in place of `&tab.visible`; nothing else in the function
     changes. Add `metadata_steps` to the `use pw_loupe::{…}` list. Its doc comment names the
     buttons in English ("Previous", "Next", in place of "Föregående" and "Nästa") and says that it
     steps between the marked images when the window's image is marked and two or more are, else
     between the shown images (U15-Q9, T244). The marking is read from the tab each time, so it is
     the marking as it is at the step (the window is modal; a rating chosen in the window can
     refilter the tab when the window moves, U10c-Q4, and the refilter may unmark images it
     hides, `Tab::refilter`). Every caller of `window_neighbour` (the enabled state of the
     Previous, Next and Save and Next buttons and their keys, and `leave_metadata_window`'s move)
     then follows the rule with no other change.
- **Tests to add** (`browse.rs`, under the heading comment of the metadata window's previous /
  next, after the existing `metadata_neighbour_*` tests):
  - `metadata_steps_are_the_marked_when_the_image_is_marked_and_two_or_more_are`: visible
    `[0, 1, 2, 3, 4, 5]`, marked `[1, 3, 5]`, current 3 → `[1, 3, 5]`;
  - `metadata_steps_are_the_shown_images_when_the_image_is_not_marked`: the same lists, current 2
    → the visible list;
  - `metadata_steps_are_the_shown_images_with_only_one_marked`: marked `[3]`, current 3 → the
    visible list; marked empty, current 3 → the visible list;
  - `metadata_steps_with_metadata_neighbour_step_only_between_the_marked`: visible
    `[0, 1, 2, 3, 4, 5]`, marked `[1, 3, 5]`, every kind supported: from 3 forward → `Some(5)`,
    back → `Some(1)`; from 5 forward → `None`; from 1 back → `None` (no wrap); with file 3 a kind
    without support, from 1 forward → `Some(5)`;
  - `metadata_steps_keep_the_contact_sheet_order`: visible `[4, 0, 2]` (a sort order that is not
    the file order), marked in that order `[4, 2]`, current 2 → `[4, 2]`, and with
    `metadata_neighbour` from 2 back → `Some(4)`;
  - `metadata_steps_count_marked_kinds_without_support`: visible `[0, 1, 2, 3, 4]`, marked
    `[1, 3]`, file 3 a kind without support, current 1 → `[1, 3]`, and with `metadata_neighbour`
    from 1 forward → `None` (the window does not fall back to the images not marked).
  `window_neighbour` itself is drawn code's helper in the binary and has no test of its own; the
  rule lives in `metadata_steps`.
- **Not in this task:** the buttons on a thumbnail (T245). Where the window opens: `M` and the
  menus open it, as today, on the current image if it is marked, else on the first marked image
  when two or more are (`metadata_subject`, U12c-Q1); that is unchanged. "Apply to {n} Marked"
  is unchanged (its list is still the marked images when the window opened; it is not updated
  when a refilter after a rating in the window unmarks an image, as today). The preview's own
  steps with "only the marked" are unchanged.
- **Rules:** `docs/tasks/README.md` rules 1–13; no new texts. No `.unwrap()`, `.expect()`,
  `panic!`, `unreachable!` or unchecked indexing outside tests.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Open | Added (ADR 0017 U15a, U15-Q9; first task of the chain T244 → T245) |
| 2026-11-21 | Open | Cold read: doable with the named files; every caller goes through `window_neighbour`. Changed: the doc comments get the English button names; `metadata_steps`' doc comment says why it differs from `preview_list`; a test for a marked kind without support (no fall back). The case of an image unmarked by a refilter after a rating in the window is a hand-check item in T245 |
| 2026-11-21 | Open | Chain review (T244 → T245): no conflict between the tasks (T245 relies only on what T244 does; T244 does not touch `open_metadata_window`). Changed in T245: the open-window guard moves into `open_metadata_window_on`; after `right_click` the button acts only if its image is current and shown, and reads the marking after it; a right click on a button is the cell's; a button click followed by a cell click is not a double click; the metadata fact explained; two hand-check items. Here: "Apply to {n} Marked"'s frozen list named under Not in this task |
