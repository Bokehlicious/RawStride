# T107. ADR 0007 I10n: one metadata window locked to its image (D16) — Done

- **Can start now** (T103, T104, T105 are committed). Not at the same time as T106 (`main.rs`); the
  reviewer decides the order. It replaces the right-hand panel of T103 and the editor window of
  T105 by one window; their logic is kept where it can be (the read in flight, the write and the
  read after it, the exit wait, the texts).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/metadata.rs`;
  `crates/pw-loupe/src/lib.rs` (`Action`, `key_action` and the `pub use` lines of `editor` and
  `info_panel` only); `crates/pw-loupe/src/editor.rs` (`EditorText`/`editor_text` only);
  `crates/pw-loupe/src/info_panel.rs` (point 4 only); `docs/test-inventory.md`.
- **Why:** owner decision D16 (2026-11-12): the information and the editing in one floating window,
  as the user knows it from other photo programs, locked to its image until OK or Avbryt.
- **What:**
  1. **Keys** (`lib.rs`): `Action::ToggleMetadata` becomes `Action::OpenMetadata` on `M` (no Ctrl
     or Alt); `Action::EditMetadata` and the key `E` are removed (`E` does nothing).
  2. **Opening** (`main.rs`): `M` in the loupe or the contact sheet opens the window for the
     current image (in the contact sheet: the current image when at most one is marked; with
     several marked, or a kind without metadata support (`supports_rating`), `M` does nothing).
     The read is asked for **at once** (no 200 ms wait: the window is opened on purpose); until it
     is back the window shows `PanelText::Loading` and only "Avbryt"; a read error shows
     `PanelText::ReadError` and only "Avbryt". The right-hand panel, `show_metadata`, the
     "Redigera" button and opening from `E` are removed.
  3. **The window** (`egui::Window`, title `EditorText::WindowTitle`, movable, resizable, default
     about 460 × 640, its own vertical scroll area, the buttons outside it at the bottom):
     - **modal**: while it is open the loupe, the contact sheet, the tabs and the top bar take no
       keys or clicks (as the import dialog keeps keys away today), so the image cannot change;
       keys typed into its fields reach nothing else;
     - top to bottom: the 22 fields of `EditorForm` (multi-line where `multiline()`, the Lang Alt
       note, field errors under their field); "Olösta konflikter" with radio buttons
       (`EditorText::ConflictChoice`); "Varningar" in the warning colour; then read-only, in small
       text: `EditorText::OtherValuesHeading` with point 4's other values (label, value, origin in
       grey), `PanelText::Conflicts` with the conflicts a rule has decided (rule text in the
       warning colour), `PanelText::Notices` with the notices; each read-only section only when it
       has rows;
     - **OK** (`EditorText::Ok`): field errors → shown, nothing written; no edits → the window
       closes; otherwise `write_edits` (T98), the window read-only with `EditorText::Saving` and
       "Avbryt" disabled; success without notices → the window closes; success with notices → the
       form is refilled from the new read and the window stays showing `EditorText::Saved` and the
       notice texts until OK or Avbryt; failure → `EditorText::SaveFailed`, the typed text kept;
     - **Avbryt** (`EditorText::Cancel`) closes without writing; **Escape** is Avbryt, but with
       edits it first asks `EditorText::DiscardQuestion` with `EditorText::Discard` and
       `EditorText::KeepEditing`; Enter does not press OK;
     - unsaved edits are dropped at program exit; a write already running is waited for as T105
       does (the exit wait and its message are unchanged).
  4. **`info_panel.rs`:** `ConflictRow` gains `pub open: bool` (the rule is `Open`), and `pub fn
     other_values(read: &MetadataRead, editor_properties: &[&str]) -> Vec<ValueRow>`: the `values`
     of `info_panel` without the properties in `editor_properties`; `main.rs` passes the
     properties of `EditorField::ALL` except `Iptc4xmpCore:CreatorContactInfo` (its address fields
     are not editor fields, so the structure stays in the list). Nothing else changes there.
  5. **`metadata.rs`:** the read of the window's image: one read in flight, a result for another
     image dropped, read again after `RatingWritten`/`EditsWritten` of the image; asked for at
     once when the window opens, not while moving between images (the 200 ms wait and the reads
     while the panel was shown go away). The editor's write, the read after it and the exit wait
     of T105 stay.
  6. **`editor.rs` texts:** `EditorText` gains `Ok` "OK", `OtherValuesHeading` "Övrig metadata",
     `DiscardQuestion` "Släng ändringarna?", `KeepEditing` "Fortsätt redigera"; `WindowTitle`
     becomes "Metadata – <file>"; `EditButton`, `Close`, `Save` and `UnsavedQuestion` are removed
     when nothing uses them. `rule_text(Open)` is not shown in the window (open conflicts are the
     choices). A text that turns out to be missing is added to `EditorText` the same way and named
     in the hand-over.
- **Rules:** rules 3–7 and 10–13. No `Cargo.toml` change. The rating path and the exit wait are
  unchanged.
- **Tests to add / change:** `key_action` for `M` (`OpenMetadata`, not with Ctrl or Alt) and `E`
  (`None`); `editor_text` of the new and changed texts (the test of every text updated);
  `other_values` (an editor property left out, `CreatorContactInfo` kept, the order of the table)
  and `ConflictRow::open`; `metadata.rs` with the fake submitter: a read sent at once when the
  window opens, one in flight, a stale result dropped, a new read after a write of the image. The
  T103 tests of the 200 ms wait and of reads while hidden are removed or rewritten for the window
  (say which in the hand-over).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: `M` on a
  JPEG, a DNG shown in RAW mode and an ARW with a sidecar shows their metadata; while the window is
  open the image cannot be changed; a Title and a keyword written with OK, read back with `M` and
  in another program (Photo Mechanic or Lightroom); an emptied Description; a conflict chosen in
  `D:\PhotoWorkflowData\Konflikttest\konflikt-stad.jpg`; Avbryt drops the edits; Escape with
  edits asks first.
