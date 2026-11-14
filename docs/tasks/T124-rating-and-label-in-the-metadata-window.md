# T124. ADR 0017 U10c: Rating and Label in the metadata window — Open

- **Can start now.** The owner approved the proposals of U10c-Q1–U10c-Q5 (at the end of this
  task) on 2026-11-14; they are written into the points below and into ADR 0017 §6.1. Not at the
  same time as another task that changes `main.rs`.
- **Crates:** `pw-loupe` only. **Files:** `crates/pw-loupe/src/main.rs` (the window's section, its
  keys, the deferred filter, `window_editor_properties`); `crates/pw-loupe/src/editor.rs`
  (`EditorSection::RatingLabel` and its heading); `crates/pw-loupe/src/lib.rs` (the window's key
  function, point 3); `crates/pw-loupe/src/settings.rs` (the section's state);
  `crates/pw-loupe/src/browse.rs` (only if point 5 needs a pure helper); `docs/glossary.md` if a
  new term is needed; `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.3.1 lists "Rating and Label" as the form's sixth section. T123 left it out
  (U10-Q5): the rating and the label are written by their own path (the rating store in
  `ratings.rs` and `metadata.rs`, the worker's `write_rating`), which keeps the badges, the filter
  and the queue right. This task shows them in the window and sets them through that same path,
  never through the form's edits.
- **What:**
  1. **The section** (U10c-Q5) `EditorSection::RatingLabel`, heading **Rating and Label** (U10-Q1), drawn
     last, after Assignment. Collapsible and remembered in `settings.txt` as the other sections
     (key `editor_rating_label`, open at first).
  2. **The controls** (U10c-Q1), for the window's image only (not the marked images):
     - stars: six buttons, "no stars" and 1–5, plus **Reject**; the button of the current value
       is shown selected;
     - label: Red, Yellow, Green, Blue, Purple and "no label"; the current one shown selected.

     Each button gives the `RatingKey` its key gives (`Stars(n)`, `Reject`, `Label(..)`,
     `ClearLabel`) and is applied with `RatingStore::set` through the same call the keys use
     (`metadata.set_ratings` with the one path), so `apply`'s rules hold: a second press of the
     current label or of Reject clears it. The shown value is the store's (`shown_rating`), so a
     queued or rolled-back write is seen at once, as on the badges. The names come from
     `rating_text.rs` and the glossary (§2), not new literals in drawing code (rule 13).
  3. **Keys** while the window is open (U10c-Q2): **Alt+0–5** stars and **Ctrl+0–5** labels, as in
     the contact sheet. They are taken before the form is drawn (as T112's Ctrl+PageDown) and work
     while a field has the keyboard. Plain digits, 6–9 and `X` stay with the text fields. A pure
     function in `lib.rs` maps them (extend `editor_key_action` or add `editor_rating_key`; Shift
     or Ctrl+Alt: nothing). While the Save / Discard / Cancel question is shown they do nothing.
  4. **Written at once** (U10c-Q3): a rating or label is queued the moment it is chosen, exactly as
     a key in the contact sheet. It is not part of the form's edits: **OK**, **Cancel**, **Discard**
     and "Save and Next" do not write or undo it, and "Save changes?" is not asked for it alone. It
     is refused, with `UNSUPPORTED_TEXT` or the path error as today, where the keys refuse it.
  5. **The filter while the window is open** (U10c-Q4): a rating chosen in the window does **not**
     refilter the tab at once (the window's image would leave the visible list and Previous / Next
     would have nothing to start from). The tab is refiltered when the window **moves** (the
     neighbour is found first in the list as it was, then the tab refiltered keeping that
     neighbour) or **closes** (refiltered as after a key in the contact sheet). Test the decision
     as a pure function.
  6. **Other Metadata** no longer lists `xmp:Rating` and `xmp:Label`. Conflicts on them are still
     listed and chosen as today.
- **Not in this task:** rating several images from the window (U12c); the reject move (U9); any
  change in `ratings.rs`' rules, `pw-metadata` or the write path; the PLUS fields (U10d).
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add / change:** `EditorSection::ALL` with `RatingLabel` last and its heading; the
  section's settings key (round trip, missing → open, bad value → problem); the key function
  (Alt+0–5, Ctrl+0–5; nothing with Shift, Ctrl+Alt, plain digits, 6–9 or `X`); the button →
  `RatingKey` mapping; the deferred-filter decision of point 5 (move: neighbour from the old list,
  then refilter keeping it; close: refilter; nothing refiltered while the window stays);
  `window_editor_properties` with `xmp:Rating` and `xmp:Label`.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated and
  `git diff --stat` shows only the files above. The owner checks by hand on a JPEG and an ARW: the
  section shows the badge's value; a star, Reject and a label set from the buttons and from
  Alt/Ctrl+digits appear on the badge behind the window and in Photo Mechanic; a second press of
  the label clears it; Cancel after a rating keeps the rating; under a star filter, rating the
  window's image out of the filter keeps the window on it, Next still moves on, and after closing
  the image is gone from the contact sheet.

## Questions (answered by the owner, 2026-11-14: every proposal approved)

- **U10c-Q1 Controls.** Buttons for no stars, 1–5 and Reject, and for the five labels and no label,
  each acting exactly as its key (a second press of the current label or Reject clears it)?
- **U10c-Q2 Keys.** Alt+0–5 for stars and Ctrl+0–5 for labels while the window is open, working
  also while typing in a field; plain digits, 6–9 and X stay with the text fields?
- **U10c-Q3 Written at once.** A rating or label from the window is written at once, as a key in
  the contact sheet, and is not undone by Cancel or Discard? (The alternative, held until OK,
  would mix the rating path with the form's write.)
- **U10c-Q4 Filter.** While the window is open its image stays even if a new rating puts it
  outside the filter; the contact sheet is refiltered when the window moves or closes?
- **U10c-Q5 Place.** The section last in the form, open at first, its state remembered?

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Blocked | Written by the reviewer (ADR 0017 U10c, after T123); waits for U10c-Q1–U10c-Q5 |
| 2026-11-14 | Open | Owner: U10c-Q1–U10c-Q5 approved as proposed; written into the points and ADR 0017 §6.1 |
