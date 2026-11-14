# T134. ADR 0007 §12: hierarchical keywords and deprecated IIM datasets kept after a write — Done

- **Why:** two of the tests §12 of ADR 0007 asks for have none (found by T133; the table "Where
  each test is" in §12 says "not covered"): hierarchical keywords kept unchanged (§8, MWG §5.10
  "MUST preserve all nested information") and the deprecated IIM datasets kept in a written file
  (§5.5). The owner has no image with hierarchical keywords, so the tests build their own files
  with ExifTool, as the other tests of `tests/edits.rs` do.
- **Crates:** `pw-metadata` only. **Files:** `crates/pw-metadata/tests/edits.rs` (new tests, using
  its existing helpers `Scratch`, `write`, `tag`, `read_value`, `assert_edit`);
  `docs/test-inventory.md` (the `tests/edits.rs` row and the totals); `docs/adr/0007-metadata-iptc.md`
  (only the row "Hierarchical keywords, unknown namespaces, other languages and deprecated IIM
  datasets kept" of the §12 table "Where each test is": the two "not covered" parts name the new
  tests). Nothing in `src/` changes: **if a test shows that `pw-metadata` loses or changes a value,
  stop and report it** (rule 5) with the before and after values, instead of changing the code or
  the test.
- **What:** add these tests to `tests/edits.rs`. Set-up data is written with the file's `write`
  helper (the `ExifTool` type, never a command line: arguments are UTF-8 only through it), and
  "before" and "after" are compared on ExifTool's own output, not on `read_metadata`, so a change
  RawStride does not model is still seen.
  1. **Hierarchical keywords in a JPEG, keywords edited.** On the small JPEG write:
     `-XMP-dc:Subject=Göteborg`, `-XMP-dc:Subject=Fåglar`,
     `-XMP-lr:HierarchicalSubject=Platser|Sverige|Göteborg`,
     `-XMP-lr:HierarchicalSubject=Djur|Fåglar`, and an `mwg-kw:Keywords` structure with two levels
     written as one ExifTool structure argument
     (`-XMP-mwg-kw:KeywordInfo={Hierarchy=[{Keyword=Platser,Children=[{Keyword=Sverige,Children=[{Keyword=Göteborg}]}]}]}`;
     ExifTool names the `mwg-kw:Keywords` structure `KeywordInfo`, corrected by the reviewer on
     2026-11-14 after the agent's question). If ExifTool refuses that structure, stop and report
     the message. Take "before" as
     `-j -struct -XMP-lr:all -XMP-mwg-kw:all` of the file. Then edit `dc:subject` to the list
     `Göteborg`, `Fåglar`, `Hamn` with `s.edit(…)` and expect it written. Check: `dc:subject` reads
     back as the three keywords, and the same `-j -struct` output (less the `SourceFile` key) is
     equal to "before".
  2. **Hierarchical keywords in a sidecar, another field edited.** On the fake RAW of `Scratch`,
     first create its sidecar `TN0001.xmp` with `write` holding the two `lr:HierarchicalSubject`
     entries and `dc:subject` as in point 1. Edit `dc:title` on the RAW (as
     `a_title_on_a_raw_writes_only_the_sidecar` does). Check: written to the sidecar, the title
     reads back, the sidecar's `-j -struct -XMP-lr:all` output (less `SourceFile`) equals "before",
     and the RAW's bytes are unchanged.
  3. **Deprecated IIM datasets in a JPEG, another field edited.** On the small JPEG write both
     forms equal: `-IPTC:CodedCharacterSet=UTF8`, `-IPTC:Urgency=5`, `-IPTC:Category=SPO`,
     `-IPTC:SupplementalCategories=Fotboll`, `-IPTC:SupplementalCategories=Allsvenskan`,
     `-XMP-photoshop:Urgency=5`, `-XMP-photoshop:Category=SPO`,
     `-XMP-photoshop:SupplementalCategories=Fotboll`,
     `-XMP-photoshop:SupplementalCategories=Allsvenskan`, and `-IPTCDigest=new`; check as a
     precondition that `read_metadata` gives the digest `IimDigest::Matches`. Take "before" as
     `-j -IPTC:Urgency -IPTC:Category -IPTC:SupplementalCategories -XMP-photoshop:Urgency
     -XMP-photoshop:Category -XMP-photoshop:SupplementalCategories`. Edit `photoshop:Headline`
     and expect it written (the file has IIM, so the plan writes IIM too). Check: the headline is
     in both `XMP-photoshop:Headline` and `IPTC:Headline`; the same output (less `SourceFile`)
     equals "before"; the digest is still `Matches`.
- **Tests to add:** the three above, each with a doc comment saying what it checks and that it
  covers ADR 0007 §12 / §8 or §5.5.
- **Not in this task:** editing hierarchical keywords (ADR 0007 §15 "not part of this ADR"; the
  keyword panel of ADR 0017), synchronising the deprecated datasets between the forms when only one
  form has them, and any change in `src/`.
- **Rules:** rules 1–12 of `docs/tasks/README.md`; no new dependencies.
- **Done when:** the three tests pass in `.\tools\check.ps1`, it is all OK, and `git diff --stat`
  shows only the three files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Added after T133 (the two "not covered" rows of ADR 0007 §12) |
| 2026-11-14 | Ready for review | Three tests in `tests/edits.rs`, all pass; point 3 compares with `-G1 -a` so the IIM and XMP tags of the same name are both seen; `check.ps1` all OK (1328 passed, 53 ignored) |
| 2026-11-14 | Done | Reviewed, no changes; owner approved. check.ps1 all OK (1328 passed, 53 ignored) |
