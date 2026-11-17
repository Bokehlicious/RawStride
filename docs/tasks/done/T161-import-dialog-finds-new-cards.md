# T161. The import dialog finds a card inserted while it is open — Done

- **Why:** the owner's job test (2026-11-16): a card reader or card put in while the import dialog
  is open does not show up. The dialog asks the import worker for the cards
  (`pw_ingest::card_volumes`: drives with a `DCIM` folder) only when it opens
  (`open_import`, `main.rs`) and after an eject (`poll_import_worker`). **Owner, 2026-11-16**
  (ADR 0010, "Source"): the dialog keeps its card buttons up to date by itself. A new card shows as
  a button; a card taken out goes away. A card going into a reader that is already plugged in does
  not change the drive letters (the letter is there, without media), so the dialog asks again on a
  timer instead of watching the drive letters.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import_dialog.rs`,
  `crates/pw-loupe/src/lib.rs` (the export only), `crates/pw-loupe/src/main.rs`,
  `docs/test-inventory.md`.
- **What:**
  1. In `import_dialog.rs`, a small, pure timer (derive `Debug`):
     `pub struct CardRefresh { asked_at: Option<Instant>, waiting: bool }` with
     `pub const CARD_REFRESH_INTERVAL: Duration = Duration::from_secs(2);` and
     - `pub fn new(now: Instant) -> Self`: counts the ask made when the dialog opens (`asked_at =
       Some(now)`, `waiting = true`);
     - `pub fn due(&self, now: Instant, idle: bool) -> bool`: true only when `idle`, nothing is
       waiting, and at least `CARD_REFRESH_INTERVAL` has passed since the last ask
       (`saturating_duration_since`, as `tick` does);
     - `pub fn asked(&mut self, now: Instant)` and `pub fn answered(&mut self)`.

     Export `CardRefresh` and `CARD_REFRESH_INTERVAL` from `lib.rs` with the other
     `import_dialog` items.
  2. In `main.rs`:
     - the `Loupe` keeps a `card_refresh: Option<CardRefresh>`. `open_import` sets it with the ask
       it already makes; if that `cards()` call fails, it also calls `answered()`, so the timer
       tries again after the interval. `open_import` on an already open dialog (the "show" path)
       keeps the timer it has. `close_import` clears it;
     - **where:** in `draw_import_dialog`, after the `tick` block and before the repaint at its end
       (the shown dialog repaints every 100 ms; a hidden dialog is only ever a running, ejecting
       or finished import, which needs no refresh). If `card_refresh.due(now, idle)`, call
       `self.import_worker.cards()` and `asked(now)`;
     - **`idle`** means the dialog's state is `Editing` or `Done` (not `Previewing`, `Running` or
       `Ejecting`) and no Clear confirmation is open (`import_clear_confirm`). The worker is one
       queue: an ask during a preview would wait behind the card scan and delay the next preview;
     - an error from this timed `cards()` call is **not** shown (the user did nothing): it only
       calls `answered()`. A stopped worker is already shown by the other paths;
     - the `cards()` call after an eject (`poll_import_worker`) also calls `asked(Instant::now())`
       when the timer is there, and `answered()` on its error;
     - `ImportEvent::Cards` calls `answered()` (when the timer is there) before it replaces
       `detected_cards`, as today.
  3. Nothing else changes. The source field is not filled in by itself, and the buttons are drawn
     as today from `detected_cards`.
- **Tests to add** (in `import_dialog.rs`, with `Instant` values made in the test):
  - `new` is not due before the first answer, even long after the interval;
  - after `answered`, not due before the interval and due at the interval;
  - not due while not `idle`, and due again once idle;
  - a `now` earlier than the last ask is not due and does not panic.
- **Not in this task:** watching Windows' device messages (`WM_DEVICECHANGE`); filling in the
  source; the Library's drive list (`library.rs`, U4), which has its own refresh; a faster card
  check. `card_volumes` looks at `X:\DCIM` on all 26 letters. A disconnected network drive can
  make one pass take seconds, and a Start pressed during that pass waits for it. This is accepted
  (reviewer, 2026-11-16). If the hand check shows it, it becomes a backlog item.
- **Rules:** AGENTS.md §3; no new texts.
- **Hand check (owner):**
  1. Open the import dialog with no card in. Put a card in an already plugged-in reader: its button
     shows within about 2 s. Take it out: the button goes away.
  2. Plug in a reader with a card while the dialog is open: same result.
  3. During an import nothing changes.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (besides files of other tasks still uncommitted in the tree).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Open | Added after the owner's job test (the SD reader put in while the dialog was open); the owner's decision is in ADR 0010 ("Source") |
| 2026-11-16 | Open | Cold read: `lib.rs` added for the export; a failed ask at opening counts as answered; the place in `draw_import_dialog` is named; `idle` excludes `Previewing`; timed-ask errors are not shown; the eject's ask goes through the timer; the network-drive delay is accepted and named |
| 2026-11-17 | Ready for review | `CardRefresh` and `CARD_REFRESH_INTERVAL` in `import_dialog.rs` (exported from `lib.rs`), four tests; `main.rs` asks for the cards every 2 s while the shown dialog is `Editing` or `Done` and no Clear confirmation is open, the eject's ask and `ImportEvent::Cards` go through the timer, timed-ask errors are not shown. `check.ps1` all OK: 1694 passed, 65 ignored, 0 failed |
| 2026-11-17 | Done | Reviewed: no bugs (a timed ask just before a debounced preview can delay it by one card scan; accepted under "Not in this task"). Reviewer's `.\tools\check.ps1` on this tree (T160, T161, T163 and T164): all OK, 1694 passed, 65 ignored, 0 failed; clippy without warnings; the inventory's header is this count. The owner's hand check is done after the commit (owner, 2026-11-17); a slow card check seen there becomes a backlog item, as the task says |
