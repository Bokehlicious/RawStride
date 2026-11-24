# T304. The metadata window in two columns — Done

- **Why:** ADR 0017 §6.3 U10b-Q10 (owner, 2026-11-24, at the hand check of U13d): the metadata
  window's sections are drawn in two columns, as the template window's are (U13-Q13), instead of
  one long list. Nothing else in the window changes.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/lib.rs` (exports only),
  `docs/test-inventory.md`.
- **Background (read before starting):** ADR 0017 §2.3.1 (the metadata window), §6.3 U10b-Q2,
  U10b-Q9 and U10b-Q10, §6.5 U13-Q13 (the template window's two columns). `main.rs`:
  `draw_metadata_window` (the window, `WINDOW_SIZE`, the size and position it opens with,
  `metadata_window_position`, the scroll area `metadata-window-scroll`), `draw_window_form` (the
  sections as `CollapsingHeader`s, then the open conflicts, the warnings and Other Metadata),
  `draw_window_field`, `draw_structure_list`, `draw_rating_buttons`, `MetadataWindow`; the
  template window's `draw_template_sections` and `draw_template_section` (the model: the split by
  `left_column_sections` of last frame's heights, `ui.columns(2, …)`, `push_id` per section) and
  its `section_heights`. `shell.rs`: `TEMPLATE_COLUMN_MIN_WIDTH`, `TEMPLATE_WINDOW_MIN`,
  `TEMPLATE_WINDOW_DEFAULT`, `TEMPLATE_WINDOW_MARGIN`, `template_window_size` and its test
  `the_template_window_opens_wide_enough_for_two_columns`. `template_form.rs`:
  `left_column_sections` (used as it is, not changed).
- **What:**
  1. **The columns** (`main.rs`): the shown sections (`layout.shown_sections()`, in the layout's
     order) are drawn in two columns when the scroll area's inner width is at least twice
     `TEMPLATE_COLUMN_MIN_WIDTH` (the same width as the template window, so both windows change
     at the same width), otherwise in one, as today. The left column takes the first sections and
     the right the rest, split by `left_column_sections` of each section's height as drawn in the
     last frame (a collapsed section counts with its header's height); on the first frame, or
     when a shown section has no height yet, the split uses the number of items each section
     shows (`layout.shown_items(section)`, which is empty for Rating and Label, counted as one).
     The estimates come from a plain function, for example
     `section_estimates(sections: &[EditorSection], layout: &EditorLayout, heights:
     &HashMap<EditorSection, f32>) -> Vec<f32>`: last frame's heights when every shown section
     has one, else the item counts for all of them (as the template window's
     `draw_template_sections` does). Each section's height drawn now, in one column or two, is
     kept in a new `MetadataWindow` field `section_heights` (a `HashMap<EditorSection, f32>`,
     empty in `MetadataWindow::new`) for the next frame; `draw_window_form` gets it as a new
     `&mut` parameter, passed from its call in `draw_metadata_window`. Since the split follows the
     heights, opening or collapsing a section may move a section to the other column, the split
     may change frame by frame while a header's animation runs, and the first frame after a
     change between one and two columns uses heights measured at the other width; all of this is
     accepted ("about equally long", U10b-Q10) and is not to be worked around.
  2. **One section, unchanged** (`main.rs`): the body of today's loop in `draw_window_form` (the
     `CollapsingHeader` with its `id_salt`, its open state from `shell`, `section_enabled`, the
     rating buttons, the fields, the contact fields and the structure lists, the toggled
     section and the rating choice) moves into a function of its own that draws one section
     inside `push_id(("metadata-window-section", section))`, as `draw_template_section` does, and
     returns a small struct with its height (the height of the `push_id` response's rect),
     whether its header was clicked and the rating button pressed
     (`draw_rating_buttons`' `Option<RatingKey>`); the caller fills `FormResponse`'s `toggled` and
     `rating` from it as the loop does today. The two columns are drawn in one closure that walks
     both column `Ui`s in turn (`for (ui, part) in [(left_ui, left), (right_ui, right)]`, as
     `draw_template_sections`), so `state`, `values` and the results are borrowed one column at a
     time. Inside two columns a section's ids are the same in either column; when the window
     changes between one and two columns the ids change (a field loses the keyboard focus, a
     header may replay its animation), which is accepted. Nothing a section draws changes.
  3. **Below the columns, full width** (`main.rs`): everything `draw_window_form` draws after the
     sections (the open conflicts, the warnings, Other Metadata with the rating line of U10b-Q9,
     the conflicts a rule decided and the notices) stays after them, across the whole width, as
     today. The rows above the scroll area (the marked images' lines) and the buttons'
     panel at the bottom are not changed.
  4. **The size it opens with** (`shell.rs`, exported from `lib.rs`): `METADATA_WINDOW_DEFAULT`
     (room for two columns, with the scroll bar and the spacing between them, as
     `TEMPLATE_WINDOW_DEFAULT`; height 640 as today) and `metadata_window_size(screen)`: the
     program window less `TEMPLATE_WINDOW_MARGIN` on each side, between `TEMPLATE_WINDOW_MIN` and
     `METADATA_WINDOW_DEFAULT`, each side apart, never larger than the program window
     (as `template_window_size`). In `draw_metadata_window` it replaces `WINDOW_SIZE` (removed)
     both in `default_size` and as the size `metadata_window_position` is given on the first
     frame when egui remembers no size; the size the window had when it was last open in this
     session still wins, as today. `main.rs` imports both through `pw_loupe::{…}` as it does the
     template window's. The window stays resizable with no minimum size added (it has none
     today), and narrowed below twice `TEMPLATE_COLUMN_MIN_WIDTH` it shows one column.
- **Tests to add:** `shell.rs`: a large program window (1920 × 1080) gives
  `METADATA_WINDOW_DEFAULT`, whose width less the margins is at least twice
  `TEMPLATE_COLUMN_MIN_WIDTH`; exactly the default plus the margins gives the default; a small
  program window (for example 600 × 400) gives a size no larger than it; the height of the default
  is 640. `main.rs`, without a window: `section_estimates` (point 1) with `EditorLayout::default()`,
  with every shown section's height, with one height missing (item counts for all, Rating and
  Label as one), and with none. The
  test inventory's `pw-loupe` row and totals are updated.
- **Not in this task:** a setting for one or two columns; sections that are not collapsible
  (the template window's heading bars); a change to the template window; the window's size saved
  between sessions; Other Metadata, the conflicts or the warnings in columns.
- **Rules:** `docs/tasks/README.md` rules 1–13 (no new text the user sees is expected; if one is
  needed, stop and ask); no new dependency; no `unwrap`/`expect` or unchecked indexing in
  non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At Done the reviewer adds to the CHANGELOG (Changed) that the metadata window shows its
  sections in two columns when it is wide enough. **The owner's hand check:** the metadata window
  opens wide enough for two columns in a large program window, the sections in the layout's
  order filling the left column and then the right, about equally long; narrowed, it shows one
  column, widened again two; a section collapsed or opened keeps its state (also after a
  restart, U10-Q6) and the columns rebalance; the order and hidden sections set on the Settings
  window's Metadata Window page are followed; the conflicts, the warnings and Other Metadata are
  below the columns across the whole width; typing in a field, the ▼ lists, Location Created's
  entries, Rating and Label's buttons, field errors, Previous / Next, Load Template… and
  Save as Template…, Copy / Paste All Fields and "Apply to {n} Marked" work as before; the locked
  window during a batch looks as before; in a small program window the window fits inside it.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0017 U10b-Q10, owner, 2026-11-24). Waits for its cold read |
| 2026-11-24 | Blocked | Cold read (fresh agent); fixed in place by the reviewer: the new field initialised in `MetadataWindow::new` and passed to `draw_window_form` as a new parameter; heights kept in one column too; the fall-back as a named plain function (`section_estimates`) with its signature and the test's `EditorLayout::default()`; the section function returns its height, its header's click and its rating button; the two columns in one closure walking both `Ui`s (borrows); ids stable within two columns, a change between one and two columns changes them (accepted); the decided conflicts and the notices named among what stays below; no minimum size added; the imports through `pw_loupe`; the one-frame jumps of the split accepted, not worked around. The named files are enough |
| 2026-11-24 | Open | Cold read done; the owner accepted that a section may move to the other column when another is opened or collapsed (to be changed at the hand check if it is disturbing). Set Open (owner, 2026-11-24) |
| 2026-11-24 | Ready for review | Done by the task agent. `shell.rs`: `METADATA_WINDOW_DEFAULT` (840 × 640) and `metadata_window_size`, two tests; `lib.rs`: both exported. `main.rs`: `WINDOW_SIZE` removed, `metadata_window_size(screen)` in `default_size` and for the first frame's position; `MetadataWindow::section_heights`; `draw_window_form` gets `heights`, splits by `left_column_sections` of `section_estimates` and draws each section with the new `draw_window_section` (inside `push_id`, returns `SectionResponse`: height, header click, rating button) through one closure, in `ui.columns(2, …)` or one column; what follows the sections unchanged; three tests of `section_estimates`. Only `metadata_window_size` is imported in `main.rs`: `METADATA_WINDOW_DEFAULT` is not used there and an unused import would warn. Test inventory: pw-loupe 1436 -> 1441, totals 2574 passed, 78 ignored. check.ps1 all OK, no clippy warnings |
| 2026-11-24 | Done | Reviewed against every point and test; no finding to fix (nits left: the test's `48.0` for twice the margin). egui 0.37 checked: `columns` gives both column `Ui`s the same scope id, so a section's ids are the same in either column. check.ps1 run again by the reviewer: all OK, 2574 passed, 78 ignored, no clippy warnings. Nothing left. CHANGELOG: one Changed line. Task moved to done/, task index and plan updated; the owner's hand check follows |
| 2026-11-24 | Done | Hand-checked by the owner: every point of the hand check works, approved 2026-11-24; nothing to change, so the accepted moves of sections between columns stay |
