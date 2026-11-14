# T117. Translate pw-loupe's UI texts to English — Open

- **Starts after T116 is Done** (the texts must be in the text modules first).
- **Why:** English is the interface's base language (`docs/GUI_REQUIREMENTS.md` GR14, owner
  2026-11-13). The terms are fixed in `docs/glossary.md` (approved 2026-11-13, G1–G8).
- **Crate:** `pw-loupe` only. **Files:** the text modules `crates/pw-loupe/src/shell_text.rs`,
  `import_text.rs`, `loupe_text.rs`, `rating_text.rs`, `inspector_text.rs`, `preview_text.rs`,
  `info_panel.rs`, `editor.rs`; `inspector.rs` (decimal comma, point 4); the unit tests in these
  files and in the files whose tests check a text (`main.rs`, `browse.rs`, `tabs.rs`, `import.rs`,
  `import_dialog.rs`, `import_worker.rs`, `metadata.rs`, `filter.rs`, `ratings.rs`); `docs/glossary.md`
  (only to add a missing term, point 6); `docs/test-inventory.md`.
- **What:**
  1. **Translate every text** in the text modules to English with the glossary's terms. Each text
     stays one whole sentence from one template (rule 13); reorder the placeholders as English
     needs. Singular/plural in English inside the same function ("1 file" / "2 files",
     "1 star" / "2 stars"). Keep the punctuation style of today's texts (ellipsis `…`, the
     separator `  ·  `, `–`).
  2. **Property names in English** (`info_panel.rs`, `property_label`; glossary G1 and G2):
     - every caller in `pw-loupe` that passes `Language::Sv` passes `Language::En`;
     - `Language::En` gives IPTC's name from the property table with these exceptions:
       " (legacy)" left out (Sublocation, City, Province or State, Country, Country Code), the
       contact fields `Iptc4xmpCore:CiEmailWork`, `CiTelWork`, `CiUrlWork` as **Email**, **Phone**,
       **Web URL**, `xmp:Rating` as **Rating**, `xmpRights:Marked` as **Copyrighted**, `xmp:Label` as
       **Label**, and the deprecated `photoshop:Urgency`, `photoshop:Category`,
       `photoshop:SupplementalCategories` as **Urgency**, **Category**, **Supplemental
       Categories**;
     - where the full IPTC name differs from the label (the " (legacy)" ones and the contact
       fields), the metadata window shows the full IPTC name as the field's tooltip;
     - `Language::Sv` and the Swedish table `SV_LABELS` stay as they are (a second language later).
  3. **Label colours:** the label names are the English names already stored (`Red`, `Yellow`,
     `Green`, `Blue`, `Purple`); `label_name` returns them unchanged.
  4. **Numbers:** decimals with a point, not a comma (`inspector.rs`: `decimal_comma` and the
     exposure-time `replace('.', ",")`; rename the function to say what it does); percentages
     without a space (`100%`); dates stay `YYYY-MM-DD`.
  5. **Messages that name an internal part** say what stopped for the user (glossary G6), for
     example "Importarbetaren har stoppat." → "Import has stopped.".
  6. **A term the glossary lacks:** add it to the right table of `docs/glossary.md` and name it
     in the report. Do not change a term that is already there; if one seems wrong, stop and ask.
- **Tests:** changing a test's expected text from today's Swedish to the English of this task is
  the point of the task and is allowed; nothing else in a test changes (no test removed or
  weakened). Add:
  - `property_label` with `Language::En`: one test per exception of point 2 and one plain IPTC
    name (for example `dc:title` → "Title"); `Language::Sv` unchanged (today's tests);
  - singular and plural for every text that has a number;
  - the decimal point in focal length, aperture and exposure time;
  - a test that the text modules' non-test code holds no Swedish letters (`å`, `ä`, `ö`, `Å`, `Ä`,
    `Ö`) in a string literal, except `SV_LABELS` in `info_panel.rs`; its failure message names the
    file and line.
- **Not in this task:** texts made in other crates (T118); a language setting (none for now);
  the ADRs, which keep the Swedish names they were decided with (the glossary maps them); the
  Tauri app.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Done when:** `.\tools\check.ps1` is all OK, `git diff --stat` shows only the files above, and
  the owner has checked by hand that the window, menus, import dialog, metadata window, settings
  and status lines are in English with no Swedish left (except messages from other crates, T118).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-13 | Open | Added; starts after T116 |
