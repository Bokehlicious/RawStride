# T300. The metadata window's form to and from a template file — Done

- **Why:** ADR 0017 U13-Q7, U13-Q23, U13-Q24 and U13-Q25 (owner, 2026-11-24; part U13d). The
  metadata window gets **Load Template…** and **Save as Template…** (T301). This task makes the
  two in plain Rust, without egui: a `TemplateFile` put into the window's form as unsaved edits,
  and a `TemplateFile` made from what the form shows.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/editor.rs`,
  `crates/pw-loupe/src/template_form.rs`, `crates/pw-loupe/src/lib.rs` (exports only),
  `docs/test-inventory.md`. `main.rs` is a binary that reaches these only through `lib.rs`'s
  `pub use`, so `load_template` is `pub` (as `copy_shown` and `paste_shown` are) and
  `window_template` is added to `lib.rs`'s `pub use template_form::{…}` list.
- **Background (read before starting):** ADR 0017 §6.5 U13-Q4, U13-Q5, U13-Q7, U13-Q9 (with its
  amendment from T289's review), U13-Q12, U13-Q23–U13-Q25; U10b-Q4 (Copy and Paste All Fields
  and the layout). `editor.rs`: `EditorForm::copy_shown`, `paste_shown`, `put_template_fields`,
  `template_values`, `field_edit`, `list_entries`, `EditorField::text_of`, `check_box`,
  `choice_list`, `read_only_text`, `list_read_only`, `ListEntry` (`pasted`, and how a typed or
  pasted entry is told from one read from the file), `LOCATION_CREATED`, `LOCATION_FIELDS`,
  `CONTACT_INFO`. `editor_layout.rs`: `EditorLayout::is_shown`, `EditorItem::of_field`,
  `EditorItem::List`. `template_form.rs`: the module comment, `TemplateForm::load` and
  `template` (how a form becomes a `TemplateFile`), `TemplateRefused`, `TemplateFieldError`,
  `TemplateItem`, `refused_text`, `not_loaded_lines`. `pw_metadata::template_file`:
  `TemplateFile` (`fields`, `add`, `new`, `check_variables`), `can_add`;
  `pw_metadata::variables::escape`.
- **What:**
  1. **Load into the window's form** (`editor.rs`, a `pub` method of `EditorForm`, by
     this name, as T301 calls it: `load_template(&mut self, template: &TemplateFile, layout: &EditorLayout) ->
     Vec<String>`): each field of the template goes into the form **as typed**, an unsaved
     edit, as Paste All Fields puts a copied text (so its variables are checked and filled per
     image, U13-Q9):
     - a text, Lang Alt (`x-default`), date, number or choice field gets the template's value;
       Copyrighted is ticked for `True` and unticked for `False`;
     - a list of plain text values (`can_add`) **without** its id in `add` gets the template's
       entries in place of the form's; **with** it, the form's own entries are kept and the
       template's added after them, an entry the form already holds (exact text) not added again
       (U13-Q7, U13-Q12); the list text is split and joined as the form's own list text is
       (`list_entries`, `LIST_SEPARATOR`), so a template entry holding a comma becomes two
       entries (the known limit "A comma in an entry"; not reported as not loaded);
     - Creator's Contact Info: each sub-field in the template replaces that contact field's text,
       the others are kept;
     - Location Created: each sub-field in the template replaces that sub-field of the form's
       first entry, the others are kept (an entry is added when the form has none); a sub-field
       the form has no field for is not loaded;
     - any other structure list: the template's entries replace the form's, each as pasted.
     **Not loaded**, and returned by their XMP id (`property` or `property/sub-field`) in the
     template's order (U13-Q23; a hidden or read-only contact field as
     `Iptc4xmpCore:CreatorContactInfo/{sub-field}`, as `put_template_fields` names a sub-field):
     a field or list the layout hides (`EditorLayout::is_shown`), one
     shown read-only (`read_only_text`, `list_read_only`), a property the form has no editable
     field or list for, a value of a shape the field cannot take, a Copyrighted that is neither
     `True` nor `False`. The form's start texts are not changed, so what was loaded counts as
     changed (the window then asks before leaving the image, as after typing). `put_template_fields`
     (the template window's, on an empty form) stays as it is; shared parts may be factored out
     without changing what it does.
  2. **A template from what the window shows** (`template_form.rs`, a `pub` function, by this
     name, as T301 calls it: `window_template(form: &EditorForm, layout: &EditorLayout) -> Result<TemplateFile,
     TemplateRefused>`, with any `pub(crate)` helper it needs in `editor.rs`): the items
     Copy All Fields copies (`copy_shown`: the editable fields the layout shows that are not
     read-only, and the shown structure lists that are not read-only) that have a value, each a
     checked field, `add` empty (U13-Q7):
     - a field's value is made from its text as `template_values` makes it (`field_edit` with
       an empty start text), but from the text with the parts read from the image escaped first
       (the braces point below), so an image's own `{b}` never becomes an unknown variable;
       Copyrighted gives `True` when its box is ticked and nothing when not (the form cannot
       tell an image's `False` from no value, and Copy gives an unticked box no text either;
       reviewer's decision at the cold read, within U13-Q7); a field without a value gives
       nothing;
     - the shown contact fields with a value give one Creator's Contact Info structure of only
       those sub-fields; Location Created its first entry's sub-fields of `LOCATION_FIELDS` with
       a value (not the sub-fields the form does not show, such as Location Name and GPS);
       another structure list its entries that write something, whole, as Copy copies them;
     - **braces** (U13-Q25): what was read from the image and left unchanged is put in the
       template with `variables::escape`, so the image's own `{` and `}` stay text; what was
       typed or pasted is put as it stands, its variables kept. Per kind: a field whose text
       equals its start text is escaped as a whole; in a list of plain text values each entry
       that is also in the start text's entries is escaped and the others are not; a structure
       list's entries are taken with `ListEntry::written` (which already escapes unchanged base
       texts and keeps typed or pasted ones, as the window's own write does), through a new
       `pub(crate)` helper in `editor.rs` (`TemplateValues`' `value()` is not escaped and must
       not be used for this);
     - refused with `TemplateRefused::FieldErrors` when a shown item has an error (each named
       with a `TemplateItem`, as `TemplateForm::template` names them; their doc comments, which
       say "checked item", are widened to say so), with `Empty` when nothing has a value, and
       with `Variables` when `TemplateFile::check_variables` fails. A hidden or read-only item's
       error never refuses it. `TemplateFile::new` then keeps only `x-default` of a Lang Alt,
       the first Location Created entry, and leaves out a structure list over its limit
       (U13-Q5); the tests expect that.
- **Tests to add:** `editor.rs` or `template_form.rs` (the agent chooses where each fits), on forms
  made with `EditorForm::from_read` of hand-made reads as the existing tests make them:
  - Load: a text, a Lang Alt, a date, Copyrighted `True` and `False`; Keywords without + replaces,
    with + adds after the form's own and skips one already there; two contact sub-fields replace
    only those two; a Location Created City replaces only City of the first entry; Location
    Shown's entries replace the form's; a field hidden by the layout, one read-only, an unknown
    property and Location Created's Location Name are returned as not loaded and leave the form
    as it was there; afterwards the form's batch edits (`batch_edits`) hold the loaded fields as
    changed; a loaded Headline `{date}` on an image whose Headline differs gives an edit whose
    value is `{date}` as typed (not `{{date}}`).
  - Window template: a form read from an image with a Headline `a {b}` unchanged gives
    `a {{b}}`; Keywords read as `x{1}` with a typed `{date}` added give `x{{1}}` and `{date}`;
    a Location Shown entry read with a City `{c}` gives `{{c}}`; the same Headline typed as
    `{date}` gives `{date}`; Copyrighted ticked gives `True`, unticked nothing; a hidden field and a
    read-only field are left out; an empty form is `Empty`; a shown field with an error refuses
    it and names it; `add` is always empty; contact fields and Location Created give one
    structure each.
  - Round trip: for a form with every kind of field shown, typed texts without braces, Load of
    `window_template` of it into an empty form of the same layout gives the same texts (a text
    with braces read from an image comes back doubled, `{{b}}`, which writes `{b}`; Load does
    not unescape).
  - The test inventory's `pw-loupe` row and totals are updated.
- **Not in this task:** the buttons, the dialogs and the template thread (T301); any text the
  window shows (T301); the right-click menu (T303); a change to the template window's form.
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Order:** may run beside T299 (other crate). T301 waits for it.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  No hand check of its own (T301's covers it). At Done the reviewer records U13-Q24 in
  `docs/tasks/known-limits.md` (after Load Template…, "Apply to {n} Marked" replaces a list
  without + only on the window's image; the others get adds and removes).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0017 U13-Q7, U13-Q23–U13-Q25, owner, 2026-11-24; part U13d). Waits for its cold read and the chain review of T299–T303 |
| 2026-11-24 | Blocked | Cold read (fresh agent). Fixed: `lib.rs` added (exports; `main.rs` is a binary), `load_template` `pub`; the values made from the text with what was read escaped first, per kind (whole field, list entries in the start text, `ListEntry::written` for structure entries), as `field_edit` with an empty start would otherwise refuse an image's own `{b}`; Copyrighted gives `True` or nothing (reviewer's decision within U13-Q7: the form cannot tell `False` from no value); Location Created only its shown sub-fields; hidden contact fields named per sub-field; a comma in a loaded entry splits it (known limit); `TemplateFile::new`'s drops named; the `{date}` and round-trip tests made writable (Load does not unescape) |
| 2026-11-24 | Blocked | Chain review (T299–T303; recorded in T299's log): the two names fixed, as T301 calls them; U13-Q24's known limit recorded by the reviewer at Done |
| 2026-11-24 | Open | Cold read and chain review done (see T299's log). Set Open; may run beside T299 |
| 2026-11-24 | Ready for review | Done by the task agent. `editor.rs`: `pub fn EditorForm::load_template` (fields, + lists merged, contact sub-fields, Location Created's first entry, other structure lists as pasted; not loaded returned in the template's order), `pub(crate) fn window_template_values` (only shown, not read-only items; unchanged texts escaped, list entries of the start text escaped, structure entries `ListEntry::written`, Copyrighted `True` or nothing); `put_template_fields` unchanged in behaviour, its field part shared (`template_field_text`), the list errors shared with `template_values` (`entry_errors`). `template_form.rs`: `pub fn window_template`, the tail shared with `TemplateForm::template` (`finish_template`), the doc comments of `TemplateFieldError` and `TemplateRefused` widened; `lib.rs` exports it. 15 tests in `editor.rs`; test inventory `pw-loupe` 1397 -> 1412, totals 2543 passed, 78 ignored. Noted: a loaded value equal to the field's (or Location Created sub-field's) start text is not a change, so a template `{date}` on an image whose text is literally `{date}` is kept as text (as typing it would); Location Created errors are named per sub-field without the entry, as `TemplateForm::template` names them. `.\tools\check.ps1` all OK, no clippy warnings |
| 2026-11-24 | Done | Reviewed by the reviewer against every point and test; no independent review needed (one crate, four files, not the safe write). Approved by the owner 2026-11-24. No finding to fix. Leftovers: U13-Q24 (after Load Template…, "Apply to {n} Marked" replaces a list without + only on the window's image) and, from the review, a Location Created error in an entry after the first refusing Save as Template… (only the first entry is saved; the error is named without its entry) are in `known-limits.md`, "Templates in the metadata window (ADR 0017 U13d)"; the agent's two notes (a loaded value equal to the start text is no change; Location Created errors named per sub-field) are as the template window does and need no home. No CHANGELOG line (nothing the user sees before T301). check.ps1 all OK (run by the task agent after its last change, with T299's uncommitted `pw-metadata` changes in the tree; T300 uses none of them). No hand check of its own: T301's covers it |
