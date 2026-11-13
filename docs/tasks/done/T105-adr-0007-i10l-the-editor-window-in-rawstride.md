# T105. ADR 0007 I10l: the editor window in RawStride, saved through the worker — Done

- **Can start after T103 and T104 are accepted.** Not at the same time as T106 (`main.rs`).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/metadata.rs`;
  `crates/pw-loupe/src/lib.rs` (`Action` and `key_action` only, and `EditorText`/`editor_text` in
  the `pub use editor::…` line); `crates/pw-loupe/src/editor.rs` (point 5 only: the window's
  texts; owner decision 2026-11-12, rule 13); `docs/test-inventory.md`.
- **Why:** I10 in the step plan: the editor, written with §9 through T97/T98.
- **What** (as D12 and D15):
  1. **Opening:** `Action::EditMetadata` on the key `E` (no Ctrl or Alt), and a button "Redigera"
     at the top of the info panel; only for one image whose kind can hold metadata
     (`supports_rating`), and only when its `MetadataRead` is loaded (otherwise the key does
     nothing). The window "Redigera metadata – <filnamn>" belongs to that image; moving to another
     image does not change it.
  2. **Contents:** the fields of `EditorForm` (a multi-line box where `multiline()`), the field
     errors under their fields, the "Olösta konflikter" section with a choice per conflict (radio
     buttons with origin and value), the warnings (in the warning colour), and the buttons
     "Spara" and "Stäng".
  3. **Saving:** "Spara" with field errors does nothing but show them; with no edits it does
     nothing; otherwise `write_edits` (T98). While it runs the form is read-only with "Sparar …".
     `EditsWritten`: `Ok(report)` → "Sparat." and the notice texts (T104), the panel's read is
     renewed (T103) and the form is filled again from the new read; `Err(e)` → "Kunde inte spara
     metadata för <fil>: <skäl>" with the reason as `write_error_text` gives it for ratings
     (`metadata.rs`), the form kept as typed.
  4. **Closing** with unsaved edits asks "Spara ändringarna?" with "Spara", "Släng" and "Avbryt".
     At program exit an editor write still running is waited for as rating writes are (T44's
     exit wait, the same limit), and listed if it does not finish.
  5. **The window's texts** (`editor.rs`; owner decision 2026-11-12: T105 needs these texts and
     rule 13 puts them in the editor's text module): `pub enum EditorText` and `pub fn
     editor_text(text: &EditorText) -> String`, as `PanelText`/`panel_text` (T102), each a whole
     sentence from one template:
     - `EditButton` "Redigera" (the button in the info panel);
     - `WindowTitle { file }` "Redigera metadata – <file>";
     - `ConflictsHeading` "Olösta konflikter";
     - `ConflictChoice { value, origin }` "<value> (<origin>)" (one radio button);
     - `WarningsHeading` "Varningar";
     - `Save` "Spara"; `Close` "Stäng"; `Saving` "Sparar …"; `Saved` "Sparat.";
     - `SaveFailed { file, reason }` "Kunde inte spara metadata för <file>: <reason>" (the reason
       as `write_error_text` gives it);
     - `UnsavedQuestion` "Spara ändringarna?"; `Discard` "Släng"; `Cancel` "Avbryt" (the
       question's "Spara" is `Save`).
     Nothing else in `editor.rs` changes: the form, its functions and its tests stay as T104 left
     them. `main.rs` draws only texts from `editor_text`, `EditorForm`/`EditorField` (labels, notes,
     field errors, warnings), `notice_text`, `info_panel.rs` and data (file names, values typed).
     A text that turns out to be missing is added to `EditorText` in the same way, and named in
     the hand-over.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Keys typed into the editor's text fields
  must not reach the rating and navigation keys (as the import dialog already prevents).
- **Tests to add:** `editor_text` of every `EditorText` once (`editor.rs`); `key_action` for `E`; `metadata.rs` with the fake submitter: an edits request
  sent with the form's edits, `EditsWritten` `Ok` asks for a new read of the image, `Err` keeps the
  pending state cleared and gives the Swedish text; the exit wait counts a running edits write.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: a Title and a
  keyword on a JPEG, a DNG shown in RAW mode and an ARW (sidecar), read back in the panel and in
  another program (Photo Mechanic or Lightroom); an emptied Description; a conflict chosen.
