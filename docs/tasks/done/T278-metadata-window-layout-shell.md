# T278. The metadata window drawn by the layout, and the Settings window's pages — Done

- **Why:** ADR 0017 §2.3.1 and step U10b (§6.3, U10b-Q1–U10b-Q9, the owner's answers
  2026-11-22): the user chooses which sections and fields of the metadata window are shown, and in
  which order. T277 made the layout (`EditorItem`, `EditorLayout`, `Settings::editor_layout`, the
  lines in `settings.txt`, `copy_shown` and `paste_shown`). This task draws the window by it and
  gives the Settings window the pages General, External Programs and Metadata Window (U10b-Q6).
  The owner's hand check follows.
- **Crates:** `pw-loupe` (the shell). **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/shell_text.rs`,
  `crates/pw-loupe/src/lib.rs` (`SettingsPage` re-exported beside the other `shell` types),
  `docs/glossary.md`,
  `docs/test-inventory.md`.
- **Background (read before starting):**
  - `editor_layout.rs` (T277): `EditorItem` (`Field`, `ContactInfo`, `List`; `fields()`,
    `label()`), `EditorLayout` (`sections()`, `items()`, `shown_sections()`, `shown_items()`,
    `is_shown()`, `set_section_shown`, `set_item_shown`, `move_section_up`, `move_item_up`,
    `shown_properties()`, `Default`).
  - `main.rs`: `draw_window_form` (about lines 8060–8120) loops over `EditorSection::ALL` and,
    per section, `draw_structure_lists(ui, form, section, None)` and each field with
    `draw_window_field` followed by `draw_structure_lists(.., Some(field))`;
    `draw_structure_lists` (about line 8477) draws each list of `lists_at`/`lists_after_list`;
    `draw_window_field` draws the heading "Creator's Contact Info" before the first contact field;
    `other_values(&state.read, &window_editor_properties())` gives Other Metadata;
    `state.form.copy(..)` and `state.form.paste(..)` (about lines 4066–4072).
  - `main.rs`: the Settings window, `draw_settings_window` (about lines 1065–1190),
    `open_settings_window`, `save_settings_window` (keeps every top-level field of `chosen`, so
    `editor_layout` is saved and applied by Save without a change there); `Command::Settings` and
    `Command::OpenInAnother` (about line 6497: "The Settings window has no scrolling, so its
    section External Programs is in view when it opens", and Ctrl+E with an empty program list,
    about line 7399). The metadata window and the Settings window are never open together
    (`open_settings_window` checks it), so the layout never changes under an open form.
  - `shell_text.rs`: `SettingsText` and `settings_text`.
- **What:**
  1. **The form by the layout.** `draw_window_form` gets the layout (`&EditorLayout`, from
     `self.settings.editor_layout`) and draws, for each section of `layout.shown_sections()`, in
     that order, its collapsible heading as today (open state, enabled state and Rating and
     Label's buttons as today), and inside it each item of `layout.shown_items(section)` in that
     order:
     - `Field(field)`: `draw_window_field` as today;
     - `ContactInfo`: `draw_window_field` for each field of `item.fields()` in order (the heading
       "Creator's Contact Info" comes from `draw_window_field`'s own rule, so it is drawn once,
       before the first; if that rule no longer fits, move the heading here, drawn once);
     - `List(list)`: the list drawn as `draw_structure_lists` draws one list today (its heading,
       entries, Add, Remove, the copy buttons, the read-only text). Split the body of
       `draw_structure_lists` into a function for one list, and remove `draw_structure_lists`
       when nothing calls it any more.
     A hidden section is not drawn at all; a hidden item is not drawn. The conflicts, warnings,
     Other Metadata, the decided conflicts and the notices stay after the sections as today
     (U10b-Q2), with every open conflict still listed (U10b-Q4). The layout is cloned before the
     window's closures, as `shell` is (about line 3779), so `self` is not borrowed inside them.
     The doc comment of `draw_window_form` (about lines 8052–8065), which names the lists' fixed
     places, is rewritten to say the order comes from the layout.
  2. **Other Metadata shows what is hidden (U10b-Q3).** The form's Other Metadata comes from
     `other_values(&state.read, &layout.shown_properties())`. `window_editor_properties()` is
     then used only by the existing tests (about lines 11506–11571, 11625 and 11641): mark it
     `#[cfg(test)]` and let it return `EditorLayout::default().shown_properties()`, and reword
     its doc comment (about lines 8019–8024); those tests do not change and must still pass. They
     check `contains` and the rows, and the one about line 11506 also checks
     `properties.len() == 55` and that every entry is unique (39 fields, the contact info, 13
     lists, `xmp:Rating`, `xmp:Label`); T277's `shown_properties` lists each property once, so it
     holds. If it does not, stop and report. The only non-test caller (about line 8157) takes the
     layout.
     **Rating and Label hidden (U10b-Q9, owner 2026-11-22):** the rating keys still rate in the
     window as today, and `shown_properties` always holds `xmp:Rating` and `xmp:Label` (T277), so
     the rating read when the window opened is never shown. Instead, while that section is not
     shown, Other Metadata starts with one line, `status_text(rating)` (`rating_text.rs`, for
     example "Rating: 3 stars, Red"; "Rating: Unrated" without a rating), from the window's
     current shown `rating`, which `draw_window_form` already gets; so a key press is seen at
     once. Other Metadata is drawn (its heading included) whenever it has rows or this line. No
     new text: `status_text` exists; `rating_text.rs` is not changed.
  3. **Copy and paste (U10b-Q4).** "Copy All Fields" calls `copy_shown(&window.file, layout)` and
     "Paste All Fields" `paste_shown(copied, layout)`, with the same layout. (A known small
     limit, accepted: when the layout is changed between a Copy and a Paste, the Paste button's
     tooltip still counts the items copied that are now hidden, while the paste skips them; the
     reviewer notes it in `known-limits.md` at Done.)
  4. **The Settings window's pages (U10b-Q6).** A new plain enum in `shell.rs`,
     `SettingsPage { General, ExternalPrograms, MetadataWindow }` with `ALL` in that order, and
     its names in `shell_text.rs` through `settings_text`: a new variant
     `SettingsText::Page(SettingsPage)` gives "General", "External Programs" and "Metadata
     Window", and `SettingsText::RestoreDefaults` "Restore Defaults" (`SettingsText` stays
     `Copy`; `shell_text.rs` already imports from `crate::shell`). The window draws the page
     buttons in one row at the top (`selectable_label`, the chosen one selected), then the chosen
     page:
     - **General:** the IIM check box and its note, the encoding and the theme (its label from
       `shell_text(ShellText::ThemeLabel)` as today), as today;
     - **External Programs:** the program list, Add…, Move Up, Remove, as today, without the
       heading "External Programs" (the selected page button names it);
     - **Metadata Window:** in a vertical scroll area with a fixed greatest height (a named
       constant; the window does not grow past the screen), for each section of
       `chosen.editor_layout.sections()`: a row with a check box named by the section's heading
       (`editor_text(&EditorText::SectionHeading(section))`) and Move Up (greyed on the first
       section); under it, indented, each item of `items(section)`: a check box named by
       `item.label()` and Move Up (greyed on the section's first item). The items of an
       unchecked section are drawn greyed (their own checks kept). Under the scroll area,
       "Restore Defaults" sets `chosen.editor_layout` to `EditorLayout::default()` (not the
       sections' open or collapsed state, the `editor_*` keys, which stay as they are).
       `SettingsPage` derives `Debug` (the workspace warns on `missing_debug_implementations`). Like every
       choice in the window, nothing applies before Save, and Cancel drops it. The clicks are
       applied after the rows are drawn, as the program rows' are.
     Save and Cancel stay at the bottom under every page. The chosen page is kept while the window
     is open (a field beside `settings_window`, for example `settings_page: SettingsPage`), not
     saved: `Command::Settings` opens the window on General, `Command::OpenInAnother` (also Ctrl+E
     with no programs) on External Programs, so the comment about line 6497 is rewritten to say
     why. The page is set only when the window actually opens (`open_settings_window` gets the
     page; it still does nothing while the window or the metadata window is open, so a window
     already open keeps its page). The window's width stays `SETTINGS_WINDOW_WIDTH`; a long name
     wraps: a check box beside its Move Up in a plain `ui.horizontal` does not wrap in egui, so
     use a layout that does (for example Move Up first and the check box after it in
     `horizontal_wrapped`, or the check box with a wrapping label).
  5. **Texts and glossary** (`docs/tasks/README.md` rule 13): every new text from
     `settings_text`; no string literal in drawing code. `docs/glossary.md` §6 gets rows for
     General, Metadata Window (the Settings pages) and Restore Defaults, and the existing rows of
     External Programs and Move Up name their use on the pages too (each "added by T278").
- **Tests to add** (in the binary's test module or `shell.rs`/`shell_text.rs`'s, as fits):
  `SettingsPage::ALL` is the three pages in order and each has a non-empty, distinct name
  (`settings_text(SettingsText::Page(page))`; rows may be added to the table test about
  `shell_text.rs:1023`, without changing its rows); the
  new `settings_text` entries are non-empty; a test that the form's Other Metadata for a layout
  with Title hidden holds Title's row when the image has a title (with `other_values` and
  `layout.shown_properties()` on a read with `dc:title`, as the existing tests about line 11625
  build their read), and does not with the default layout; a test that a hidden section's
  field (for example Instructions in a hidden Assignment) is in Other Metadata the same way; a
  test that with Rating and Label hidden the rows of `other_values` with
  `layout.shown_properties()` on a read with `xmp:Rating` hold no rating row (U10b-Q9; the line
  itself is checked by hand).
  The drawing itself is checked in the owner's hand check.
- **Hand check (the owner, release build):**
  1. Settings opens on General with today's choices; External Programs and Metadata Window are
     pages; Ctrl+E with no programs opens External Programs.
  2. On Metadata Window: uncheck Event and Organisation Name, move Keywords to the top of Content,
     move Licence (PLUS) above Creator and Rights, uncheck Assignment; Save. The metadata window
     shows that layout; on an image with an Event and Instructions their values are under Other
     Metadata, read-only.
  3. "Creator's Contact Info" and a list (Location Shown) hidden and moved work the same way.
     With Location Created hidden and Location Shown shown, "Copy from Location Created" copies
     the file's Location Created (seen under Other Metadata).
  4. Copy All Fields on an image, Paste All Fields on another: Event (hidden) is not pasted; the
     tooltip's count leaves it out.
  5. Rating and Label hidden: the metadata window's Other Metadata starts with "Rating: …"; a
     rating key (for example Alt+3 or `X`) changes that line at once, and after closing the
     window the contact sheet shows the new rating.
  6. After a restart the layout is the same; `settings.txt` has the four lines of T277.
  7. Restore Defaults, Save: today's form again, and the four lines are gone from `settings.txt`.
  8. Cancel after changes keeps the old layout. A long section or item name wraps; the page
     scrolls on a small window.
- **Not in this task:** a choice for the inspector's panels (U10b-Q1); the companion suffixes'
  page (U8, U10b-Q8); drag and drop; any change of `editor_layout.rs`, `editor.rs` or
  `settings.rs` (if one is needed, stop and report).
- **Rules:** `AGENTS.md` §3; no new dependency; no existing test changed (point 2 moves
  `window_editor_properties` under `#[cfg(test)]`, its tests are left as they are);
  `docs/tasks/README.md` rule 13.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Added. Waits for T277. ADR 0017 §6.3 (U10b-Q1–U10b-Q9). Second and last of the chain T277 → T278 (step U10b, order 2b); the owner's hand check after it. At Done: CHANGELOG (Added), README's Planned item, plan rows |
| 2026-11-22 | Blocked | Cold read: doable once T277 is Done; nothing in the code contradicts it. Changed: `lib.rs` re-exports `SettingsPage` (required); the third test using `window_editor_properties` (about line 11506, 55 unique entries) named; the page names through `SettingsText::Page(SettingsPage)` and `RestoreDefaults`; the External Programs heading dropped on its page; the page set only when the window opens; how a long name wraps beside Move Up; the layout cloned before the closures; the doc comments to rewrite; the tooltip count after a layout change as an accepted limit. Chain review: Restore Defaults leaves the sections' open state; `Debug` on `SettingsPage`; hand check point on "Copy from Location Created" with the source hidden; U10b-Q9 (owner, proposal A): with Rating and Label hidden Other Metadata starts with `status_text` of the current rating, with a test and hand check point 5. For `known-limits.md` at Done: the tooltip count (point 3), and an older RawStride reading `settings.txt` gives a problem per layout line and drops them at its next save |
| 2026-11-22 | Open | T277 Done: `EditorItem`, `EditorLayout`, `Settings::editor_layout`, `copy_shown` and `paste_shown` are in place as the task expects |
| 2026-11-22 | Ready for review | Done by the task agent. `main.rs`: the form drawn by `layout.shown_sections()`/`shown_items()` (the layout cloned beside `shell`), `draw_structure_lists` split into `draw_structure_list` and removed, the contact heading kept by `draw_window_field`'s own rule; Other Metadata from `layout.shown_properties()`, led by `status_text(rating)` while Rating and Label is hidden; Copy/Paste All Fields through `copy_shown`/`paste_shown`; `window_editor_properties` `#[cfg(test)]` returning the default layout's `shown_properties` (its three tests unchanged and passing); the Settings window's page buttons, the pages General, External Programs (no heading) and Metadata Window (`draw_layout_rows`, Move Up before each check box in `horizontal_wrapped`, a hidden section's items greyed, a scroll area of at most `LAYOUT_PAGE_MAX_HEIGHT` = 420 points or half the program's window, Restore Defaults), `settings_page` set only when the window opens (General from Settings, External Programs from Open in Another Program and Ctrl+E). `shell.rs`: `SettingsPage`. `shell_text.rs`: `SettingsText::Page`, `RestoreDefaults`. `lib.rs`: re-export. Glossary §6: General, Metadata Window, Restore Defaults; External Programs and Move Up extended. Four tests; test inventory row (1260 -> 1264) and totals. check.ps1 all OK (2332 passed, 78 ignored), no clippy warnings |
| 2026-11-22 | Done | Reviewed: every point checked against the task; the form drawn by `shown_sections`/`shown_items`, the contact heading still drawn once before Address (first of `ContactInfo.fields()`), so it moves with the item; Other Metadata from `shown_properties`, led by `status_text` of the current rating while Rating and Label is hidden; Copy/Paste through `copy_shown`/`paste_shown` with the same layout; the page set only when the window opens (Ctrl+E goes through `Command::OpenInAnother`); Restore Defaults leaves the sections' open state; no string literal in drawing code; no panic path. One crate, seven files: no extra code review needed. For the hand check: a hidden section's items are drawn disabled (`add_enabled_ui`), not only greyed, so they cannot be checked or moved until the section is shown again; their own checks kept. Reviewer: CHANGELOG (Added), `known-limits.md` (the Paste count after a layout change, an older RawStride, a hand-written partial order line), plan row 2b, README index, done list; README's Planned "Settings" item stays until U8. The owner's hand check follows. check.ps1 all OK (2332 passed, 78 ignored), no clippy warnings |
