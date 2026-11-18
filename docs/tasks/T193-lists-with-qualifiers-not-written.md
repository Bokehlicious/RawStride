# T193. A list whose items have qualifiers is shown but not written (ADR 0007 §8) — Open

- **Why:** MWG §5.1 says XMP qualifiers on keywords "SHOULD be preserved on any keyword
  manipulation". The reviewer checked ExifTool 13.59 on 2026-11-18 with a sidecar whose
  `dc:subject` had one item with a qualifier and one plain item `cat` (the packet is in **Tests
  to add**):
  - reading with the reader's own options (`-j -n -b -struct -api structformat=jsonq -G1
    -XMP:all`) gives `"XMP-dc:Subject": ["San Francisco","cat"]` and an extra key
    `"XMP-dc:SubjectType": "location"` (ExifTool names a qualifier as the property's tag name
    followed by the qualifier's name);
  - writing the list as a whole, as RawStride does (`-XMP-dc:Subject=` then one
    `-XMP-dc:Subject=<item>` per item), keeps the qualified item and adds "San Francisco" again as
    a plain item: a duplicate;
  - `-XMP-dc:Subject-=San Francisco` and `-XMP-dc:Subject=` leave the qualified item.

  So RawStride cannot write such a list without damaging it. The owner decided on 2026-11-18
  (ADR 0007 revision 21, §8): such a list is shown as usual with a notice and is never written to
  XMP; a change to it is left out of the write with a write notice; the other properties are
  written.
- **Crates:** `pw-metadata`, and in `pw-loupe` only the two text functions that `match` on the
  changed enums (one task, not split: the new variants of `FileNotice` and `WriteNotice` break
  those `match`es until both are changed). The batch's line for other images is T194.
  **Files:**
  - `crates/pw-metadata/src/value.rs`
  - `crates/pw-metadata/src/read.rs`
  - `crates/pw-metadata/src/write_plan.rs`
  - `crates/pw-metadata/src/edit.rs`
  - `crates/pw-metadata/tests/read.rs`
  - `crates/pw-metadata/tests/edits.rs`
  - `crates/pw-loupe/src/info_panel.rs`
  - `crates/pw-loupe/src/editor.rs`
  - `docs/glossary.md`
  - `docs/test-inventory.md`
- **Terms used below:**
  - **A list property:** a property of `properties::properties()` with `et_xmp` set, occurrence
    `Multi` and a kind that is not `ValueKind::Struct` (the properties `write_plan.rs` writes
    through its `Value::List` branch: `-<et_xmp>=` then one argument per item). Today these are
    `dc:subject`, `dc:creator`, `Iptc4xmpCore:Scene`, `Iptc4xmpCore:SubjectCode`,
    `Iptc4xmpExt:PersonInImage`, `Iptc4xmpExt:OrganisationInImageName`,
    `Iptc4xmpExt:OrganisationInImageCode`, `Iptc4xmpExt:EventID`, `Iptc4xmpExt:ModelAge`,
    `plus:ModelReleaseID`, `plus:PropertyReleaseID` and any such property of the extra table;
    compute the set from the table, do not list it in code.
  - **A qualifier key** of a list property with ExifTool name `N` (for example `XMP-dc:Subject`):
    a key of the JSON object that starts with `N`, is longer than `N`, whose next character after
    `N` is an ASCII upper-case letter, and that is **not** the `et_xmp` of any property of
    `properties::properties()` (so `XMP-iptcExt:PersonInImageWDetails` is not a qualifier key of
    `XMP-iptcExt:PersonInImage`).
  - **Marked:** a property for which `Metadata::has_qualifiers` is true (point 1).
- **How it fits together:** the reader marks the lists (points 2–4). The write paths of
  `edit.rs` put a marked property the user changed back to the value read from the file before
  the plan is made, so the plan treats it exactly like an untouched property: IIM conversion,
  checksum and the in-line pass all see the file's own value, and the edit leaks nowhere
  (point 6). The plan itself only makes sure an untouched marked list is never written to XMP by
  the in-line pass, and that no new IIM checksum is then written (point 5): a checksum would
  make the stale XMP list win at the next read and lose the IIM value, the same reason as Q1.
- **What:**
  1. `value.rs`, `Metadata`: add a private field `qualified: BTreeSet<String>` (XMP ids), and
     - `pub fn mark_qualified(&mut self, xmp_id: &str)`;
     - `pub fn has_qualifiers(&self, xmp_id: &str) -> bool`;
     - `pub fn restore(&mut self, original: &Metadata, xmp_id: &str)`: the property gets the
       value and origin it has in `original` (or none, if `original` has none) and is no longer
       marked changed. Conflicts are not touched (nothing removes a conflict today).

     Each with a doc comment naming ADR 0007 §8. `set`, `clear` and `changes` are unchanged.
  2. `read.rs`: a function `qualified_lists(object: &Map<String, Json>) -> Vec<&'static str>`
     that returns the XMP ids of the list properties that have at least one qualifier key in
     `object`, in table order.
  3. `read.rs`, `read_metadata` (about lines 116–182): call it on the **primary** XMP's JSON
     object only: `file` for JPEG, TIFF and DNG; for a manufacturer RAW the sidecar's object,
     which today is used inline (`xmp_values(&self.read_json(&sidecar_args(&path), &path)?)`,
     about line 129), so bind it to a variable first (no sidecar: no ids); never the camera XMP
     inside a RAW. **After** `reconcile(…)`, for each id: `reconciled.metadata.mark_qualified(id)`
     and push `FileNotice::ListWithQualifiers(id.to_string())` to `file_notices`.
  4. `read.rs`, `FileNotice`: add the variant `ListWithQualifiers(String)` (the XMP id) with a
     doc comment ("a list property whose items have qualifiers RawStride cannot keep when it
     writes the list; shown, never written, ADR 0007 §8").
  5. `write_plan.rs`:
     - `WriteNotice`: add `QualifiersKept { property: String }` with a doc comment (made by
       `edit.rs`, point 6; the plan does not make it).
     - `bring_in_line`: a marked property is never pushed to `to_xmp`; it goes to a new field
       `InLine::held` instead (the XMP ids). `to_iim` stays as it is (it writes IIM, which keeps
       no qualifiers).
     - The checksum (about lines 254–263): when `in_line.held` is not empty, no
       `-IPTCDigest=new` is written and `WriteNotice::DigestNotWritten` is added, exactly as
       when `kept` is not empty (Q1). `held` gives no `ConflictKept` and no `QualifiersKept`
       notice (the user did not change it; the info panel's notice tells why).
     - Update the module comment (the **XMP** point: a marked list is never written by the
       in-line pass; a changed one is put back by `edit.rs` first, §8) and the doc comment of
       `plan` (callers put a changed marked property back first, as `edit.rs` does).
  6. `edit.rs`, in `write_edits_safely` and `apply_template_safely`: keep the read metadata
     (`read.reconciled.metadata`) as the original (a clone, before the edits are applied);
     after the edits or the template are applied, after `entries_left_alone` (which keeps its
     place and works on the edited metadata as today), and **before** `write_plan::plan`, for each changed property (`metadata.changes()`) that is
     marked, call `metadata.restore(&original, id)` and note the id. In `write_edits_safely` each
     noted id adds `WriteNotice::QualifiersKept { property }` to the returned notices **also when
     the report is `WriteReport::Unchanged`**, never twice, the same way `ConflictKept` is added
     for `left_alone` today (about lines 464–473). `apply_template_safely` returns only a
     `WriteReport` today and stays so: the template still leaves the list unwritten, but without
     a notice (the import's template path shows no plan notices today either; the info panel's
     notice of point 7 still tells the user). Update the module comment and both doc comments.
  7. `info_panel.rs`, `file_notice_text` (about line 428): the arm for
     `ListWithQualifiers(property)`:
     `"{field} has qualifiers RawStride cannot keep, so it is shown but never changed"`, with
     `field = property_label(property, Language::En)`.
  8. `editor.rs`, `notice_text` (about line 2545): the arm for `QualifiersKept { property }`:
     `"{field} was not written: it has qualifiers RawStride cannot keep"`, `field` as above.
  9. `docs/glossary.md`: add **qualifier** (an XMP qualifier on a value, for example on a
     keyword; not `xml:lang`), in the glossary's form, if it is not there.
- **Tests to add:**
  - `value.rs` unit tests: `restore_puts_back_the_read_value_and_unmarks_the_change` (a read
    value, then `set`, then `restore`: the read value and origin, `changes()` empty) and
    `restore_of_a_property_the_file_lacked_removes_it`.
  - `read.rs` unit tests:
    - `qualified_lists_finds_a_qualifier_key`: an object with `XMP-dc:Subject` (a list) and
      `XMP-dc:SubjectType` gives `["dc:subject"]`.
    - `qualified_lists_ignores_other_properties_and_languages`: an object with
      `XMP-iptcExt:PersonInImage`, `XMP-iptcExt:PersonInImageWDetails`, `XMP-dc:Title-sv` and
      `XMP-dc:Subject` without a qualifier key gives `[]`.
  - `write_plan.rs` unit tests (the `mod tests` in `src/write_plan.rs`, not
    `tests/write_plan.rs`): `a_marked_list_is_not_brought_in_line_to_xmp`: a JPEG with UTF-8
    IIM whose marked `dc:subject` came from IIM (rule `Iim` conflict, unchanged) and another
    property changed: no `XMP-dc:Subject` argument, no `-IPTCDigest=new`, one
    `DigestNotWritten` and no `ConflictKept`, and the other property's arguments are there.
    And `a_marked_list_whose_xmp_won_still_brings_iim_in_line`: rule `Xmp`, unchanged: the IIM
    `Keywords` arguments from the XMP list are there and the checksum is written.
  - `tests/read.rs` (real ExifTool): `a_sidecar_keyword_with_a_qualifier_is_marked`: an ARW
    made as in `a_raw_file_reads_its_sidecar_and_ignores_a_sidecar_with_the_extension`, with a
    sidecar `TN0001.xmp` written with `std::fs::write` (the `write` helper cannot make a
    qualifier) holding this packet:

    ```xml
    <?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>
    <x:xmpmeta xmlns:x='adobe:ns:meta/'>
    <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>
     <rdf:Description rdf:about='' xmlns:dc='http://purl.org/dc/elements/1.1/' xmlns:ex='http://example.com/ns/'>
      <dc:subject><rdf:Bag>
       <rdf:li rdf:parseType='Resource'><rdf:value>San Francisco</rdf:value><ex:type>location</ex:type></rdf:li>
       <rdf:li>cat</rdf:li>
      </rdf:Bag></dc:subject>
     </rdf:Description>
    </rdf:RDF>
    </x:xmpmeta>
    <?xpacket end='w'?>
    ```

    It reads `dc:subject` as `["San Francisco", "cat"]`, `has_qualifiers("dc:subject")` is true
    and `file_notices` holds `ListWithQualifiers("dc:subject")`; the same packet without the
    qualifier (`<rdf:li>San Francisco</rdf:li>`) gives neither.
    And `an_embedded_keyword_with_a_qualifier_is_marked`: the small JPEG of `jpeg(..)` given
    the same packet as its embedded XMP (`write` with `-xmp<=<path of the packet file>`, which
    copies the packet as it is; check it reads back with the qualifier) is marked the same way.
  - `tests/edits.rs` (real ExifTool, safe write), with that sidecar next to a fake ARW (or the
    same `dc:subject` in a JPEG if a RAW cannot be written there; say which in the log):
    - `a_keyword_edit_of_a_qualified_list_is_not_written_and_says_so`: `write_edits_safely`
      with only a `dc:subject` edit gives `WriteReport::Unchanged` and the notices
      `[QualifiersKept { property: "dc:subject" }]`; the sidecar's bytes are unchanged.
    - `the_other_edits_are_written_beside_a_qualified_list`: a `dc:subject` edit and a
      Headline edit: Headline is written, `dc:subject` reads back as before (no duplicate "San
      Francisco"), the notices hold `QualifiersKept` once.
  - `info_panel.rs` and `editor.rs`: one test each for the new text, as the tests of the
    existing arms do.
- **Accepted (owner's rule §8, follows from it):** a marked list with an `Open` IIM/XMP
  conflict cannot be decided in RawStride (deciding it would mean writing the list), so such a
  file keeps getting no new checksum, as for any unresolved conflict (Q1); the notices then show
  both `ConflictKept` (if the edit left it alone) and `QualifiersKept`.
- **Not in this task:** the batch's line for other images (T194); the batch's "Written" count
  that also counts an image whose write was `Unchanged` (as today); recent values that also
  record a keyword that was put back (`main.rs` `recorded`); a warning in the editor before OK
  (backlog); a notice from the template
  path (`apply_template_safely` returns no notices today); editing a list with its qualifiers
  kept (backlog; needs RawStride to edit the XMP packet itself); qualifiers on items of lists of
  structures, on Lang Alt entries or on single values; the editor's fields (the list stays
  editable there; the write notice tells the user); the Tauri app (D7).
- **Rules:** `docs/tasks/README.md`, its point 13 for the two texts (English, in the named text
  functions, one `format!` template each). No new dependency. This task changes the safe-write
  path's caller (`edit.rs`), so it gets an extra code review (README, "An extra code review").
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  and this task's file.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-18 | Open | Added (ADR 0007 revision 21, §8; owner's approval of K1 the same day) |
| 2026-11-18 | Open | Cold read: a fresh agent found that dropping the change inside `plan` leaks the edit into IIM through `to_iim` or skips the IIM conversion, that `write_edits_safely` drops the plan's notices when nothing is written (`edit.rs` ~465) so the notice would be lost, that the template path returns no notices, that the batch lists only `ConflictKept` for other images, that the RAW sidecar object is not bound in `read_metadata`, and that the test sidecar needs `std::fs::write`. The reviewer rewrote the design: `edit.rs` puts a changed marked property back to its read value before the plan (`Metadata::restore`), the plan only keeps the in-line pass from writing it to XMP, the notice is added in `edit.rs` like `ConflictKept`; `edit.rs` and `tests/edits.rs` added to the files; the template path's missing notice is accepted (noted under **Not in this task**); the batch line split out as T194; the packet and the reader's options checked with ExifTool 13.59 |
| 2026-11-18 | Open | Chain review (T193 → T194): a fresh agent found that skipping a marked list in `to_xmp` while still writing a new checksum would let the stale XMP list win at the next read and lose the IIM value (gap A), that a marked list with an `Open` conflict can never be decided (gap B), and that the order of `entries_left_alone` and the restore was not pinned; also that only a RAW sidecar was tested against real ExifTool, and that between T193 and T194 another image's `QualifiersKept` in a batch is dropped silently (only misleading, no damage; T194 fixes it). Fixed: point 5 `InLine::held` with no checksum and `DigestNotWritten` (Q1), point 6 the order, gap B written as accepted, a JPEG reader test and a rule-`Xmp` plan test added; the template path's silent drop and an editor warning before OK put in the backlog |
