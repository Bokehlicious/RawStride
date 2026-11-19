# T205. Rating percentages and a flat `lr:hierarchicalSubject` kept equal (ADR 0007 Q15, Q16) — Blocked: waits for T204

- **Why:** ADR 0007 revision 26 (owner 2026-11-19), from the test with DxO PhotoLab (ADR 0029
  step P0, §1.1):
  - **Q16, §6.1, §6.3:** DxO PhotoLab adds Exif `IFD0:Rating` and `IFD0:RatingPercent` and
    `xmp:RatingPercent` when it writes a DNG, and `xmp:RatingPercent` when it writes a sidecar
    (`D:\PhotoWorkflowData\P0-dxo\B\TN005956.xmp`: Rating 3, RatingPercent 50). A rating that
    RawStride writes leaves them with the old value today. Existing ones are written to match;
    a reject removes them; none is created or read.
  - **Q15, §8:** DxO PhotoLab writes the keywords into `lr:hierarchicalSubject` as flat entries
    and reads them back from there, so a keyword renamed in RawStride came back
    (`…\B\TN005960.xmp`: `dc:subject` `p0test6`, `lr:hierarchicalSubject` `p0test4`). An
    `lr:hierarchicalSubject` whose every entry is flat is written equal to `dc:subject` whenever
    RawStride writes the keywords.

  Second task of ADR 0029 step P0b (T204 → T205). It does not depend on T204's code, but both
  change `tests/safe_write.rs` and `tests/edit.rs`, so it starts when T204 is Done.
- **Crates:** `pw-metadata`; one line per test literal in `pw-loupe` and `pw-imagecheck`
  (point 7). **Files:**
  - `crates/pw-metadata/src/write_plan.rs`
  - `crates/pw-metadata/src/read.rs`
  - `crates/pw-metadata/tests/read.rs`
  - `crates/pw-metadata/tests/write_plan.rs`, `tests/safe_write.rs`, `tests/verify.rs` (test
    literals only, point 7)
  - `crates/pw-metadata/tests/edit.rs`
  - `crates/pw-metadata/tests/edits.rs`
  - `crates/pw-imagecheck/tests/real_files.rs`, `crates/pw-loupe/src/editor.rs`,
    `crates/pw-loupe/src/info_panel.rs`, `crates/pw-loupe/src/main.rs`,
    `crates/pw-loupe/src/metadata.rs` (test literals only, point 7)
  - `docs/test-inventory.md`

  This is more than README's eight files: the owner made P0b two tasks, not three (2026-11-19);
  seven of the files get only the one-line literal change of point 7.
- **What:**
  1. `write_plan.rs`, `ExifPresent`: two new fields, `rating: bool` (`IFD0:Rating`, 0x4746) and
     `rating_percent: bool` (`IFD0:RatingPercent`, 0x4749). Its doc comment becomes "Which Exif
     fields the plan may update exist in the file (§6.3: only these are written)", since Rating is
     "XMP → Exif only", not "both ways" (§6.1).
  2. `write_plan.rs`, a new `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct
     XmpPresent` with `rating_percent: bool` (the primary XMP has `xmp:RatingPercent`, ExifTool
     `XMP-xmp:RatingPercent`) and `flat_hierarchical_subject: bool` (the primary XMP has
     `lr:hierarchicalSubject`, ExifTool `XMP-lr:HierarchicalSubject`, with at least one entry, no
     entry holding `|`, and no qualifier key; see point 5), and a new field
     `pub xmp_present: XmpPresent` in `FileState`, after `exif_present`. Neither property is in
     the property table and neither is added: RawStride never shows or reads them as values
     (§6.3: "none is read as a source"; §8).
  3. `write_plan.rs`, `plan`, **the rating** (§6.3, Q16). When `xmp:Rating` is among `changes`:
     - its Exif and percentage values come from the new value: `Some(Value::Number(text))` that
       parses as `f64` is rounded with `f64::round` and clamped to −1…5 (as shown, §8); 0–5 give
       Rating 0–5 and RatingPercent 0, 1, 25, 50, 75, 99; −1 (a reject), a value that is not a
       number, and `None` (the rating cleared, §8 "Clearing a field") remove all three;
     - **XMP part** (every kind, so also a manufacturer RAW's sidecar; owner 2026-11-19): when
       `state.xmp_present.rating_percent`, `-XMP-xmp:RatingPercent=<percent>` or
       `-XMP-xmp:RatingPercent=`, placed right after the XMP arguments of the changes (before
       those of the in-line pass);
     - **Exif part** (inside the existing `if state.kind != FileKind::Raw`, after the other Exif
       arguments): `-IFD0:Rating=<n>` when `exif_present.rating`, `-IFD0:RatingPercent=<percent>`
       when `exif_present.rating_percent`, each `-<tag>=` for a removal;
     - nothing is created: a flag that is false gives no argument. A plan without an
       `xmp:Rating` change gives none of these arguments, whatever the flags.
  4. `write_plan.rs`, `plan`, **the keywords** (§8, Q15). When the plan writes `dc:subject` to
     XMP, as a change (set or cleared) or from the in-line pass (`in_line.to_xmp`), and
     `state.xmp_present.flat_hierarchical_subject`: `lr:hierarchicalSubject` gets the same
     arguments, made with `xmp_args("XMP-lr:HierarchicalSubject", <dc:subject's kind>, value,
     ..)` for a value and `-XMP-lr:HierarchicalSubject=` for a clear, placed right after
     `dc:subject`'s own arguments. A `dc:subject` with qualifiers is never written (§8; `edit.rs`
     puts it back, `bring_in_line` holds it), so it gives nothing here either. Without the flag
     nothing is written to `lr:hierarchicalSubject` (absent: never created; an entry with `|`:
     the whole property kept, §8).
  5. `read.rs`:
     - `IMAGE_TAGS`: add `-IFD0:Rating` and `-IFD0:RatingPercent`.
     - `file_state` gets the two Exif flags from the file's JSON object: `true` when the key
       `IFD0:Rating` (`IFD0:RatingPercent`) is present, whatever its value (a stale 0 is still
       updated). Read them directly from the object, not through `ExifFields` (that struct feeds
       the reconciliation, which never reads a rating from Exif). A manufacturer RAW keeps
       `ExifPresent::default()`.
     - a new function `xmp_present(object) -> XmpPresent`, called with the **primary** XMP object
       only: the sidecar's for a manufacturer RAW (`XmpPresent::default()` without a sidecar),
       the file's for a DNG, JPEG or TIFF; never the camera XMP inside a RAW. `rating_percent`:
       the key `XMP-xmp:RatingPercent` is present. `flat_hierarchical_subject`: the key
       `XMP-lr:HierarchicalSubject` holds a JSON array, or one string, with at least one entry,
       no entry contains `|`, and no other key starts with `XMP-lr:HierarchicalSubject` followed
       by an ASCII upper-case letter (a qualifier, the rule of `qualified_lists`). Compute it in
       `read_metadata` beside `qualified_lists` (same object, same `match kind`) and pass it to
       `file_state`, whose signature becomes `file_state(kind, iim, exif, file: &Map<String,
       Json>, xmp_present: XmpPresent)` (`file` is the image's JSON object, for the two Exif
       flags; a manufacturer RAW ignores it). With `-struct` ExifTool gives one entry as an
       array (`["a"]`, measured in the cold read); a lone string is accepted too.
     - the module comment's last paragraph (`MetadataRead::state`) names the new flags: "which
       Exif fields the plan may update exist (the "both ways" fields and the Microsoft rating
       fields), and from the primary XMP whether it has `xmp:RatingPercent` and a flat
       `lr:hierarchicalSubject`".
  6. `write_plan.rs`, the module comment: the **XMP** point gets "an existing
     `xmp:RatingPercent` follows a rating change and an existing flat `lr:hierarchicalSubject`
     follows the keywords (§6.3 Q16, §8 Q15)"; the **Exif** point gets "Rating and RatingPercent
     follow a rating change when they exist, and are removed for a reject (§6.3 Q16)".
  7. **Test literals:** every `FileState { … }` that names all its fields gets
     `xmp_present: XmpPresent::default(),` (and the import of `XmpPresent` where needed); every
     `ExifPresent { … }` that names all its fields gets `rating: false, rating_percent: false`.
     This includes `all_exif()` in `write_plan.rs`'s tests: `differs()` uses it and the tests
     around line 1300 change the rating, so `true` there would change their expected arguments;
     its doc comment (if any) says "all the 'both ways' fields". The new tests of point 3 build
     their own `ExifPresent`. Literals with `..` need nothing. Known places
     (`grep -rn "FileState {\|ExifPresent {" crates`, checked in the cold read): `write_plan.rs`
     tests (`jpeg` ~706, `all_exif` ~727, the literal in `a_raw_file_gets_xmp_arguments_only`
     ~1758), `tests/write_plan.rs` (`state` ~241; ~567 and ~663 use `..` and need nothing), `tests/safe_write.rs`
     (`bare_state`), `tests/verify.rs` (`bare_state`), `tests/read.rs` (~line 761, see point 8),
     `pw-imagecheck/tests/real_files.rs` (~127), `pw-loupe/src/editor.rs` (~3044),
     `info_panel.rs` (~529), `main.rs` (~9387, ~9463, ~9853, ~9933), `metadata.rs` (~2231). No
     other change in those files.
  8. `tests/read.rs` (~line 758): the expected `ExifPresent` of that test gets `rating: false,
     rating_percent: false` (its file has neither).
- **Tests to add:**
  - `write_plan.rs` (unit, beside `exif_fields_are_written_only_when_present`):
    - `a_rating_updates_the_existing_percentages_and_exif_rating`: Rating 3 on a JPEG with all
      three flags gives `-XMP-xmp:Rating=3`, `-XMP-xmp:RatingPercent=50`, `-IFD0:Rating=3`,
      `-IFD0:RatingPercent=50`, in that order; on `jpeg()` (no flags) only `-XMP-xmp:Rating=3`;
    - `the_percentages_of_every_star`: 0, 1, 2, 3, 4, 5 give 0, 1, 25, 50, 75, 99; `2.5` gives
      Rating 3 and 50 (rounded as shown);
    - `a_reject_or_a_cleared_rating_removes_the_percentages_and_exif_rating`: −1 and `None` give
      `-XMP-xmp:RatingPercent=`, `-IFD0:Rating=`, `-IFD0:RatingPercent=`;
    - `a_raw_sidecar_gets_only_the_xmp_percentage`: `FileKind::Raw` with all flags gives
      `-XMP-xmp:RatingPercent=50` and no `IFD0:` argument;
    - `no_rating_change_writes_no_percentage`: a City change on a file with all flags gives no
      rating argument;
    - `a_flat_hierarchical_subject_follows_the_keywords`: keywords set to `["a", "b"]` with the
      flag give `-XMP-dc:Subject=`, `-XMP-dc:Subject=a`, `-XMP-dc:Subject=b`,
      `-XMP-lr:HierarchicalSubject=`, `-XMP-lr:HierarchicalSubject=a`,
      `-XMP-lr:HierarchicalSubject=b`; without the flag only the `dc:subject` arguments; cleared
      keywords with the flag give `-XMP-dc:Subject=` and `-XMP-lr:HierarchicalSubject=`;
    - `keywords_brought_in_line_from_iim_also_go_to_a_flat_hierarchical_subject`: an unchanged
      `dc:subject` whose IIM value won (rule `Iim`, as the in-line tests do) on a JPEG with IIM
      and the flag also writes `lr:hierarchicalSubject`.
  - `read.rs` (unit): `xmp_present_finds_a_rating_percentage_and_a_flat_hierarchy` on JSON
    objects: `{"XMP-xmp:RatingPercent": "50"}` → `rating_percent`; `["a", "b"]` and `"a"` → flat;
    `["a", "x|y"]` → not flat; `[]` → not flat; a key `XMP-lr:HierarchicalSubjectLang` → not
    flat; absent → both false.
  - `tests/read.rs`: `the_state_names_the_rating_fields_and_a_flat_hierarchy`: a JPEG written
    with `-IFD0:Rating=5 -IFD0:RatingPercent=99 -XMP-xmp:RatingPercent=99
    -XMP-lr:HierarchicalSubject=a` reads all four flags true; a fake RAW (`TN0001.ARW`) with a
    sidecar holding `XMP-xmp:RatingPercent` reads `xmp_present.rating_percent` true and
    `ExifPresent::default()`.
  - `tests/edit.rs` (through `write_rating_safely`):
    - `a_rating_brings_existing_percentages_and_exif_rating_along`: a JPEG set up with
      `-IFD0:Rating=5 -IFD0:RatingPercent=99 -XMP-xmp:Rating=5 -XMP-xmp:RatingPercent=99`; 2 stars
      → `IFD0:Rating` 2, `IFD0:RatingPercent` 25, `XMP-xmp:RatingPercent` 25; then a reject → the
      three gone and `xmp:Rating` −1; `assert_clean`;
    - `a_rating_creates_no_percentage`: the plain JPEG, 4 stars → none of the three exists;
    - `a_raw_sidecar_percentage_follows_the_rating`: a fake RAW whose sidecar holds
      `XMP-xmp:Rating=3 XMP-xmp:RatingPercent=50`; 5 stars → 99 in the sidecar; the RAW's bytes
      unchanged.
  - `tests/edits.rs` (through `write_edits_safely` with an Entries edit of `dc:subject`):
    - `a_flat_hierarchical_subject_follows_a_renamed_keyword`: a JPEG with `dc:subject` and
      `lr:hierarchicalSubject` both `["p0test4", "sjö"]`; remove `p0test4`, add `p0test6` →
      both read back as `["sjö", "p0test6"]` (read `lr:hierarchicalSubject` with the file's
      `tag` helper or `execute`);
    - `a_nested_hierarchical_subject_is_kept`: the same with `lr:hierarchicalSubject`
      `["natur|fåglar|fiskgjuse"]` → `lr:hierarchicalSubject` unchanged, `dc:subject` changed;
    - `no_hierarchical_subject_is_created`: a JPEG with keywords and no `lr:hierarchicalSubject`
      → still none after the edit.
- **Not in this task:**
  - `XMP-microsoft:RatingPercent` and `XMP-microsoft:Rating` (Windows Explorer's): not in
    ADR 0007 §6.3; left unchanged. Noted for the backlog when the chain is Done.
  - The Tauri path (`write_rating`, `write_iptc`, `write_tags`, D7): unchanged.
  - Reading any of these fields as a rating or as keywords (§6.3: never read as a source).
  - The modification time: T204.
  - The documents: the reviewer changes ADR 0007 §18, ADR 0029 §5 (P0b) and the backlog when the
    chain is Done.
- **Rules:** `AGENTS.md`; this README; `/code-review` or a fresh review agent on the diff before
  the owner is asked to approve (it changes what the safe write writes, and more than one crate).
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated (the new tests
  in the rows of `pw-metadata`'s unit tests, `tests/read.rs`, `tests/edit.rs` and
  `tests/edits.rs`, with their counts; the header's totals and its "after …" range include T205), and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (ADR 0029 step P0b split into T204 → T205 by the owner, 2026-11-19); waits for T204. Cold read: verification accepts the new arguments (also the Exif block dropped on a reject, measured); one entry comes as an array; `file_state`'s new signature and the literal list made exact |
