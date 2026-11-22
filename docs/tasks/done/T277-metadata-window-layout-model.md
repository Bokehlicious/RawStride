# T277. The metadata window's layout: which sections and items, in which order (model) — Done

- **Why:** ADR 0017 §2.3.1 and step U10b: the user chooses which sections and fields of the
  metadata window are shown, and in which order, so unused fields take no room. The owner
  answered the step's questions on 2026-11-22 (ADR 0017 §6.3, U10b-Q1–U10b-Q9, every proposal
  approved). This task is the plain-Rust part: the layout, its default, its changes, its lines in
  `settings.txt`, and "Copy/Paste All Fields" limited to what is shown. T278 draws the window by
  it and adds the Settings page (Blocked until this task is Done). Nothing the user sees changes
  in this task: the default layout is today's form.
- **Crates:** `pw-loupe` (library part). **Files:** `crates/pw-loupe/src/editor_layout.rs` (new),
  `crates/pw-loupe/src/editor.rs`, `crates/pw-loupe/src/lib.rs`,
  `crates/pw-loupe/src/settings.rs`, `docs/test-inventory.md`.
- **Background (read before starting):**
  - `editor.rs`: `EditorSection` (six sections, `EditorSection::ALL` in form order),
    `EditorField` (47 fields, `EditorField::ALL`, `section()`, `property()`, `struct_field()`,
    `label()`), the structure lists (`EditorForm::ALL_LISTS`, `EditorForm::lists_at`,
    `EditorForm::lists_after_list`), `EditorForm::copy` and `EditorForm::paste` (T125, U12b-Q2).
    The eight contact fields are the fields whose `struct_field()` is `Some`
    (`Iptc4xmpCore:CreatorContactInfo`, the constant `CONTACT_INFO`).
  - Today's form order is drawn by `draw_window_form` in `main.rs` (about lines 8060–8120): per
    section, `lists_at(section, None)`, then each field of the section followed by
    `lists_at(section, Some(field))`, each list followed by `lists_after_list(list)`. The
    default layout of this task is exactly that order. `main.rs` is not changed in this task.
  - `settings.rs`: `Settings` (top level: `create_iim`, `fallback`, `shell`,
    `external_programs`), `load_settings_and_library` (one problem per bad line, naming the line
    number), `save_settings` and the line writers, the `companion_suffixes` line (a list separated
    by `, `). `main.rs`'s `save_settings_window` keeps only the theme of `chosen.shell` and takes
    every top-level field of `chosen` (`..chosen`); so the layout is a **top-level** field of
    `Settings`, not of `ShellSettings`.
- **What:**
  1. **`EditorItem`** (new, in `editor_layout.rs`, re-exported from `lib.rs` as the other editor
     types are): one item of a section's list (U10b-Q5).
     ```rust
     pub enum EditorItem {
         /// A field that is not a contact field.
         Field(EditorField),
         /// The eight contact fields together, "Creator's Contact Info".
         ContactInfo,
         /// A structure list by its XMP id, one of `EditorForm::ALL_LISTS`.
         List(&'static str),
     }
     ```
     with `Clone, Copy, Debug, PartialEq, Eq, Hash`, and:
     - `of_field(field) -> EditorItem`: `ContactInfo` for a contact field, else `Field(field)`;
     - `section(self) -> EditorSection` (a list's section is the section `lists_at` gives it;
       find it by asking `lists_at`/`lists_after_list` per section, or add a small public
       function in `editor.rs` that returns a list's section from `LIST_SPECS`);
     - `fields(self) -> Vec<EditorField>`: the field; the eight contact fields in `ALL` order; empty
       for a list;
     - `property(self) -> &'static str`: the field's property, `Iptc4xmpCore:CreatorContactInfo`,
       or the list's id;
     - `label(self) -> String`: `field.label()`; for `ContactInfo` and a list
       `property_label(<id>, Language::En)` (the same names the window draws today);
     - `name(self) -> &'static str` and `from_name(&str) -> Option<EditorItem>`: the item's name in
       `settings.txt`, exactly as in the table under point 4;
     - `defaults(section) -> Vec<EditorItem>`: the section's items in today's form order (above),
       `ContactInfo` once, in the place of the first contact field (Address). Rating and Label has
       no items.
     `LIST_SPECS`, `CONTACT_INFO` and the list id constants are private to `editor.rs` and
     cannot be seen from `editor_layout.rs`: use public items (`EditorForm::ALL_LISTS`,
     `lists_at`, `lists_after_list`, `EditorField::Address.property()`) or add a small
     `pub(crate)` helper in `editor.rs`. **A value outside the valid ones** (a `List` id not in
     `ALL_LISTS`, a `Field` holding a contact field; only code can make one, never
     `from_name`): no panic (`AGENTS.md` §3); `section()` gives the field's section, or Content
     for an unknown list; `name()` gives the list's id itself, or the field's own name; the
     layout never holds one, so `is_shown` is false for it and `set_item_shown`/`move_item_up`
     do nothing. A test shows each.
  2. **`EditorLayout`** (new, in `editor_layout.rs`, re-exported): `Clone, Debug, PartialEq`
     (`Settings` derives them), private fields, `Default` = every section in `EditorSection::ALL`
     order and every section's `defaults`, all shown (U10b-Q2). Public:
     - `sections(&self) -> Vec<(EditorSection, bool)>`: every section in the layout's order with
       whether it is shown;
     - `items(&self, section) -> Vec<(EditorItem, bool)>`: every item of `section` in the layout's
       order with whether the item itself is shown (not looking at the section);
     - `shown_sections(&self) -> Vec<EditorSection>`; `shown_items(&self, section) ->
       Vec<EditorItem>` (the items shown, whether or not the section is);
     - `is_shown(&self, item) -> bool`: the item is shown **and** its section is shown;
     - `set_section_shown(section, bool)`, `set_item_shown(item, bool)`;
     - `move_section_up(section)` and `move_item_up(item)`: one place up, inside the item's own
       section (U10b-Q2: an item never moves to another section); nothing on the first place;
     - `shown_properties(&self) -> Vec<&'static str>`: the property of every item `is_shown`
       gives, each once, in the layout's order, then `xmp:Rating` and `xmp:Label` **always**,
       whether Rating and Label is shown or not (U10b-Q9, owner 2026-11-22: with the section
       hidden, T278 shows the image's current rating as one line instead of the rating read when
       the window opened). It is the list `other_values` will get in T278: what it leaves out is shown
       read-only under Other Metadata (U10b-Q3). For the default layout it holds the same
       properties as `main.rs`'s `window_editor_properties()` (every field's property once,
       `xmp:Rating`, `xmp:Label`, every list of `ALL_LISTS`), in any order;
     - `is_default(&self) -> bool`.
  3. **Copy and paste only what is shown (U10b-Q4).** In `editor.rs` two new public methods on
     `EditorForm`:
     - `copy_shown(&self, file: &str, layout: &EditorLayout) -> CopiedFields`: as `copy`, but only
       the fields and lists whose item `layout.is_shown`; so `CopiedFields::count` (the Paste
       button's tooltip) counts only those;
     - `paste_shown(&mut self, copied: &CopiedFields, layout: &EditorLayout)`: as `paste`, but a
       field or list whose item is not shown is left as it is, also when `copied` holds it (the
       layout may have changed between the copy and the paste).
     `copy` and `paste` keep their signatures and behaviour (every editable field; they may call
     the new methods with `EditorLayout::default()`); no existing test changes. `batch_edits`
     needs no change: a hidden item cannot be edited in the window (T278 does not draw it) and
     `paste_shown` does not touch it; the doc comment of `paste_shown` says so, and that a hidden
     item therefore never has a field error (errors come only from changed text, and a conflict
     choice moves the start text along). An item shown again after the Copy was not copied, so
     it is not pasted.
  4. **Names** (U10b-Q7), lower case, fixed: a test pins every one, so a rename is seen.
     - Sections: `content`, `creator_rights`, `time_place`, `assignment`, `licence`,
       `rating_label` (as the existing `editor_*` keys).
     - Fields: `title`, `headline`, `description`, `alt_text`, `ext_description`,
       `description_writer`, `keywords`, `person_shown`, `organisation_name`,
       `organisation_code`, `event`, `creator`, `creator_jobtitle`, `copyright`, `credit`,
       `source`, `usage_terms`, `web_statement`, `copyrighted`, `date_created`, `sublocation`,
       `city`, `state`, `country`, `country_code`, `instructions`, `job_id`, `subject_code`,
       `scene_code`, `urgency`, `category`, `supplemental_categories`, `model_release_status`,
       `model_release_id`, `minor_model_age`, `property_release_status`, `property_release_id`,
       `data_mining`, `digital_source_type`; the contact fields together `contact_info`.
     - Lists: `person_details`, `product_shown`, `artwork`, `contributor`, `location_created`,
       `location_shown`, `genre`, `about_cv_term`, `registry_entry`, `image_creator`,
       `copyright_owner`, `licensor`, `image_supplier`.
     The section names and their `from_name` go beside `EditorItem`'s (free functions or a small
     extension in `editor_layout.rs`; `EditorSection` stays in `editor.rs`).
  5. **Restoring an order from names**, in `editor_layout.rs` (used by point 6):
     `set_section_order(&mut self, names: &[EditorSection])` and `set_item_order(&mut self, names:
     &[EditorItem])`. The order given is kept for what it names (a repeated entry counts once, at
     its first place); what it does not name is put back in its default place: for each missing
     section or item, in default order, right after the one before it in the default order (of
     its own section), or first when it is the first. So an item a later version adds shows up
     where the default has it (U10b-Q7). Items keep their own section whatever order the list has.
     Both keep every section's and item's shown flag as it was (the hidden lines may come before
     the order lines in the file).
  6. **`settings.txt`** (`settings.rs`): `Settings` gets `pub editor_layout: EditorLayout`
     (default `EditorLayout::default()`), and four keys, comma-separated names written with `, `
     as `companion_suffixes`:
     - `editor_section_order = content, licence, …` (every section, the layout's order);
     - `editor_item_order = title, headline, …` (every item of every section, sections in
       `EditorSection::ALL` order, each section's items in the layout's order);
     - `editor_hidden_sections = …` and `editor_hidden_items = …` (the hidden ones, in the same
       orders).
     Each line is written **only when it differs from the default** (an order line when the order
     is not the default one; a hidden line when something is hidden), so a file with the default
     layout is written exactly as today and the test of the whole file's text
     (`settings.rs`, about line 1024) and the `ends_with` and exact-text tests (about lines
     2211, 2439, 2584) do not change. `shell_lines` takes only `&ShellSettings`, so the writer
     goes in `save_settings_with_library` (about lines 595–597), between `shell_lines` and
     `external_program_lines`. Reading: each name through `from_name`; each unknown name is
     skipped and gives one problem naming the line number and the name (two unknown names, two
     problems), the rest of the line is used; an order line goes through point 5. An empty value
     (`editor_hidden_items =`) names nothing: nothing hidden, or the default order. A key given
     twice: the last line wins, as for the other keys. A file without these lines gives the
     default layout.
- **Tests to add** (in the test modules of the files changed):
  - `editor_layout.rs`: the default sections are `EditorSection::ALL`, all shown; the default
    items, written out per section: Content `title, headline, description, alt_text,
    ext_description, description_writer, keywords, person_shown, person_details,
    product_shown, artwork, organisation_name, organisation_code, event`; Creator and Rights
    `creator, creator_jobtitle, contact_info, copyright, credit, source, usage_terms,
    web_statement, copyrighted, contributor`; Time and Place `date_created, sublocation, city,
    state, country, country_code, location_created, location_shown`; Assignment `instructions,
    job_id, subject_code, scene_code, genre, about_cv_term, urgency, category,
    supplemental_categories, registry_entry`; Licence (PLUS) `image_creator, copyright_owner,
    licensor, image_supplier, model_release_status, model_release_id, minor_model_age,
    property_release_status, property_release_id, data_mining, digital_source_type`; Rating and
    Label none (if the code gives another order, stop and report: the list above was read from
    `LIST_SPECS` and `EditorField::ALL`). Every `EditorField` is in exactly one item and every
    list of `ALL_LISTS` in exactly one; every name round-trips through `from_name` and the names
    are the table's (point 4); `of_field` of each contact field is `ContactInfo`; the default
    `shown_properties` holds every field's property, `xmp:Rating`, `xmp:Label` and every list,
    each once: 55 entries, all different (39 fields, `Iptc4xmpCore:CreatorContactInfo`, 13
    lists, the two rating properties; a test in `main.rs` about line 11506, used by T278,
    expects exactly that).
  - Hiding and moving: a hidden item is not `is_shown` and its property is not in
    `shown_properties`; a hidden section hides its items' properties while `items` still
    reports each item's own flag; with Rating and Label hidden `xmp:Rating` and `xmp:Label` are
    still in `shown_properties` (U10b-Q9); showing
    the section again shows them; `move_item_up` on the first item and `move_section_up` on the
    first section change nothing; `move_item_up` never takes an item into another section.
  - Point 5: an order naming only some items keeps them in that order and puts the rest in their
    default places (one test with an item left out in the middle of a section, one with the first
    item left out); a repeated name counts once.
  - `editor.rs`: `copy_shown` with a hidden field and a hidden list leaves both out (and `count`
    with them); `paste_shown` leaves a hidden field and a hidden list as they are although
    `copied` (made with the default layout) holds them, and pastes the shown ones; a hidden
    section's fields are left as they are too.
  - `settings.rs`: a layout with a moved section, a moved item, a hidden section and a hidden item
    saves and loads back equal; the default layout writes none of the four keys; an unknown name
    gives one problem with its line number and the other names of the line are used; a line
    leaving out an item gives that item in its default place; a hidden line before an order line
    gives the same layout as after it; an empty value gives the default.
- **Not in this task:** drawing the window by the layout, the Settings window's pages, the
  window's use of `copy_shown`/`paste_shown` and of `shown_properties`, the glossary (all T278);
  a choice for the inspector's panels (U10b-Q1); the companion suffixes' page (U8, U10b-Q8).
- **Rules:** `AGENTS.md` §3 (no panic paths in non-test code); no new dependency; no existing
  test changed; every new public type derives `Debug` (the workspace warns on
  `missing_debug_implementations`); a helper only tests use is `#[cfg(test)]`, so no dead-code
  warning comes before T278; `docs/tasks/README.md` rule 13 for texts (this task adds none the user sees:
  `label` reuses `field.label()` and `property_label`; the problem texts for the log are English
  as the other problems of `settings.rs`).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Open | Added. ADR 0017 §6.3 (U10b-Q1–U10b-Q9, the owner's answers 2026-11-22). First of the chain T277 → T278 (step U10b, order 2b) |
| 2026-11-22 | Open | Cold read: doable with the named files; the default order and the names match the code; no existing test or `Settings` literal breaks. Changed: `LIST_SPECS`, `CONTACT_INFO` and the list constants are private (public items or a `pub(crate)` helper named); what an invalid `EditorItem` gives (no panic, a test each); the order setters keep the shown flags; one problem per unknown name, an empty value, a key given twice; the writer's place in `save_settings_with_library`; `shown_properties` 55 entries, all different (a `main.rs` test T278 keeps expects it). Chain review (T277 → T278): the API T278 uses is all delivered; every U10b answer has a home. Changed: `Debug` on every new public type, test-only helpers `#[cfg(test)]`; `paste_shown`'s doc says a hidden item never has an error. The gap it found (the rating keys while Rating and Label is hidden) went to the owner: U10b-Q9, proposal A, so `shown_properties` always holds `xmp:Rating` and `xmp:Label` |
| 2026-11-22 | Ready for review | Done by the task agent: `editor_layout.rs` (new; `EditorItem`, `EditorLayout`, the names, `set_section_order`/`set_item_order`), `editor.rs` (`pub(crate) list_section`, `copy_shown`, `paste_shown`; `copy`/`paste` call them with the default layout), `lib.rs` (module, re-export), `settings.rs` (`Settings::editor_layout`, the four keys written only when not default, between the shell lines and the external programs). 25 new tests (16 + 3 + 6), no existing test changed; test inventory pw-loupe 1235 -> 1260, totals 2328. Notes for review: point 5 taken literally, so a line naming only some entries puts each missing one right after its default predecessor, which can move named entries (`editor_section_order = assignment` alone gives the default order); `name()` of a contact field held as `Field` gives its structure field's XMP id (`Iptc4xmpCore:CiEmailWork`), the reading of "the field's own name". check.ps1 all OK (2328 passed, 78 ignored), no clippy warnings |
| 2026-11-22 | Done | Reviewed: every point checked against the task; the default order comes from `lists_at`/`lists_after_list` as `draw_window_form` draws it; `move_item_up` swaps only within the section; the order setters keep the flags, so a hidden line before or after an order line gives the same layout; no panic path; the default layout writes no line. One crate, six files: no extra code review needed. Both notes accepted (owner, 2026-11-22): a hand-written order line naming only some entries may give the default order (point 5 as written; RawStride always writes every entry, and an entry a later version adds still gets its default place, U10b-Q7); `name()` of a contact field held as `Field` gives its structure field's XMP id. No CHANGELOG line (nothing the user sees until T278). check.ps1 all OK (2328 passed, 78 ignored), no clippy warnings |
