# T112. ADR 0017 U12, first part: previous/next and "spara och nästa" in the metadata window — Done

- **Can start after T110 is Done** (both change `main.rs` and `metadata.rs`). The owner approved the
  proposals of U12-Q1–U12-Q5 (at the end of this task) on 2026-11-13; they are written into the
  points below. It does not depend on T111 and can come before or after it, but not at the same
  time as any other task that changes `main.rs`.
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/metadata.rs` (the window's read
  moved to another image, point 3); `crates/pw-loupe/src/editor.rs` (`EditorText`/`editor_text`
  only, point 8); `crates/pw-loupe/src/browse.rs` (the pure neighbour function of point 2);
  `crates/pw-loupe/src/lib.rs` (the window's key function of point 6 and its `pub use` line);
  `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.3.1 ("Not locked to the image", with the rules of review finding 7, which
  apply from stage A) and §7.1: the first part of U12 belongs to stage A. It replaces D16's lock
  (U-Q8, approved 2026-11-12). Everything else of U12 (recent values, copy/paste of all fields,
  "använd på alla markerade") and of U10, U11 and U13 is not in this task.
- **Not in this task:** more editable fields (U10, U11); "använd på alla markerade" and the batch
  rules (rest of U12); templates and variables (U13). `M` with several images marked still does
  nothing (T107). The window's fields, conflicts, warnings, read-only sections and the write are
  unchanged.
- **What:**
  1. **Buttons** at the bottom of the window, left of OK and Avbryt: "Föregående", "Nästa" and
     "Spara och nästa" (`EditorText`, point 8). They are disabled while the read or a write runs
     and when there is no image in that direction (point 2).
  2. **Which image** (§2.3.1, U12-Q3): previous/next follow the **shown images** of the active
     tab (the visible list after the filter, in the contact sheet's order) and **skip** image kinds
     without metadata support (`pw_metadata::supports_rating`, as `M` uses). No wrap: at the first or last such
     image the button in that direction is disabled and its key does nothing. A pure function in
     `browse.rs`, `metadata_neighbour(visible, current, forward, supported) -> Option<usize>`,
     decides it and is tested without a window.
  3. **Moving the window** to another image: the window stays open, at its position and size; its
     title shows the new file name; the form is emptied and `PanelText::Loading` shown while the
     new image's read runs (asked for **at once**, as when `M` opens it; one in flight; a result
     for the earlier image dropped, T107's rules). The window's read in `metadata.rs` gets a way to
     change its image without closing (no second read path).
  4. **The current image follows the window** (U12-Q2): after a move the new image is the current
     image of the tab, in the contact sheet (scrolled into view) and in the loupe or the preview
     (shown there, as a step does). **The marking is not changed** by a move, whatever view is
     behind the window (ADR 0017 §2.2, review finding 6). The window stays **modal**: the contact
     sheet, the loupe or preview, the tabs and the bars take no keys or clicks while it is open
     (D16, unchanged).
  5. **Unsaved edits** (§2.3.1, U12-Q4): moving (Föregående, Nästa), **Escape** and the window's
     **Avbryt** button with unsaved edits ask `EditorText::SaveQuestion` ("Spara ändringarna?")
     with three buttons:
     - **Spara:** the edits are written as OK writes them (field errors → shown, nothing written,
       no move); success without notices → the move or the close is done; **failure** → the
       window stays on the image with the edits kept and `EditorText::SaveFailed` shown (§2.3.1);
       **success with notices** → point 7;
     - **Släng:** the edits are dropped and the move or the close is done;
     - **Avbryt:** back to the window on the same image with the edits kept.

     Without unsaved edits a move happens at once and Escape and Avbryt close as today. **OK** is
     unchanged (writes and closes; with notices it stays as T107). "Spara och nästa" never asks:
     it writes the edits (when there are any) and moves on success; with field errors, a failure
     or notices it acts as Spara above. D16's `DiscardQuestion` and `KeepEditing` go away
     (point 8).
  6. **Keys** while the window is open (U12-Q1): **Ctrl+PageDown** Nästa, **Ctrl+PageUp**
     Föregående, **Ctrl+Enter** Spara och nästa. They work also while a field has the keyboard, so
     they are taken (`consume_key`) **before** the form is drawn and never reach a text field.
     Plain arrows, Page Up/Down, Home, End and Enter keep their meaning in the fields. A pure
     function in `lib.rs`, `editor_key_action(key, ctrl, shift, alt) -> Option<EditorKeyAction>`,
     maps them (Shift or Alt: `None`). While the Spara / Släng / Avbryt question is shown these
     keys do nothing; Escape there is Avbryt (back to the window).
  7. **After a write with notices** (§2.3.1, U12-Q5): the window stays on the image, refilled from
     the new read, showing `EditorText::Saved` and the notices (as T107). The move that was asked
     for is **not** done; the next press of a move (button or key) moves on, without a question
     when nothing has been edited since.
  8. **Texts** (`editor.rs`, rule 13): `EditorText` gains `Previous` "Föregående", `Next` "Nästa",
     `SaveAndNext` "Spara och nästa", `SaveQuestion` "Spara ändringarna?" and `Save` "Spara";
     `Discard` "Släng" and `Cancel` "Avbryt" are kept; `DiscardQuestion` and `KeepEditing` are
     removed when nothing uses them. The test of every text is updated.
- **Rules:** rules 3–7 and 10–13. No `Cargo.toml` change. The editor's write, the read after it,
  the exit wait of T105 and the rating path are unchanged. Unsaved edits are still dropped at
  program exit (T107).
- **Tests to add / change:** `metadata_neighbour` (forward and back; skipping unsupported kinds;
  none at the first and last supported image; a visible list with no supported image; the current
  image unsupported); `editor_key_action` (the three keys; nothing with Shift or Alt or without
  Ctrl; plain Page Up/Down and Enter give `None`); `metadata.rs` with the fake submitter: moving the
  window asks for the new image's read at once, a result for the earlier image is dropped, the
  write of the earlier image still reads that image back; the decision of point 5 and 7 as a pure
  function (edits or not × Spara / Släng / Avbryt × success, failure, notices, field errors → move,
  stay with edits, or stay showing notices) tested without a window; `editor_text` of the new texts
  (the removed ones out of the test).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride, on a folder
  with JPEG, ARW and a kind without metadata support: Nästa and Föregående (buttons and keys) move
  the window through the shown images and skip the unsupported one, and the contact sheet's
  current image follows while the marking stays; under a star filter only the shown images are
  visited; a Title typed and Ctrl+Enter writes it and moves on (read back with `M` and in Photo
  Mechanic); an edit and Nästa asks Spara / Släng / Avbryt and each does what it says; Escape and
  Avbryt with edits ask the same; a write refused while another program holds the file keeps the
  window on the image with the text; the same from the loupe.
- **Questions (answered by the owner, 2026-11-13: every proposal approved):**
  - **U12-Q1** The keys (point 6): Ctrl+PageDown next, Ctrl+PageUp previous, Ctrl+Enter "spara och
    nästa"? They work while typing in a field; plain arrows and Enter stay with the fields.
  - **U12-Q2** The current image follows the window (point 4): after a move the contact sheet's
    current image (and the loupe or preview) is the window's image, and the marking is not
    changed?
  - **U12-Q3** The ends (point 2): no wrap; the button and key are disabled at the first and last
    image that has metadata support?
  - **U12-Q4** The question (point 5): "Spara ändringarna?" with Spara / Släng / Avbryt for a move,
    for Escape and for the window's Avbryt button; OK unchanged; "Spara och nästa" never asks; D16's
    "Släng ändringarna?" goes away?
  - **U12-Q5** After a write with notices (point 7): the window stays and shows them, and the move
    happens only on the next press of a move button or key? (The ADR says "moves on only on the
    next key press"; this reads it as the next press of a move, not any key.)
