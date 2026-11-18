# T195. Scene Code, Subject Code and Genre checked against IPTC NewsCodes (ADR 0007 §11) — Open

- **Why:** ADR 0007 §11 (revision 22, owner 2026-11-18) adds warnings for three more controlled
  vocabularies. Today `validate.rs` checks only Digital Source Type and the PLUS choice lists, and
  its module comment says the other vocabularies are left out. Warnings only: a value is never
  refused, changed or removed.
- **Crates:** `pw-metadata` and, for the warning texts only, `pw-loupe`. **Files:**
  - `crates/pw-metadata/src/vocabulary.rs`
  - `crates/pw-metadata/src/validate.rs`
  - `crates/pw-loupe/src/editor.rs` (`warning_text` and its tests only)
  - `docs/test-inventory.md`
- **The vocabularies** (the reference copies R4 in `D:\PhotoWorkflowData\references\`; the agent
  needs only the lists below, which are copied from them exactly):
  - `IPTC-NewsCodes-scene.json`, SHA-256
    `d40592d420218a01bdeea8c1cb26205fa888cbcad856bb173012988c9c9f0796`: 24 codes, none
    retired: `010100`, `010200`, … `012400`, every multiple of 100 from 010100 to 012400.
  - `IPTC-NewsCodes-genre.json`, SHA-256
    `8595892d4c17dcb04adf2c4c581e2fa629bf52fb1c3565119db0c1f5f086b64f`. Every URI is
    `http://cv.iptc.org/newscodes/genre/` followed by the code. The 51 current codes in the file's
    order: `Actuality`, `Advertiser_Supplied`, `Advice`, `Advisory`, `almanac`, `Analysis`,
    `Archive_material`, `Background`, `Behind_the_Story`, `Biography`, `Birth_Announcement`,
    `Current`, `Curtain_Raiser`, `Daybook`, `Exclusive`, `Fact_Check`, `Feature`, `Fixture`,
    `Forecast`, `From_the_Scene`, `Help_Us_to_Report`, `History`, `horoscope`, `Interview`,
    `ListingOfFacts`, `LiveCoverage`, `Music`, `Obituary`, `Opinion`, `Polls_and_Surveys`,
    `Press_Release`, `Press-Digest`, `Preview`, `Profile`, `Question_and_Answer_Session`, `Quote`,
    `Raw_Sound`, `Response_to_a_Question`, `Results_Listings_and_Statistics`, `Retrospective`,
    `Review`, `Satire`, `Side_bar_and_supporting_information`, `Special_Report`, `Sponsored`,
    `Summary`, `Supported`, `Synopsis`, `Transcript_and_Verbatim`, `UGC`, `Voicer`. The 7 retired
    ones (still valid in a file): `Anniversary`, `Program`, `Scener`, `Text_only`, `Update`,
    `Wrap`, `Wrapup`. The case varies; compare exactly.
- **Terms used below:**
  - `SCENE` = `"Iptc4xmpCore:Scene"` and `SUBJECT_CODE` = `"Iptc4xmpCore:SubjectCode"`, both a
    list of texts (`Value::List`).
  - `GENRE` = `"Iptc4xmpExt:Genre"`, a list of `CvTerm` structures (`Value::StructList`); the
    fields used are `"Iptc4xmpExt:CvId"` and `"Iptc4xmpExt:CvTermId"`, both texts
    (`Value::Text`).
  - **An IPTC Genre entry:** an entry whose CvId is `GENRE_PREFIX` or `GENRE_PREFIX_HTTPS`
    (point 1), each also without its trailing `/` (ADR 0007 §11: http or https, with or without
    the `/`).
  - **Blank:** `iim::is_blank`, as everywhere in `validate.rs`.
- **What:**
  1. `vocabulary.rs`:
     - public constants `SCENE`, `SUBJECT_CODE`, `GENRE` (the ids above), `GENRE_PREFIX` =
       `"http://cv.iptc.org/newscodes/genre/"` and `GENRE_PREFIX_HTTPS` =
       `"https://cv.iptc.org/newscodes/genre/"`;
     - private constants `SCENE_CODES: [&str; 24]`, `GENRE_CODES: [&str; 51]` and
       `RETIRED_GENRE_CODES: [&str; 7]`, in the file's order, written out (not generated);
     - `pub fn is_scene_code(code: &str) -> bool` and `pub fn is_genre_code(code: &str) -> bool`
       (current **or** retired), both exact, no trimming, no case folding;
     - `pub fn is_iptc_genre_vocabulary(cv_id: &str) -> bool`: true when `cv_id` is one of the
       two prefixes, or one of them without its trailing `/` (use `strip_suffix('/')` on the
       prefix, which gives an `Option`; no `.unwrap()`), exact otherwise;
     - `pub fn genre_code(term_id: &str) -> Option<&str>`: `term_id` with `GENRE_PREFIX` or
       `GENRE_PREFIX_HTTPS` removed (`strip_prefix`), `None` when it starts with neither. It
       does not check the code.
     - They are **not** added to `choices`: Scene and Genre stay free-text fields in the editor
       (`choices` decides which fields become choice lists).
     - The module comment: widen its first sentence (the module now also holds vocabularies
       that are not choice lists) and add R4, the two file names with their SHA-256, and that
       these two lists are only used by `validate` and are not choice lists.
  2. `validate.rs`, `Warning`: two new variants after `VocabularyValueUnknown`, each with a doc
     comment:
     - `NewsCodeUnknown { property: String, value: String }`: an entry of Scene Code that is not
       a code of IPTC Scene NewsCodes, or the CV-Term ID of an IPTC Genre entry that is not a
       code of IPTC Genre NewsCodes; `value` is the text as read (for Genre the whole CV-Term
       ID, not only the code).
     - `SubjectCodeForm { code: String }`: an entry of Subject Code that is not 8 ASCII digits.
     Their `Display` texts (English, as the others): `"{property}: \"{value}\" is not in the IPTC
     NewsCodes vocabulary"` and `"Subject Code \"{code}\" is not 8 digits"`.
  3. `validate.rs`, `validate`: in the `match def.xmp_id`, three new arms that **extend**
     `warnings` (as the `REGISTRY` arm does) and give `None`:
     - `SCENE`: for each entry of a `Value::List`, in list order, that is not blank and for
       which `vocabulary::is_scene_code` is false: `NewsCodeUnknown { property: SCENE, value:
       entry }`. A 6-digit text that is not in the list warns too; so does `"10100"`, a code
       with spaces around it (exact), and a full Scene URI
       (`http://cv.iptc.org/newscodes/scene/010100`), since the property holds the bare code
       [IPTC §7.21].
     - `SUBJECT_CODE`: for each entry, in list order, that is not blank and is not exactly 8
       ASCII digits: `SubjectCodeForm { code: entry }`. Not checked against a vocabulary (§11:
       IPTC retired it before 2010).
     - `GENRE`: for each entry of a `Value::StructList`, in list order, whose CvId
       (`present_text`) satisfies `vocabulary::is_iptc_genre_vocabulary` and whose CvTermId
       `present_text` gives a text: `vocabulary::genre_code` of it must give a code for which
       `vocabulary::is_genre_code` is true (CvId and CvTermId need not use the same scheme);
       otherwise `NewsCodeUnknown { property: GENRE, value: the CvTermId }` (a bare code
       without the prefix warns too). An entry of another vocabulary, or without CvId or
       CvTermId, gives nothing (§11: Genre allows any vocabulary).
     Intellectual Genre (`Iptc4xmpCore:IntellectualGenre`) gets no arm (§11). Use the constants
     from `vocabulary` in the arms (the module already imports it). No `.unwrap()`, no unchecked
     indexing.
  4. `validate.rs`, the module comment: add the three rules to the list "The rules checked (§11)"
     and replace the "Left out here" point on the controlled vocabularies with: Intellectual Genre,
     and Genre entries of other vocabularies, are not checked (§11).
  5. `editor.rs`, `warning_text`: two new arms, built with `property_label(…, Language::En)` as
     the others:
     - `NewsCodeUnknown { property, value }` → `"{field} {value} is not in IPTC's list"` (the
       same words as Digital Source Type's text);
     - `SubjectCodeForm { code }` → `"{field} {code} should be eight digits"`, `field` =
       `property_label("Iptc4xmpCore:SubjectCode", Language::En)`.
     No other change in `editor.rs`; the window already shows every warning `validate` gives.
- **Tests to add:**
  - `vocabulary.rs`:
    - `the_scene_and_genre_lists_have_their_counts_ends_and_no_duplicate`: 24 Scene codes from
      `010100` to `012400`, each 6 ASCII digits; 51 current Genre codes from `Actuality` to
      `Voicer` and 7 retired from `Anniversary` to `Wrapup`; no code in both Genre lists; no
      duplicate in any list.
    - `scene_and_genre_membership_is_exact`: `is_scene_code("010100")` true, `"10100"`,
      `" 010100"`, `"012500"` false; `is_genre_code("Actuality")` and `("Wrapup")` (retired)
      true, `("actuality")`, `("Almanac")`, `("Actuality ")` false.
    - `the_iptc_genre_vocabulary_and_its_codes`: `is_iptc_genre_vocabulary` true for the two
      prefixes with and without `/`, false for `"http://cv.iptc.org/newscodes/genre//"`,
      `"HTTP://cv.iptc.org/newscodes/genre/"` and `"http://cv.iptc.org/newscodes/scene/"`;
      `genre_code` gives `Some("Feature")` for both prefixes followed by `Feature`, `Some("")`
      for the prefix alone and `None` for `"Feature"`.
    - `the_scene_and_genre_codes_are_not_choice_lists`: `choices(SCENE)`, `choices(GENRE)` and
      `choices(SUBJECT_CODE)` are `None`.
  - `validate.rs` (add a helper `cv_term(cv_id, term_id) -> StructValue` to the test module;
    the warnings below are short for `Warning::NewsCodeUnknown { property:
    vocabulary::SCENE.to_string(), value: "999999".to_string() }` and so on; the ids are reached
    as `vocabulary::SCENE` etc.):
    - `scene_codes_from_the_newscodes`: Scene `["010100", "", "999999", "012400", "10100"]`
      gives exactly `[NewsCodeUnknown(SCENE, "999999"), NewsCodeUnknown(SCENE, "10100")]`.
    - `subject_codes_of_eight_digits`: Subject Code `["04000000", "4000000", "IPTC:04000000",
      " ", "0400000a"]` gives `SubjectCodeForm` for `"4000000"`, `"IPTC:04000000"` and
      `"0400000a"`, in that order.
    - `genre_is_checked_only_for_the_iptc_vocabulary`: Genre with five entries: (CvId
      `http://cv.iptc.org/newscodes/genre/`, CvTermId `http://cv.iptc.org/newscodes/genre/Feature`)
      nothing; (CvId `https://cv.iptc.org/newscodes/genre`, CvTermId
      `https://cv.iptc.org/newscodes/genre/Wrapup`) nothing (retired); (CvId as the first, CvTermId
      `http://cv.iptc.org/newscodes/genre/feature`) a warning with that text; (CvId as the first,
      CvTermId `Feature`) a warning; (CvId `http://example.com/genres/`, CvTermId `anything`)
      nothing. So two warnings, in entry order.
    - `intellectual_genre_is_not_checked`: Intellectual Genre `"anything"` gives no warning.
    - `the_newscodes_warnings_follow_the_table_order`: one bad Scene, Subject Code and IPTC
      Genre entry each, inserted in the order Subject Code, Scene, Genre, gives the warnings in
      the table's order Genre, Scene, Subject Code; check that order with `table_position` as
      `every_warning_once_in_table_order` does. Leave that existing test unchanged (it does not
      list the PLUS variant either).
    - Every new test that calls `validate` does so with `iim_written` false (`warnings_of(..,
      false)`), so no IIM length warning can mix in.
    - `the_english_texts_of_the_newscodes_warnings`: the two `Display` texts of point 2.
  - `editor.rs`, next to `the_unknown_value_and_warning_texts`:
    - `the_newscodes_warning_texts`: `warning_text` of `NewsCodeUnknown { property:
      "Iptc4xmpCore:Scene", value: "999999" }` equals `format!("{} 999999 is not in IPTC's
      list", property_label("Iptc4xmpCore:Scene", Language::En))`, and `SubjectCodeForm { code:
      "4000000" }` equals `format!("{} 4000000 should be eight digits",
      property_label("Iptc4xmpCore:SubjectCode", Language::En))`.
- **`docs/test-inventory.md`:** the pw-metadata and pw-loupe rows and the total at the top
  (`editor.rs` is in the lib target only).
- **Not in this task:**
  - Choice lists for Scene Code or Genre in the editor; IPTC Media Topics (CV-Term About Image).
  - Intellectual Genre's IIM prefix `000:` and Subject Code's `IPTC:` (§4, already done).
  - Swedish texts (GR14: English only).
- **Rules:** `docs/tasks/README.md`, point 13 for the two texts; ADR 0007 §11 is the
  specification.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  and this task's file.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-18 | Open | Added (ADR 0007 revision 22, §11; vocabularies read as R4 the same day) |
| 2026-11-18 | Open | Cold read: a fresh agent found no statement wrong against the code (ids, value kinds, table order Genre → Scene → Subject Code, the two exhaustive matches on `Warning`, the lists and checksums). Written in from its findings: named helpers `is_iptc_genre_vocabulary` and `genre_code` and the https prefix constant (and §11 now says http or https, with or without `/`), a full Scene URI warns, the Genre warning holds the whole CV-Term ID, `present_text` for the Genre fields, the test helper `cv_term` and the warning shorthand, `iim_written` false only in tests that call `validate`, the test-inventory rows |
