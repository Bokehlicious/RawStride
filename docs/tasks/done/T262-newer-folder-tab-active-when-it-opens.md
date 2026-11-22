# T262. ADR 0026 F4: a later destination folder's tab becomes active when it opens, if the user has not moved (pw-loupe) — Done

- **Why:** the owner's hand check of T260 (2026-11-21, point 3): during an import over two dates
  the second folder's tab opened behind the first and became active only at the end. The owner
  changed F-Q10 (ADR 0026 §3.4 and §5, GR12, ADR 0010 §3.1, all 2026-11-21): **a later folder's
  tab becomes active when it opens (its first image shown), if the user has not moved since the
  import last made a tab active; otherwise it opens behind, as before. There is no rule at the
  end.** "Has not moved" is as before: the active tab's folder, current image and view are those
  kept when the import last made a tab active (the first folder's, or a later one's).
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/import_tabs.rs` (point 1, and its tests)
  - `crates/pw-loupe/src/main.rs` (points 2–3, and the tests of `placed_tabs`)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests, when tests are
    added or removed)
  - this task's own file (status and log)
- **What:**
  1. **`ImportTabs`** (`import_tabs.rs`, T259): the end rule goes, the kept position can be
     replaced.
     - `first_shown` becomes `pub fn shown(&mut self, at: ImportPosition, settling: bool)`: keeps
       `at`, the active tab's position after the import made a tab active, **replacing** any
       position kept before; `settling` is true when that tab's ratings were not read yet, so the
       position is kept again once they are (point 2).
     - `pub fn settling_in(&self, folder: &Path) -> bool`: a position is kept, it was kept with
       `settling` true, and its folder is `folder` (`same_folder_lexically`).
     - `first_kept` becomes `pub fn position_kept(&self) -> bool`: whether a position was kept.
     - `end_target` goes. In its place `pub fn unmoved(&self, now: Option<&ImportPosition>) ->
       bool`: true when a position was kept and `now` is equal to it (folder by
       `same_folder_lexically`, `current` exactly, `mode` equal), the comparison `end_target` made;
       false without a kept position or without `now`.
     - The module doc comment and `ImportPosition`'s say what the kept position is now (no end
       rule). The field `first` may be renamed. `folders()` stays.
     - Tests: the three tests of `first_shown`, `first_kept` and `end_target` (`import_tabs.rs`
       around lines 410, 418 and 454) are replaced by tests of `shown` (a second call replaces the
       first, also its `settling`), `settling_in` (true only for a settling position of that
       folder), `position_kept` and `unmoved` (equal; the same folder written with a trailing
       separator, kept from the old test; another folder, another image, no image, another view;
       `None`; nothing kept).
  2. **`Loupe::import_placed`** (`main.rs`, T260): for each group, `make_active` is true when the
     group is its folder's first (`PlacedInFolder::first`) and either it is the import's first
     shown folder (no position kept yet and the event's first group, as today) or
     `ImportTabs::unmoved` with the active tab's position now (`import_position()`, taken for each
     group before that group's fill or open) is true.
     - `placed_tabs(has_tab, first_in_folder, make_active)` (its third parameter renamed, its doc
       comment and `PlacedTabs`' variants' doc comments saying "made active" instead of "the
       import's first shown folder"): with a tab, `Fill`; without a tab and the folder's first
       group, `OpenActive` with `make_active`, otherwise `OpenBehind`; without a tab and not the
       first group, `Closed`. Its test is changed to match.
     - `Fill` with `make_active`: after the fill, when the active tab is not one of the folder's
       tabs, its first tab is activated and loaded (`load_activated_tab`), as T260 does today for
       the first shown folder only.
     - After a group with `make_active` (opened active, or filled and activated), the active tab's
       position is kept with `ImportTabs::shown`, as `first_shown` is today, with `settling` =
       `!self.ratings_ready()`; a group without `make_active` keeps nothing.
     - **The position settles** (cold read): a tab made active is loaded with its ratings unread;
       under an active filter, the refilter when they arrive (`refilter_when_ratings_change`) can
       move its current image or leave the preview for the contact sheet without the user doing
       anything, which would make `unmoved` false. So, after the refilter of the active tab in
       `refilter_when_ratings_change`, when `import_tabs` is `Some`, `self.ratings_ready()`, and
       `settling_in` the active tab's folder is true, the position is kept again with
       `shown(import_position, false)`. A tab opened by `OpenActive` (no filter, contact sheet)
       is not moved by that refilter, but goes through the same rule.
     - So within one event two new folders in a row both become active in turn when the user has
       not moved; the last one stays active.
     - The doc comment of `import_placed` says this.
  3. **At `Done`:** `import_ended` and its call in `poll_import_worker` go (no end rule); `Done`
     still applies the dialog and sets `import_tabs` to `None`. The doc comment of
     `import_position` ("for the import's end rule") says what it is for now.
- **Tests to add:** point 1's tests in `import_tabs.rs`; `placed_tabs`' test changed. The event
  wiring is checked by hand (as T260).
- **Hand check (the owner, a release build, with a card or a folder of camera files over two
  dates):**
  1. An import into two new folders, touching nothing (the dialog may stay shown): the first
     folder's tab becomes active when its first image is shown; the second folder's tab becomes
     active when its first image is shown, not only at the end.
  2. The same with the dialog hidden and a click on another image of the first folder during the
     import, before the second folder's first image: the second folder's tab opens behind, and
     the first folder's tab is still active at the end.
  3. The second destination folder already open in a tab that is not active, touching nothing:
     that tab becomes active when the import's first image in it is shown, and its filter and
     marks stay.
- **Not in this task:** the import worker, the listings and the hold (T258–T260); no new user
  texts. At Done the reviewer changes the CHANGELOG line under `## [Unreleased]` ("at the end the
  newest folder's tab is made active if you have not moved") and the known-limits item "Another
  spelling of the destination is not recognised" (its last clause names the end of the import).
- **Rules:** `docs/tasks/README.md` rules 1–13 (no new texts); `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git status` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Open | Written after the owner's hand check of T260 (point 3) and the change of F-Q10 |
| 2026-11-21 | Open | Cold read: one gap fixed, the kept position taken before the activated tab's ratings are read (an active filter's refilter could then move its current image or view, and later folders would open behind): `shown(at, settling)`, `settling_in`, and the position kept again after `refilter_when_ratings_change` (point 2, reviewer's choice). Also: `now` taken per group before its fill; `import_position`'s doc named; the doc follow-ups that named nothing removed; the trailing-separator test kept |
| 2026-11-22 | Ready for review | Points 1–3 done. `import_tabs.rs`: `first_shown`/`first_kept`/`end_target` replaced by `shown(at, settling)`, `settling_in`, `position_kept`, `unmoved` (field `first` renamed `kept`); 3 tests replaced by 4. `main.rs`: `import_placed` with `make_active` (folder's first group and either the first shown folder or `unmoved`, `now` taken per group before the fill or open), `shown` after each group made active with `settling` = `!ratings_ready()`; the position kept again at the end of `refilter_when_ratings_change` while settling in the active tab's folder; `placed_tabs`' third parameter `make_active`, its test and doc comments changed (also `open_import_folder`'s doc on `active`); `import_ended` and its call removed. `docs/test-inventory.md`: counts and description. check.ps1 all OK (2250 passed, 76 ignored), no clippy warnings |
| 2026-11-22 | Done | Reviewer: points 1–3 as the brief says; `now` taken per group before its fill or open, `make_active` gated by the folder's first group, the settling position kept again once per tab (`load_tab` resets `ratings_read` at once, so a tab made active is always settling until its ratings are read). check.ps1 all OK (2250 passed, 76 ignored), no clippy warnings. Reviewer: CHANGELOG line (Changed) and the known-limits item's last clause changed (the end rule is gone), plan F4 row; awaiting the owner's hand check |
