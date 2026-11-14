# T125. ADR 0017 U12b: Copy and paste all fields between images — Done

- **Can start now.** The owner approved the proposals of U12b-Q1–U12b-Q5 (at the end of this
  task) on 2026-11-14; they are written into the points below and into ADR 0017 §6.1. Not at the
  same time as another task that changes `main.rs`.
- **Crates:** `pw-loupe` only. **Files:** `crates/pw-loupe/src/editor.rs` (the copied fields,
  copy and paste on `EditorForm`, the texts); `crates/pw-loupe/src/lib.rs` (the window's keys,
  point 4); `crates/pw-loupe/src/main.rs` (the copied fields kept in `Loupe`, the buttons, the
  keys); `docs/glossary.md` (the new terms); `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.3.1, "Speed while captioning": "kopiera alla fält" and "klistra in alla
  fält" between images. With T112's "Save and Next" a series is captioned as: fill in the first
  image, copy, then on each next image paste, change what differs, Ctrl+Enter. U12c ("Apply to all
  marked") comes after this task and uses the same copied fields.
- **What:**
  1. **Copy** (U12b-Q1): "Copy All Fields" takes the form's texts **as the window shows them**,
     unsaved edits included, of every field that can be edited: the fields of sections 1–4 that
     `EditorField::editable()` allows and that are not shown read-only (`read_only_text`), and
     the Copyrighted check box. Not copied: the read-only fields, the Rating and Label section (it
     has its own path, U10c-Q3), the conflict choices, and a field's error. A Lang Alt field gives
     its `x-default` text (the only language the window edits, ADR 0007 Q7). The copy is a value
     of its own (for example `CopiedFields`: the source file's name and one text per field), kept
     in `Loupe`, not in the window, so it outlives closing the window and moving it; it is kept
     until the next copy or the end of the program, and is not saved in `settings.txt` and not put
     on the Windows clipboard.
  2. **Paste** (U12b-Q2): "Paste All Fields" puts the copied texts into the form of the window's
     image as **unsaved edits**, exactly as if the user had typed them; nothing is written until
     OK, Save or "Save and Next". Only fields whose copied text is **not empty** are pasted; the
     other fields keep what the form has. A field shown read-only on this image is left as it is.
     A pasted text equal to the field's start text gives no edit (`EditorForm::edits` as today),
     and a pasted text that is not valid for the field (a date) shows the field's error as typing
     it would. A pasted value on a property with an open conflict decides the conflict as typing
     does (T83, Q1). A list field (Keywords, Creator, …) is pasted as its shown text and split at
     commas when it differs (T108).
  3. **Buttons** (U12b-Q3): "Copy All Fields" and "Paste All Fields" in the window's button row,
     before "Previous". Copy is enabled when the form is shown; Paste when something has been
     copied and the form is enabled (not while the form's write runs, not while "Save changes?"
     is asked). The Paste button's tooltip names what it holds: "{n} fields from {file}" (one
     function, singular "1 field").
  4. **Keys** (U12b-Q4): **Ctrl+Shift+C** copies and **Ctrl+Shift+V** pastes while the window is
     open, also while a field has the keyboard; plain Ctrl+C and Ctrl+V stay with the text fields
     (egui gives Copy and Paste only for Ctrl without Shift). They are taken before the form is
     drawn, as T112's keys, through `editor_key_action` (two new `EditorKeyAction`s) and do what
     the button would (nothing when it is disabled).
  5. **Texts** (U12b-Q5): "Copy All Fields", "Paste All Fields" and the tooltip in `editor_text`
     (rule 13), and in `docs/glossary.md` with the Swedish names of ADR 0017 ("Kopiera alla fält",
     "Klistra in alla fält").
- **Not in this task:** pasting on several marked images or from the contact sheet (U12c, with the
  batch rules of §2.3.1); templates (U13); recent values (U12a); the lists of structures (U11a);
  any change in `pw-metadata` or the write path.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add:** in `editor.rs`, on hand-built `MetadataRead` values: copy takes every editable
  field's shown text (an unsaved edit included), the check box, a Lang Alt's `x-default` text, and
  no read-only field, deprecated field or rating; paste of a copy with empty fields leaves those
  fields as they were; a pasted text equal to the start text gives no edit and a different one a
  `Set`; a pasted bad date gives the field's error; a field shown read-only on the target is not
  changed; a pasted list split at commas; a pasted value on a property with an open conflict
  deciding it; copy → paste on the same form gives no edits. In `lib.rs`: Ctrl+Shift+C and
  Ctrl+Shift+V give the two actions; Ctrl+C, Ctrl+V, Shift+C, Alt or Ctrl+Alt with C or V give
  none; T112's keys unchanged. The texts of point 5, the tooltip for 1 and 12 fields.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated and
  `git diff --stat` shows only the files above. The owner checks by hand on a JPEG and an ARW:
  copy on one image, Next, paste: the fields appear unsaved, Next asks "Save changes?"; "Save and
  Next" with a paste writes the fields (seen in Photo Mechanic); an empty field in the copy does
  not empty the target's field; the copy is still there after closing and opening the window;
  Ctrl+C / Ctrl+V still copy and paste text inside a field.

## Questions (answered by the owner, 2026-11-14: every proposal approved)

- **U12b-Q1 What is copied.** The form as shown (unsaved edits included), every editable field of
  sections 1–4 and Copyrighted; not the read-only fields, the rating and label, or the conflict
  choices. The copy lives in memory until the next copy or the end of the program (not the Windows
  clipboard, not saved). *Alternative:* copy the saved values of the image only.
- **U12b-Q2 What paste does.** It fills the form as unsaved edits (nothing written until OK, Save
  or "Save and Next"), and **only non-empty copied fields** are pasted, so an empty field in the
  copy never empties the target (the same as §2.3.1's "fields not touched are left as they are",
  and no copyright field is emptied by a paste, ADR 0007 §8). *Alternative:* paste every field,
  so empty copied fields empty the target (except the copyright fields).
- **U12b-Q3 Buttons.** "Copy All Fields" and "Paste All Fields" in the button row before
  "Previous"; the Paste button's tooltip says how many fields and from which file.
- **U12b-Q4 Keys.** Ctrl+Shift+C and Ctrl+Shift+V while the window is open, also while typing in a
  field; Ctrl+C and Ctrl+V stay with the fields.
- **U12b-Q5 Names.** English "Copy All Fields" / "Paste All Fields" (Swedish "Kopiera alla fält" /
  "Klistra in alla fält" as in ADR 0017), added to the glossary.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Blocked | Written by the reviewer (ADR 0017 U12b, after T124); waits for U12b-Q1–U12b-Q5 |
| 2026-11-14 | Open | Owner: U12b-Q1–U12b-Q5 approved as proposed; written into ADR 0017 §6.1 |
| 2026-11-14 | Ready for review | Task agent: copy and paste on `EditorForm` (`CopiedFields`), the buttons, Ctrl+Shift+C/V; 11 new tests; `check.ps1` all OK (1259 passed, 43 ignored) |
| 2026-11-14 | Ready for review | Reviewer: code accepted as written, no changes. Copy takes every editable field as shown (unsaved edits included) and Copyrighted, no read-only field, rating or label; paste only non-empty fields, read-only fields on the target left, edits, errors, lists and conflicts as typing; the copy kept in `Loupe`; buttons before "Previous", Paste disabled while writing or asking, its tooltip also when disabled; Ctrl+Shift+C/V through `editor_key_action`, Ctrl+C/V left to the fields. `test-output.txt` newer than the code: fmt, test and clippy OK, 1259 passed, 0 failed, 43 ignored |
| 2026-11-14 | Done | Owner: hand check done and approved; committed by the reviewer |
