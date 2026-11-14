# T128. ADR 0017 U10d and ADR 0007 I11: the Licence (PLUS) section with fixed choice lists — Open

- **Can start now.** The owner approved the proposals of U10d-Q1–U10d-Q6 (at the end of this task)
  on 2026-11-14; they are written into the points below, into ADR 0017 §6.1 and ADR 0007 §11 (Q9).
  R2 is done: the Digital Source Type vocabulary is saved in `D:\PhotoWorkflowData\references\`
  (`IPTC-NewsCodes-digitalsourcetype.json`, described in that folder's `README.md`; the codes are
  also listed in point 1, so the task can be done without the file). It may run beside T127: T127 changes only `crates/pw-metadata/tests/level_c.rs`
  and the `pw-metadata` rows of `docs/test-inventory.md`; the one finished second brings the total
  up to date. Not at the same time as another task that changes `main.rs` or `editor.rs`.
- **Changed 2026-11-14 after the review of T127 (owner approved):** the PLUS fields hold **codes**,
  not full URIs (points 1, 3, 4 and the tests). ExifTool removes the prefix
  `http://ns.useplus.org/ldf/vocab/` when it reads a PLUS value (also with `-n`) and adds it when
  it writes one; a full URI given to ExifTool is stored with the prefix twice (the reviewer tried
  it on a copy). Digital Source Type is not converted by ExifTool and keeps its full URI. Work
  already done for the section, the fields, the settings and the labels stays as it is.
- **Crates:** `pw-metadata`, `pw-loupe`. **Files:** `crates/pw-metadata/src/vocabulary.rs` (new);
  `crates/pw-metadata/src/lib.rs` (the module, one `pub mod` line);
  `crates/pw-metadata/src/validate.rs` (point 4); `crates/pw-metadata/tests/edits.rs` (point 8);
  `crates/pw-metadata/src/write_plan.rs` (`xmp_args`, point 9; added 2026-11-14, owner approved); `crates/pw-loupe/src/editor.rs`;
  `crates/pw-loupe/src/main.rs` (`draw_window_form`, `window_editor_properties`);
  `crates/pw-loupe/src/settings.rs` (the new section's state); `docs/glossary.md`;
  `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.3.1 section 5 and step U10, merged with milestone 8's I11 (ADR 0007 §11,
  Q9): the PLUS release fields and Digital Source Type are shown today only read-only under Other
  Metadata. A photo desk sets them for every model or property shot, and their values must be
  exact URIs, so they are edited as fixed choice lists, never as free text. The PLUS vocabulary
  was read on 2026-11-14 (R1, `D:\PhotoWorkflowData\references\`). The PLUS structures Image
  Creator, Copyright Owner, Licensor and Image Supplier are lists of structures and come in U11a.
- **What:**
  1. **The vocabulary** (`pw-metadata`, new module `vocabulary.rs`; ADR 0007 §11: "the allowed
     values are taken from the PLUS controlled vocabulary into the mapping table"): for each
     choice-list property its XMP id and its allowed values in the vocabulary's order, as constants
     written by hand from the saved files, with a comment naming the file and its SHA-256:
     - `plus:ModelReleaseStatus`: `MR-NON`, `MR-NAP`, `MR-UMR`, `MR-LMR`;
     - `plus:PropertyReleaseStatus`: `PR-NON`, `PR-NAP`, `PR-UPR`, `PR-LPR`;
     - `plus:MinorModelAgeDisclosure`: `AG-UNK`, `AG-A25`, `AG-A24` … `AG-A15`, `AG-U14`
       (13 values);
     - `plus:DataMining`: the nine `DMI-` codes of the references folder's `README.md`, in its
       order;
     - `Iptc4xmpExt:DigitalSourceType` (R2, U10d-Q2), the 17 current codes in the file's order:
       `digitalCapture`, `computationalCapture`, `negativeFilm`, `positiveFilm`, `print`,
       `humanEdits`, `compositeWithTrainedAlgorithmicMedia`, `algorithmicallyEnhanced`,
       `digitalCreation`, `dataDrivenMedia`, `trainedAlgorithmicMedia`, `algorithmicMedia`,
       `screenCapture`, `virtualRecording`, `composite`, `compositeCapture`,
       `compositeSynthetic`; and, separately, the three **retired** codes `minorHumanEdits`,
       `softwareImage`, `digitalArt` (point 3).

     **The values are the form RawStride reads and writes through ExifTool:**
     - for the four PLUS properties the **code alone** (`MR-NAP`, `AG-A25`, `DMI-UNSPECIFIED` …).
       The file holds `http://ns.useplus.org/ldf/vocab/` followed by the code, but ExifTool
       removes that prefix when it reads and adds it when it writes, so the code is what
       `read_metadata` gives and what an edit must hold; an edit with the full URI would store the
       prefix twice;
     - for Digital Source Type the **full URI**,
       `http://cv.iptc.org/newscodes/digitalsourcetype/` followed by the code (ExifTool does not
       convert it).

     The module also gives each property's full URI of a value (for the tooltip, point 3) from one
     constant prefix per vocabulary. It holds no texts for the user (rule 13); a function gives the
     list of a property and one tells whether a value is in it (exact comparison, no case folding,
     no trimming).
  2. **The fields** (`EditorField`, `EditorField::ALL`), a new section **Licence (PLUS)**
     (`EditorSection::Licence`, U10-Q1) between Assignment and Rating and Label, in this order:
     Model Release Status; Model Release Id (`plus:ModelReleaseID`, list); Minor Model Age
     Disclosure (U10d-Q1); Property Release Status; Property Release Id
     (`plus:PropertyReleaseID`, list); Data Mining (U10d-Q1); Digital Source Type. The two Id
     fields are one line with their entries separated by commas, as Keywords (T108, U10-Q2).
  3. **A choice list** (U10d-Q3) for each field of point 1: a drop-down whose entries are
     the empty entry (no value) and the vocabulary's values in its order, each shown by its
     English label (point 5); the file's value selected. Choosing an entry is a `Set` of its value
     as point 1 gives it (`Value::Text`; the code for PLUS, the full URI for Digital Source Type);
     choosing the empty entry is a `Clear`; an untouched list gives no edit.
     **A value not in the vocabulary** (ADR 0007 §11: "kept and shown with a warning, never
     refused or removed"): the list shows one more entry, selected, with the value's text
     (U10d-Q4: "Unknown value: {value}", with the value as read; the warning of point 4 below the form); left as it is it
     gives no edit; it is only replaced when the user picks another entry. **A retired Digital
     Source Type code** in a file is still a valid value: it is shown as an extra entry with its
     own label, selected, with no warning, and kept the same way; the retired codes are not
     offered otherwise. Each entry's tooltip is its full URI (U10d-Q3; for PLUS the prefix and the
     code, from point 1). Edits, conflicts, warnings, copy and paste, and "Apply to All Marked" work for the new
     fields as for the others (`from_read`, `choose`, `edits`, `warnings`, `CopiedFields`,
     `BatchEdits`; U10d-Q5). None of the new fields is a copyright-management field (§8).
  4. **The warning** (`validate.rs`): a new `Warning::VocabularyValueUnknown { property, value }`
     for a PLUS choice-list property whose value is not one of its codes (point 1); `Display` in English
     as the other variants; the module comment's "Left out here" list no longer names the PLUS
     properties. Digital Source Type keeps its present warning
     (`DigitalSourceTypeUnknown`, unchanged).
  5. **Texts** (rule 13): the section heading from `editor_text` (`EditorText`); the field names
     from `property_label`; each choice's label from one function in `editor.rs`, taking the value
     of point 1 and giving the vocabulary's own English label (U10d-Q3), exactly as listed here:
     - `MR-NON` None, `MR-NAP` Not Applicable, `MR-UMR` Unlimited Model Releases, `MR-LMR`
       Limited or Incomplete Model Releases; `PR-NON` None, `PR-NAP` Not Applicable, `PR-UPR`
       Unlimited Property Releases, `PR-LPR` Limited or Incomplete Property Releases;
     - `AG-UNK` Age Unknown, `AG-A25` Age 25 or Over, `AG-A24` … `AG-A15` Age 24 … Age 15,
       `AG-U14` Age 14 or Under;
     - `DMI-UNSPECIFIED` Unspecified - no prohibition defined, `DMI-ALLOWED` Allowed,
       `DMI-PROHIBITED-AIMLTRAINING` Prohibited for AI/ML training,
       `DMI-PROHIBITED-GENAIMLTRAINING` Prohibited for Generative AI/ML training,
       `DMI-PROHIBITED-EXCEPTSEARCHENGINEINDEXING` Prohibited except for search engine indexing,
       `DMI-PROHIBITED` Prohibited, `DMI-PROHIBITED-SEECONSTRAINT` Prohibited, see Other
       Constraints property, `DMI-PROHIBITED-SEEEMBEDDEDRIGHTSEXPR` Prohibited, see Embedded
       Encoded Rights Expression property, `DMI-PROHIBITED-SEELINKEDRIGHTSEXPR` Prohibited, see
       Linked Encoded Rights Expression property;
     - `digitalCapture` Digital capture sampled from real life, `computationalCapture`
       Multi-frame computational capture sampled from real life, `negativeFilm` Digitised from a
       transparent negative, `positiveFilm` Digitised from a transparent positive, `print`
       Digitised from a non-transparent medium, `humanEdits` Human-edited media,
       `compositeWithTrainedAlgorithmicMedia` Edited using Generative AI,
       `algorithmicallyEnhanced` Algorithmically-altered media, `digitalCreation` Digital
       creation, `dataDrivenMedia` Data-driven media, `trainedAlgorithmicMedia` Created using
       Generative AI, `algorithmicMedia` Pure algorithmic media, `screenCapture` Screen capture,
       `virtualRecording` Virtual event recording, `composite` Composite of elements,
       `compositeCapture` Composite of captured elements, `compositeSynthetic` Composite
       including generative AI elements; retired: `minorHumanEdits` Original media with minor
       human edits, `softwareImage` Created by software, `digitalArt` Digital art.

     The shown entry for an unknown value and the warning text of point 4 from functions in
     `editor.rs`. The new section name and the choice labels go into `docs/glossary.md` and are
     named in the report.
  6. **The section** is collapsible and its state remembered in `settings.txt` as the other
     sections (U10-Q6, T123), open at first; a missing key gives open.
  7. **Other Metadata** no longer lists the properties that are now fields.
  8. **The write through ExifTool** (`tests/edits.rs`, against a real ExifTool as the other tests
     there): an edit `Set` `plus:ModelReleaseStatus` = `MR-UMR` on the small JPEG; ExifTool's
     XMP of the file (`-b -XMP`) then holds `http://ns.useplus.org/ldf/vocab/MR-UMR` exactly once
     and the prefix never twice, and `read_metadata` gives back `MR-UMR`. The same for Digital
     Source Type with the full URI of `digitalCapture` (stored once, read back unchanged).
  9. **URI values written without ExifTool's print conversion** (added 2026-11-14 after the
     review, owner approved). The reader asks ExifTool for values without print conversion
     (`-n`), but the write plan gave XMP values *with* it, so ExifTool expected the label
     ("Unlimited Model Releases") and refused the code: "Can't convert XMP-plus:ModelReleaseStatus
     (not in PrintConv)", seen in point 8's test. In `xmp_args`, a property whose kind in the
     property table is `ValueKind::Uri` is written with `#=` instead of `=` (`-<tag>#=<value>`),
     for a single value and for each entry of a list; this turns off only the print conversion,
     so ExifTool still adds the PLUS prefix once (the reviewer checked it on a copy). Clearing
     (`-<tag>=`), the other kinds, the IIM arguments and the structures' fields are unchanged.
     A unit test in `write_plan.rs` checks the arguments for a PLUS code, a Digital Source Type
     URI, Web Statement of Rights and a list of Event Ids (`#=` each), and that a Text property
     still uses `=`; point 8's test then passes.
- **Not in this task:** Image Creator, Copyright Owner, Licensor, Image Supplier and the other
  structures (U11a); Model Age (`Iptc4xmpExt:ModelAge`, stays under Other Metadata); the other
  PLUS vocabularies of the LDF file (Credit Line Required, Adult Content Warning, Image Type …);
  choosing and ordering the fields (U10b); recent values (U12a); any change in the write path or
  in `pw-metadata` beyond points 1, 4 and 9.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add / change:**
  - `pw-metadata`: each list of point 1 has the expected count, starts and ends with the expected
    values and has no duplicate; the PLUS lists hold codes without a prefix, the Digital Source
    Type list full URIs; the membership test is exact (a different case, a trailing space, the
    full PLUS URI or the `https` form of a Digital Source Type is not in the list); the full URI of
    a PLUS code; the write test of point 8; `validate` gives `VocabularyValueUnknown` for an unknown
    PLUS value and nothing for a known one or a blank one; Digital Source Type unchanged.
  - `pw-loupe`: `EditorField::ALL` order and sections with the new section; the property and kind
    of each new field; `from_read` → `edits` for a choice list: unchanged → no edit, another entry
    → `Set` of its exact value (the code for PLUS), the empty entry → `Clear`; an unknown value shown as its own entry,
    untouched → no edit, replaced → `Set`; the Id lists as T123's lists; copy and paste and
    "Apply to All Marked" carry a changed choice; `window_editor_properties` and Other Metadata
    without the new fields; the section's key in `settings.rs` (missing → open); `editor_text` of
    the heading; a label for every vocabulary value (no value without a label, no empty label); the
    label test covers every field.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` and `docs/glossary.md`
  are updated, and `git diff --stat` shows only the files above. The owner checks by hand on a
  JPEG, an ARW (sidecar) and a DNG: each choice set, written, read back with `M` and in Photo
  Mechanic; a choice cleared; the level-C `PM-jpg.JPG` copy (`MR-NAP`, `PR-NON`, `AG-A25`,
  `DMI-UNSPECIFIED`) shows its values selected; a file with an unknown value shows it with the
  warning and keeps it after an edit of another field.

## Questions (answered by the owner, 2026-11-14: every proposal approved)

- **U10d-Q1 Which PLUS properties.** ADR 0017's plan names the two release statuses, the release
  ids and Digital Source Type; ADR 0007 Q9 also names Minor Model Age Disclosure and Data Mining
  as choice lists, and ADR 0017 §2.3.1 lists Data Mining under Other Metadata "until a later
  step". Both vocabularies are read. Proposal: take all four PLUS choice lists now (Data Mining
  is how a photographer says "no AI training"); Model Age (numbers) stays under Other Metadata.
- **U10d-Q2 The Digital Source Type list (R2).** Its values come from IPTC's NewsCodes
  vocabulary `digitalsourcetype`, which is not saved in `D:\PhotoWorkflowData\references\` (only
  the IPTC specification, which names the vocabulary but not its codes). Proposal: the reviewer
  downloads it on the owner's yes, as R1 (`https://cv.iptc.org/newscodes/digitalsourcetype/`,
  the English labels, the server's bytes unchanged, size and SHA-256 in the folder's
  `README.md`), before T128 starts. The alternative is to leave Digital Source Type as a text
  field with today's warning and take it out of this task.
- **U10d-Q3 Labels in English.** ADR 0007 Q9 said "Swedish labels"; since GR14 (2026-11-13) the
  interface is English. Proposal: the vocabularies' own English labels (for example "Not
  Applicable", "Unlimited Model Releases", "Age 25 or Over", "Prohibited for AI/ML training"),
  the full URI as the entry's tooltip; Q9 is updated in ADR 0007 to say so.
- **U10d-Q4 An unknown value.** Proposal: shown as the list's extra entry "Unknown value: {uri}",
  selected, with the warning of point 4 below the form; never removed unless the user picks
  another entry (ADR 0007 §8, §11). Photo Mechanic writes the full URIs (level-C sample
  `PM-jpg.JPG`), so this should be rare.
- **U10d-Q5 Copy, paste and several images.** U12b-Q1 and U12c-Q3 name the fields of sections
  1–4 and Copyrighted. Proposal: the Licence section's fields join them, with the same rules
  (paste never empties a field; only changed fields go to the other marked images).
- **U10d-Q6 Section name.** U10-Q1 approved "Licence (PLUS)". Digital Source Type is an IPTC
  property, not PLUS, but belongs with the licence fields in ADR 0017 §2.3.1. Proposal: keep
  "Licence (PLUS)".

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Blocked | Written by the reviewer (ADR 0017 U10d with ADR 0007 I11) while T127 runs; waits for U10d-Q1–U10d-Q6 and R2 |
| 2026-11-14 | Open | Owner: U10d-Q1–U10d-Q6 approved as proposed; written into the points, ADR 0017 §6.1 and ADR 0007 §11 (Q9). R2 done by the reviewer on the owner's yes: `IPTC-NewsCodes-digitalsourcetype.json` (12 753 bytes, SHA-256 `1caaa5bb…88e7cca4fe52b9bb4d65701da4fb1d6`), 17 current and 3 retired codes, listed in points 1 and 5 |
| 2026-11-14 | Open | Reviewer, after the review of T127 (owner approved): the PLUS values are codes, since ExifTool removes and adds the prefix (a full URI is stored twice); Digital Source Type keeps full URIs; new point 8 (write test in `tests/edits.rs`). The agent paused; it reads the file again and continues |
| 2026-11-14 | Open | Reviewer: first review of the work so far: point 8's test fails because the write plan sends XMP values through ExifTool's print conversion ("not in PrintConv"); new point 9 (`#=` for URI properties in `write_plan.rs`), owner approved. Also left: `cargo fmt` in `editor.rs`, `docs/glossary.md` |
