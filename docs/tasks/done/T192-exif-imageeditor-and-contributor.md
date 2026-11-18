# T192. Exif ImageEditor ↔ Contributor, the content editor (ADR 0007 §6.6) — Done

- **Why:** ADR 0007 §6.1 lists Exif 3.0 ImageEditor (0xA438) as mapped "both ways" to the
  Contributor whose role is "Content Editor" (IPTC Mapping Guidelines, Exif Note on Contributor).
  It was left out of T81. The owner decided the details on 2026-11-18 (ADR 0007 revision 21,
  §6.6). Today ImageEditor is neither read nor written.
- **Crates:** `pw-metadata` only. **Files:**
  - `crates/pw-metadata/src/read.rs`
  - `crates/pw-metadata/src/reconcile.rs`
  - `crates/pw-metadata/src/write_plan.rs`
  - `crates/pw-metadata/tests/read.rs`
  - `docs/test-inventory.md`
- **Terms used below:**
  - `CONTRIBUTOR` = `"Iptc4xmpExt:Contributor"` (a `Multi` structure `EntityWRole`, so a
    `Value::StructList`); its fields are `"Iptc4xmpExt:Name"` (Lang Alt), `"Iptc4xmpExt:Role"`
    (a list of texts, `Value::List`) and `"xmp:Identifier"` (`iptc_table.rs`, `EntityWRole`).
  - **Content editor URI:** `https://cv.iptc.org/newscodes/contentprodpartyrole/contentEditor`.
    An item of Role counts as the content editor role when it equals this URI or the same URI
    with `http://` instead of `https://` (exact text otherwise).
  - **The content editor** of a Contributor list: its **first** structure whose Role holds the
    content editor role. **Its name:** the shown text of its Name
    (`LangAlt::display(..).map(|(text, _)| text)`, `x-default` or else the first entry), `""`
    without a Name field.
- **What:**
  1. **One shared helper** in `reconcile.rs`, `pub(crate)`, used by points 4 and 6: given a
     `&[StructValue]` it returns the index of the content editor (`Option<usize>`), and a second
     one returns its name (`Option<&str>`: `None` without a content editor, `Some("")` for one
     without a Name). Put the URI, the `http://` form and `CONTRIBUTOR` as constants next to the
     other constants at the top of `reconcile.rs`, and add `StructValue` to its `use
     crate::value::{…}`. No `.unwrap()`, no unchecked indexing.
  2. `read.rs`: add `"-ExifIFD:ImageEditor"` to `IMAGE_TAGS` (after `"-ExifIFD:Photographer"`),
     and `image_editor: field("ImageEditor")` in `exif_fields`, so it is decoded and made `None`
     when blank exactly like Photographer.
  3. `reconcile.rs`, `ExifFields`: add `pub image_editor: Option<String>` after `photographer`.
     Every `ExifFields { … }` literal in the code uses `..ExifFields::default()` or names all
     fields; add the field where all are named (`read.rs` `exif_fields` at about line 622, and the
     unit test literal in `read.rs` at about line 1108 with `image_editor: None`).
  4. `reconcile.rs`, `reconcile_exif`: after Creator, when `present(&exif.image_editor)` gives a
     text `name`:
     - **No Contributor** (`work.current(CONTRIBUTOR)` is `None`): `work.fill` it with
       `Value::StructList(vec![editor])`, origin `Origin::Exif`, where `editor` is a
       `StructValue` with the fields, in this order, `("Iptc4xmpExt:Name", Lang Alt with only
       x-default = name)` and `("Iptc4xmpExt:Role", Value::List(vec![the https URI]))`.
     - **A Contributor whose content editor's name equals `name`** (exact text): nothing.
     - **Otherwise** (no content editor in the list, or one with another name): a conflict with
       rule `ConflictRule::Xmp`, values `[(current origin, current value), (Origin::Exif,
       exif_side)]`. `exif_side` is the current list with `editor` (as above) appended at the
       end when it has no content editor; otherwise the same list where only the content
       editor's Name changes: its `x-default` is set to `name` with `LangAlt::set_default`, so
       its other languages are kept (Q7, ADR 0007 §8); a content editor without a Name field
       gets `("Iptc4xmpExt:Name", Lang Alt with only x-default = name)` inserted as its first
       field. Its other fields and every other structure are unchanged. The stored value is not changed (XMP wins, §6.2). This applies to every file
       kind, a manufacturer RAW included (§6.4: there Exif is camera data and also only gives
       rule `Xmp`).
     - If the current value is not a `Value::StructList` (it cannot be from the reader, but the
       type allows it), do nothing.
     Update the module comment's point 4 and the step comment `// 4. Exif …` in `reconcile`
     (it says ImageEditor ↔ Contributor "are not part of this step yet"; only GPS stays out):
     the module comment's point 4 at about lines 12–14, the step comment at about line 213.
  5. `write_plan.rs`, `ExifPresent`: add `pub image_editor: bool` (`ExifIFD:ImageEditor`, Exif
     3.0) after `photographer`. In `read.rs` `file_state`, set it from
     `exif.image_editor.is_some()` (the RAW branch stays `ExifPresent::default()`).
  6. `write_plan.rs`, `exif_args`: for `CONTRIBUTOR` (add the constant next to `CREATOR`) a
     branch of its own **before** the existing `match` on the tags (today `exif_text` gives
     `None` for a `Value::StructList` and the function returns without writing). When
     `present.image_editor` is true: write `-ExifIFD:ImageEditor=<name>` with the content
     editor's name (point 1) of the new value; when the property was cleared (`value` is `None`),
     or the new list has no content editor, or its name is blank (`iim::is_blank`, as the reader
     treats a blank Exif value), write `-ExifIFD:ImageEditor=` (removes the tag). When `present.image_editor` is false, nothing (never created, §6.3).
     ImageEditor is never written to Artist. Update the module comment's **Exif** point and the
     doc comment of `exif_args` (about lines 21–23 and 587–590).
  7. `tests/read.rs`, `the_state_of_a_jpeg_names_only_the_exif_fields_it_has` (about line 539):
     its `ExifPresent { … }` literal names every field, so add `image_editor: false`. Change
     nothing else in that test; do the same in `write_plan.rs`'s test helper `all_exif` (set
     `image_editor: true`) and in any other literal the compiler names.
- **Tests to add:**
  - `reconcile.rs`, section `// --- Exif ---`:
    - `image_editor_fills_an_absent_contributor`: a JPEG reading with `image_editor:
      Some("Åsa")` and no Contributor gives Contributor = one structure (Name x-default "Åsa",
      Role [https URI]), origin `Exif`, no conflict, no notice.
    - `image_editor_equal_to_the_content_editor_gives_nothing`: XMP Contributor with a
      structure (Name "Åsa", Role [**http** URI]) and Exif "Åsa": no conflict, value and origin
      unchanged (also shows that `http://` counts).
    - `image_editor_differing_from_the_content_editor_is_a_conflict_that_keeps_the_others`: XMP
      Contributor [(Name "Bo", Role ["photographer"]), (Name "Åsa", Role [https URI])] and Exif
      "Eva": one conflict, rule `Xmp`; the Exif side is the same two structures with the second
      one's Name "Eva"; the stored value is still the XMP list.
    - `image_editor_with_no_content_editor_in_the_list_appends_one_on_the_exif_side`: XMP
      Contributor [(Name "Bo", Role ["photographer"])] and Exif "Eva": conflict rule `Xmp`, Exif
      side [(Bo…), (Name "Eva", Role [https URI])].
    - `the_first_content_editor_is_the_one_compared`: two content editors "Åsa" then "Eva", Exif
      "Åsa": no conflict.
    - `the_exif_side_keeps_the_other_languages_of_the_name`: content editor Name [(x-default
      "Åsa"), (sv "Åsa S")], Exif "Eva": the Exif side's Name is [(x-default "Eva"), (sv
      "Åsa S")].
    - `a_name_without_x_default_is_compared_by_its_first_entry`: content editor Name [(sv
      "Åsa")], Exif "Åsa": no conflict.
    - `a_content_editor_without_name_gets_one_on_the_exif_side`: content editor with only Role,
      Exif "Eva": conflict; the Exif side's content editor has Name x-default "Eva" as its first
      field and the same Role.
  - `reconcile.rs`, section `// --- RAW ---`:
    - `a_raw_file_image_editor_fills_an_absent_contributor_and_conflicts_with_rule_xmp`: a
      `FileKind::Raw` reading: with no Contributor in the sidecar, Exif fills as above; with a
      sidecar content editor of another name, a conflict with rule `Xmp`.
  - `write_plan.rs` tests:
    - `a_changed_contributor_writes_the_content_editor_to_an_existing_image_editor`: a JPEG
      with `all_exif()` (so Artist and Photographer exist too), Contributor set to [(Bo, photographer), (Eva, https URI)]:
      the args contain `-ExifIFD:ImageEditor=Eva` and no `Artist`/`Photographer` argument for it.
    - `a_contributor_without_content_editor_removes_image_editor`: same, list without a content
      editor: `-ExifIFD:ImageEditor=`; a content editor whose Name is only spaces:
      `-ExifIFD:ImageEditor=`; and a cleared Contributor: `-ExifIFD:ImageEditor=`.
    - `image_editor_is_not_created`: `image_editor: false`: no `ImageEditor` argument.
  - `tests/read.rs` (real ExifTool, as `the_state_of_a_jpeg_names_only_the_exif_fields_it_has`):
    - `exif_image_editor_fills_contributor`: a JPEG written with `-ExifIFD:ImageEditor=Åsa
      Öberg` reads Contributor with Name "Åsa Öberg" and the https Role, origin `Exif`, and
      `state.exif_present.image_editor` is true.
- **Not in this task:**
  - The Exif 3.0 UTF-8 data type: ImageEditor is written as Photographer is today (ExifTool's
    choice); nothing new.
  - The editor, the info panel and templates (`pw-loupe`): Contributor is already a list of
    structures there; no text is added.
  - GPS → Location Created (backlog); lists with qualifiers (T193).
  - A Contributor list of only empty structures counts as present (`is_blank_value` treats every
    `StructList` as non-blank, as for every list of structures today), so it gives a conflict
    rather than a fill; not changed here.
  - The Tauri app (D7).
- **Rules:** `docs/tasks/README.md`; ADR 0007 §6.6 is the specification; no user-visible text is
  added.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  and this task's file.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-18 | Open | Added (ADR 0007 revision 21, §6.6; owner's approval of E1–E3 the same day) |
| 2026-11-18 | Open | Cold read: a fresh agent found that `reconcile.rs` needs its own `CONTRIBUTOR` constant and `StructValue` import (point 1), that `LangAlt::display` gives a pair (terms), that `exif_args` returns before its `match` for a `StructList` so Contributor needs its own branch first (point 6), the unit-test literal at `read.rs` ~1108 (point 3), that a blank name must remove the tag, and that the write test needs `all_exif()` to mean anything. The reviewer also settled two cases the text left open: the Exif side changes only the Name's `x-default` (other languages kept, Q7) and a content editor without a Name gets one as its first field; tests added for these and for a Name without `x-default` |
| 2026-11-18 | Ready for review | Done by the task agent: points 1–7 and every listed test; besides them `reconcile.rs` has `content_editor_helpers_find_the_first_content_editor` (the two helpers directly) and constants for the Contributor's Name and Role fields. A Name field that is not a Lang Alt (the reader cannot give one) is replaced on the Exif side. `.\tools\check.ps1` all OK (1893 passed, 72 ignored), no clippy warnings |
| 2026-11-18 | Done | Reviewed: points 1–7 and every listed test match the text and ADR 0007 §6.6; the extra helper test and the Name/Role constants are fine; the test counts in `docs/test-inventory.md` add up (unit +13, `tests/read.rs` +1). The reviewer's `.\tools\check.ps1`: all OK, 1893 passed, 0 failed, 72 ignored, no clippy warnings. Left as it is: the step comment `// Exif (§6.3, §6.5).` before the `exif_args` loop in `write_plan.rs` does not name §6.6 (the text did not ask for it). No hand check: no user-visible change. T196 is unblocked |
