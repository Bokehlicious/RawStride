# T243. The Label submenu right after the Rating submenu, in the Image menu and the right-click menu (pw-loupe) — Done

- **Why:** after using the program, the owner decided (2026-11-21, ADR 0019 §2.9, E-Q17) that the
  Label submenu comes on the row right below the Rating submenu, in the Image menu of the menu bar
  and in the right-click menu on images. Today Reject and Move to Picked (and, in the Image menu,
  "Move Rejected and Picked Images to Subfolders") sit between them. Only the order changes: no
  command, key, text or greying rule changes.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/shell.rs` (points 1–2 and the tests below)
  - `crates/pw-loupe/src/shell_text.rs` (the expected lists of two tests below, nothing else)
  - this task's own file (status and log)
- **What:**
  1. In `MENUS`, the `MenuKind::Image` menu's entries begin, in this order:
     `MenuEntry::Submenu(SubmenuKind::Rating, RATING_ITEMS)`,
     `MenuEntry::Submenu(SubmenuKind::Label, LABEL_ITEMS)`, `item(Command::Reject, …)`,
     `item(Command::MovePicked, …)`, `item(Command::MoveToSubfolders, None)`, then
     `MenuEntry::Separator`, `item(Command::RotateLeft, …)`, `item(Command::RotateRight, …)` and
     the rest of the menu unchanged from there on (the separator, RAW ↔ JPEG, …). Each entry keeps
     its key; nothing is added or removed.
  2. In `IMAGE_CONTEXT_MENU`, the entries after the first separator are, in this order: the Rating
     submenu, the Label submenu, Reject, Move to Picked; then the rest unchanged (the separator,
     Metadata…, the separator, Delete to Recycle Bin). The doc comment above
     `IMAGE_CONTEXT_MENU` only refers to ADR 0019 §2.9 and E-Q17 and stays as it is.
- **Tests:** four existing tests lock the old order. Change exactly these four, and no other test
  (this is the owner's change of the order, not a test weakened to pass; README rule 5). In
  `shell.rs`'s `tests` module:
  - `the_image_context_menu_holds_the_image_commands_in_order`: the expected array in point 2's
    order; its doc comment also names the owner's change (ADR 0019 §2.9, 2026-11-21, T243).
  - `the_rotate_items_follow_the_label_submenu_in_the_image_menu`: rename it
    `the_image_menu_begins_with_rating_and_label_then_the_rotate_items`. It asserts that
    `image.entries.get(..10)` equals this array of ten entries: the Rating submenu, the Label
    submenu, Reject, Move to Picked, Move to Subfolders, a separator, Rotate Left, Rotate Right, a
    separator, `item(Command::ToggleMode, plain(Key::R))`, each with its key as in `MENUS`. The
    lookup of the Label submenu's position and its `panic!` are no longer needed and go. Keep its
    other asserts as they are (both rotate commands act in both views and are not in the
    right-click menu). Its doc comment names ADR 0024 §2.1, O-Q1, T237 and ADR 0019 §2.9
    (2026-11-21, T243).

  In `shell_text.rs`'s `tests` module (both read `menu_items()`, which lists the Image menu in
  table order):
  - `every_menu_item_text_in_menu_order`: the six Label texts ("Red" … "No label") move to right
    after "5 stars", before "Reject".
  - `every_menu_key_text_in_menu_order`: the six Label keys (`Some("6")` … `Some("Ctrl+0")`)
    move to right after `Some("5")`, before `Some("X")`.
  - No test is added, so the test count is unchanged and `docs/test-inventory.md` is **not**
    changed (it is T240's file; if `check.ps1` or anything else asks for a change there, stop
    and ask). Its line for `pw-loupe` describes the old order of the right-click menu and of
    the rotate items; the reviewer updates that description at T243's Done, after T240 is
    committed.
- **Not in this task:** any other menu, any key, text, greying rule or command; `main.rs` (it
  draws both menus from these tables, so the new order shows with no change there); the text
  functions in `shell_text.rs`; ADR 0019 (the reviewer has written the decision).
- **Parallel work:** T240 runs at the same time and changes other files (`lib.rs`, `sort.rs`,
  `tabs.rs`, `browse.rs`, `grid.rs`, `docs/test-inventory.md`). Do not touch them, and do not
  revert or format their uncommitted changes. Run `.\tools\check.ps1` only when T240's agent is not
  running it (README "Parallel tasks"); if its changes make the build or a test fail, stop and
  report that instead of fixing them.
- **Rules:** `AGENTS.md`; README rules 1–12.
- **Done when:** `.\tools\check.ps1` is all OK, and this task has changed only `shell.rs`,
  `shell_text.rs` and its own file (T240's uncommitted files are not this task's).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Open | Added after the owner's decision (2026-11-21), written into ADR 0019 §2.9. Cold read: two tests in `shell_text.rs` (`every_menu_item_text_in_menu_order`, `every_menu_key_text_in_menu_order`) also lock the order, so `shell_text.rs` added for those two only; the renamed test's assert written as `get(..10)` with the dead Label lookup dropped; the context menu's doc comment stays; `docs/test-inventory.md`'s description of the order is left to the reviewer at Done (T240's file) |
| 2026-11-21 | Ready for review | Done as written. `shell.rs`: `MENUS` Image menu begins Rating, Label, Reject, Move to Picked, Move to Subfolders, separator, Rotate Left, Rotate Right; `IMAGE_CONTEXT_MENU` after the first separator: Rating, Label, Reject, Move to Picked (its doc comment unchanged). Tests: `the_image_context_menu_holds_the_image_commands_in_order` (expected array, doc comment names T243); `the_rotate_items_follow_the_label_submenu_in_the_image_menu` renamed `the_image_menu_begins_with_rating_and_label_then_the_rotate_items` (`get(..10)`, Label lookup and its `panic!` dropped, other asserts kept). `shell_text.rs`: the Label texts and keys moved in `every_menu_item_text_in_menu_order` and `every_menu_key_text_in_menu_order`. No test added; `docs/test-inventory.md` not changed (its description of the order is for the reviewer). check.ps1 all OK: 2080 passed, 76 ignored, no clippy warnings |
| 2026-11-21 | Done | Reviewed (reviewer): as written; only `shell.rs`, `shell_text.rs` and this file. The reviewer's clean run of check.ps1: all OK, 2080 passed, 76 ignored, no clippy warnings. At Done the reviewer updated `docs/test-inventory.md`'s description of the two orders (count unchanged) and added a CHANGELOG "Changed" line (the menus were in 0.1.0). Moved to `done/`; the owner's hand check follows |
