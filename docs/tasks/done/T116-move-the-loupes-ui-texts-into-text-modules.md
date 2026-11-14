# T116. Move pw-loupe's remaining UI texts into text modules — Open

- **Why:** English becomes the interface's base language (`docs/GUI_REQUIREMENTS.md` GR14,
  owner 2026-11-13). T117 translates the texts, and it should only have to touch text modules.
  Today many texts are still written directly in drawing code (`main.rs`) or spread over logic
  files. This task **moves** them; it does **not** translate them: every text stays word for word
  as it is, in Swedish.
- **Crate:** `pw-loupe` only. **Files:** `crates/pw-loupe/src/main.rs`, `lib.rs`, `filter.rs`,
  `ratings.rs`, `import.rs`, `import_dialog.rs`, `import_worker.rs`, `metadata.rs`,
  `monitor_profile.rs`, `tabs.rs`, `thumbs.rs`, `shell_text.rs`, `import_text.rs`, `editor.rs`;
  new `crates/pw-loupe/src/loupe_text.rs` and `crates/pw-loupe/src/rating_text.rs`;
  `docs/test-inventory.md`.
- **What:**
  1. **Where texts live after this task.** Every text the user sees is made by a function in one
     of these text modules:
     - `shell_text.rs`: the main window, menus, tabs ("Tom"), the window title with the display
       profile (`monitor_profile.rs`), the settings window (today `SettingsText` and the encoding
       names in `main.rs`), the recovery message after an interrupted metadata write
       (`RECOVERY_TITLE`, the recovery text and "(säkerhetskopia: …)" in `main.rs`), the ExifTool
       messages at start ("ExifTool saknas", "ExifTool är för gammal"), and the general status
       messages in `main.rs` ("Kan inte öppna: …", "Inga bilder i …", "Ogiltig miniatyrdata",
       "Förhandsvisningsarbetaren avslutades", "Dra en mapp eller en bild hit", "Ingen bild vald",
       "Kan inte städa metadata: …", "Storlek", …);
     - `import_text.rs`: everything of the import: the dialog drawn in `main.rs` (Källa, Mål,
       Backup, Välj…, the folder-picker titles, Mappmall, Filnamnsmall, Jobb, Första nr, the
       placeholder "© 2026 Namn", the eject check box, Förhandsvisning, Sök igen, Förhandsvisar …,
       Dölj, Ny import, Stäng, Starta import, Importera, Importerar …, "Importen klar/avbruten: …",
       the queue messages "Importarbetarens kö är full." / "Importarbetaren har stoppat."), and
       the texts of `import.rs` (the mode names, the Clear confirmation, the template errors),
       `import_dialog.rs` (`STOPPED_MESSAGE`) and `import_worker.rs` (`NO_JOURNAL_TEXT`, the
       ExifTool version text, "Kunde inte läsa …", the eject-skipped text);
     - new `loupe_text.rs`: the loupe's texts of `lib.rs` (source line, zoom line,
       `PROFILE_UNUSABLE_TEXT`, the empty-window text, "förbereder …", "Inga bilder matchar
       filtret …", `READING_RATINGS_TEXT`, the RAW/JPEG error texts, "Läge: …", the counter
       "{number} / {total}") and the contact-sheet cell text "ingen förhandsvisning" (point 3);
     - new `rating_text.rs`: the texts of `filter.rs` (star and label filter names,
       `label_filter_text`) and `ratings.rs` (`UNSUPPORTED_TEXT`, the save errors, `label_name`,
       the "Betyg: …" texts);
     - `editor.rs`: the write-error texts of `metadata.rs` (`READ_ONLY_TEXT`, `LOCKED_TEXT`,
       `NO_JOURNAL_TEXT`, the ExifTool and safe-write texts, the queue texts, "Läser metadata…").
     `info_panel.rs`, `editor.rs`, `inspector_text.rs` and `preview_text.rs` already are text
     modules and keep their texts.
  2. **Moving a text:** a constant becomes a function in the text module (a `pub const` may stay a
     `const` there if it is used as one); a function moves as it is. The callers use the new
     place. Each text keeps exactly its present wording, placeholders and singular/plural
     handling. Drawing code (`main.rs` and the drawing parts of other files) then holds no
     user-visible string literal. Two `NO_JOURNAL_TEXT`s with the same text (`import_worker.rs`,
     `metadata.rs`) become one function.
  3. **No logic on a text:** `main.rs` decides how to draw a failed thumbnail by comparing the
     error text with `"ingen förhandsvisning"`, which `thumbs.rs` writes in three places. Give
     `CellState` its own variant for "no preview" (for example `CellState::NoPreview`), set it in
     those three places, and draw it with the text from `loupe_text.rs`. Nothing else may compare
     a user-visible text; if you find another comparison, stop and report it.
  4. Not user-visible, so they stay where they are: egui ids (`"metadata-window"`, …), settings
     keys and values (`settings.rs`), the colour-role names in `shell.rs`, environment-variable
     names, thread names, `eprintln!` lines starting with `pw-loupe:`, and the command-line help in
     `lib.rs` (English already).
- **Tests to add:**
  - unit tests that move with their functions move **unchanged** (only their module changes);
  - `CellState::NoPreview`: `thumbs.rs` sets it where it set `Failed("ingen förhandsvisning")`;
  - a test in `main.rs`'s tests (or `shell_text.rs`) that reads `include_str!("main.rs")` and fails
    if a non-comment line outside `#[cfg(test)]` passes a string literal straight to `label(`,
    `button(`, `Button::new(`, `checkbox(`, `on_hover_text(`, `hint_text(`, `selectable_label(`,
    `selectable_value(` or `pick_folder(`; its failure message names the line;
  - a test per new module that each moved text still gives today's Swedish text for one example
    (the strings of today's tests are enough where a test already exists).
- **Not in this task:** translating anything (T117); texts in other crates (T118); changing a
  layout, a key or behaviour; `info_panel.rs`/`editor.rs` drawing split.
- **Rules:** `docs/tasks/README.md` rules 1–13. Rule 13 already says English, but this task
  keeps every text in Swedish on purpose: T117 translates them all at once.
- **Done when:** `.\tools\check.ps1` is all OK, `git diff --stat` shows only the files above, and
  the program looks and behaves exactly as before (the owner checks the import dialog, the
  settings window and a contact sheet with a file without a preview by hand).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-13 | Open | Added (owner decision GR14 and the glossary, 2026-11-13) |
