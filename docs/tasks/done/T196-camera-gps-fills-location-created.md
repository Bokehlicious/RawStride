# T196. Camera GPS fills the coordinates of Location Created (ADR 0007 §6.7) — Done

- **Why:** ADR 0007 §6.1 maps the Exif GPS tags 1–6 to Location Created; §6.7 (revision 22,
  owner 2026-11-18) limits it to the coordinates. Today the reader does not read the Exif GPS
  tags at all. Nothing is written: GPS is "Exif → IPTC only" (§6.3).
- **Blocked** because T192 changes the same lines (`IMAGE_TAGS`, `ExifFields`, the step comment
  in `reconcile`, the `StructValue` import); start after T192 is Done. Line numbers below are
  from before T192.
- **Crates:** `pw-metadata` only. **Files:**
  - `crates/pw-metadata/src/read.rs`
  - `crates/pw-metadata/src/reconcile.rs`
  - `crates/pw-metadata/tests/read.rs`
  - `docs/test-inventory.md`
- **How ExifTool gives the values** (checked with ExifTool 13.59, the reader's options `-j -n
  -struct -api structformat=jsonq -G1`; every value arrives as a JSON string):
  - Exif: `GPS:GPSLatitude` and `GPS:GPSLongitude` as **unsigned** decimal degrees (`"33.9"`),
    `GPS:GPSLatitudeRef` `"N"`/`"S"`, `GPS:GPSLongitudeRef` `"E"`/`"W"`, `GPS:GPSAltitude` in
    metres (`"12.5"`), `GPS:GPSAltitudeRef` `"0"` (above sea level) or `"1"` (below). `tag()`
    skips the `XMP-exif:GPS…` keys that `-XMP:all` also gives, and `-GPS:` gives no Composite
    tags, so the six fields are the GPS IFD's (also in a RAW file).
  - XMP, inside a Location Created entry: `GPSLatitude` and `GPSLongitude` as **signed** decimal
    degrees (`"-33.9"`). Other programs may store the minutes with as few as three decimals (seen
    in the level-C sample `xiaomi_jpg.jpg`: XMP and Exif latitude differ in the sixth decimal), so the
    same position can differ by a few millionths of a degree.
  - The model (`check_kind`, `value.rs` about line 397) only accepts a `Number` whose
    `trim().parse::<f64>()` is finite; an XMP Location Created with a GPS field that is not such
    a number is refused as a whole (`Notice::Refused`) and never reaches this code.
- **Terms used below:**
  - `LOCATION_CREATED` = `"Iptc4xmpExt:LocationCreated"`, a list of `Location` structures
    (`Value::StructList`). Its GPS fields, all `Value::Number`, are the last four of the
    structure in the table (`iptc_table.rs` lines 1680–1711), in this order:
    `"exif:GPSAltitude"`, `"exif:GPSAltitudeRef"`, `"exif:GPSLongitude"`, `"exif:GPSLatitude"`.
  - **Parse:** `text.trim().parse::<f64>()`, finite, as `check_kind` does.
  - **The camera position**, from the six Exif fields (any Exif GPS counts as the camera's: the
    file does not say who wrote it; ADR 0007 §6.7). There is a position only when all of these
    hold:
    - GPSLatitude and GPSLongitude parse, their trimmed texts do not start with `+` or `-`, the
      latitude is 0–90 and the longitude 0–180;
    - GPSLatitudeRef is exactly `N` or `S`, GPSLongitudeRef exactly `E` or `W`.
    Its latitude text is the trimmed Exif text, with `-` in front for `S` unless the value parses
    as 0; the longitude likewise for `W`. The altitude is part of the position only when
    GPSAltitude parses, its trimmed text does not start with `+` or `-`, and GPSAltitudeRef is
    exactly `"0"` or `"1"`; then GPSAltitude is the trimmed Exif text and GPSAltitudeRef the ref.
  - **No position, with a notice:** at least one of GPSLatitude and GPSLongitude is present but
    the rules above do not hold (also when ExifTool gives `undef` or `inf` for a camera without
    a fix: intended). The notice is `Notice::Unreadable { property: LOCATION_CREATED, origin:
    Origin::Exif, text }`, where `text` is GPSLatitude, GPSLatitudeRef, GPSLongitude,
    GPSLongitudeRef, in that order, those that are present, as read, joined with one space (as
    `suggest_date_created` does).
  - **No position, no notice:** neither GPSLatitude nor GPSLongitude is present (for example
    only the refs, or only the altitude).
  - **Has coordinates:** a Location Created entry that has a GPSLatitude or a GPSLongitude field
    (the model guarantees it is a finite number).
  - **The same coordinates:** the entry has both fields, and its latitude and longitude each
    differ from the position's by at most 0.0001° (about 11 m). Altitude is not compared (ADR
    0007 §6.7).
  - **Set a field** in an entry: the value is replaced in place when the entry has the field;
    otherwise the field is appended at the end of the entry. Fields are set in the order
    GPSAltitude, GPSAltitudeRef, GPSLongitude, GPSLatitude, so an entry without GPS fields ends in
    table order; an entry that had some may not, which is fine (the model does not check the
    order).
- **What:**
  1. `read.rs`: add `"-GPS:GPSLatitudeRef"`, `"-GPS:GPSLatitude"`, `"-GPS:GPSLongitudeRef"`,
     `"-GPS:GPSLongitude"`, `"-GPS:GPSAltitudeRef"`, `"-GPS:GPSAltitude"` to `IMAGE_TAGS` (at the
     end), and in `exif_fields` the fields of point 2 with `field("GPSLatitude")` and so on (blank
     → `None`, as the others).
  2. `reconcile.rs`, `ExifFields`: six new `pub … : Option<String>` fields at the end:
     `gps_latitude`, `gps_latitude_ref`, `gps_longitude`, `gps_longitude_ref`, `gps_altitude`,
     `gps_altitude_ref`. Add them where a literal names every field (`read.rs` `exif_fields`, the
     unit-test literal in `read.rs` at about line 1108 with `None`); every literal in
     `reconcile.rs` uses `..ExifFields::default()`. `ExifPresent` and `write_plan.rs` are **not**
     changed (GPS is never written).
  3. `reconcile.rs`, `reconcile_exif`: after Digital Image GUID, a private helper gives the
     camera position or the notice of the terms. With a position:
     - **A refused Location Created:** when `work.notices` holds a `Notice::Refused` for
       `LOCATION_CREATED`, nothing (the file's own Location Created is not replaced by one built
       from Exif; reviewer's choice, 2026-11-18).
     - **No Location Created** (`work.current` is `None`) **or an empty list**: `work.fill` with
       a list of one entry that has only the position's fields, set as in the terms (GPSAltitude
       and GPSAltitudeRef only when the position has them), origin `Origin::Exif` in both cases.
     - **The first entry has no coordinates** (owner, 2026-11-18): set the position's latitude
       and longitude in that entry; set its GPSAltitude and GPSAltitudeRef only when the position
       has them **and** the entry has neither GPSAltitude nor GPSAltitudeRef. The entry's other
       fields and every other entry are unchanged; the property keeps its origin (`work.insert`
       with the current origin): no conflict, no notice.
     - **The first entry has the same coordinates:** nothing.
     - **Otherwise** (other coordinates, or only one of them): a conflict with rule
       `ConflictRule::Xmp`, values `[(current origin, current value), (Origin::Exif,
       exif_side)]`, where `exif_side` is the current list with the position's latitude and
       longitude set in the first entry, and its GPSAltitude and GPSAltitudeRef set too when the
       position has them. The stored value is not changed (XMP wins, §6.2).
     - If the current value is not a `Value::StructList`, nothing.
     - The same for a manufacturer RAW (§6.4): the sidecar is the XMP.
     Only the first entry is looked at (Location Created is one location in the window, §8).
     Add the constant `LOCATION_CREATED` and the four field ids at the top of `reconcile.rs` with
     the others. No `.unwrap()`, no unchecked indexing.
  4. Comments: the module comment's point 4 (it must name GPS → Location Created, the
     coordinates only), the module's first line (add §6.7 to its list of sections), and the step
     comment `// 4. Exif …` in `reconcile` (after T192 it says GPS is not part of this step yet;
     it is now).
- **Tests to add** (in `reconcile.rs` use T192's structure helper if it added one, otherwise add
  a test helper that builds a `StructValue` from `(&str, Value)` pairs; `number("…")` for the GPS
  fields):
  - `reconcile.rs`, section `// --- Exif ---` (a JPEG reading unless named otherwise; "the
    position" is Exif 33.9 `S`, 151.2 `W`, so the sign is tested):
    - `camera_gps_fills_an_absent_location_created`: the position with altitude 12.5 ref `0`
      gives Location Created = one entry [GPSAltitude "12.5", GPSAltitudeRef "0", GPSLongitude
      "-151.2", GPSLatitude "-33.9"], origin `Exif`, no conflict, no notice.
    - `camera_gps_fills_an_empty_location_created`: XMP Location Created `StructList(vec![])`
      and the position without altitude: one entry [GPSLongitude "-151.2", GPSLatitude
      "-33.9"], origin `Exif`.
    - `camera_gps_without_altitude_fills_only_the_coordinates`: no altitude fields, and once
      with GPSAltitudeRef `"2"`: the entry has only GPSLongitude and GPSLatitude.
    - `camera_gps_is_added_to_a_location_without_coordinates`: XMP Location Created [(City
      "Lund"), (City "Malmö")] and the position with altitude 12.5 ref `0`: the first entry
      becomes [City "Lund", GPSAltitude "12.5", GPSAltitudeRef "0", GPSLongitude "-151.2",
      GPSLatitude "-33.9"], the second is unchanged, the origin is still `XmpEmbedded`, no
      conflict.
    - `an_existing_altitude_is_kept_when_coordinates_are_added`: first entry [City "Lund",
      GPSAltitude "40"], and once [City "Lund", GPSAltitudeRef "1"], with the position with
      altitude 12.5 ref `0`: only GPSLongitude and GPSLatitude are appended; the altitude fields
      stay as they were.
    - `the_same_coordinates_within_a_ten_thousandth_of_a_degree_give_nothing`: XMP first entry
      [GPSLongitude "-151.19996", GPSLatitude "-33.90004"] and the position: no conflict, value
      unchanged; with GPSLatitude "-33.9002" instead: a conflict.
    - `other_coordinates_are_a_conflict_that_xmp_wins`: XMP [(City "Lund", GPSLongitude "13.19",
      GPSLatitude "55.7"), (City "Malmö")] and the position without altitude: one conflict, rule
      `Xmp`; the Exif side is [(City "Lund", GPSLongitude "-151.2", GPSLatitude "-33.9"), (City
      "Malmö")]; the stored value is the XMP list.
    - `a_location_with_only_a_latitude_is_a_conflict`: XMP first entry [City "Lund", GPSLatitude
      "55.7"] and the position without altitude: a conflict whose Exif side's first entry is
      [City "Lund", GPSLatitude "-33.9", GPSLongitude "-151.2"] (latitude in place, longitude
      appended).
    - `camera_gps_that_cannot_be_used_gives_a_notice`: once each, with no Location Created; no
      Location Created is filled and there is exactly one `Notice::Unreadable` for
      `LOCATION_CREATED`, origin `Exif`, with this text:
      - no GPSLatitudeRef (33.9, 151.2 `W`): `"33.9 151.2 W"`;
      - GPSLatitudeRef `"X"`: `"33.9 X 151.2 W"`;
      - latitude `"91"`, ref `N`: `"91 N 151.2 W"`;
      - longitude `"abc"`: `"33.9 S abc W"`;
      - latitude `"+33.9"`: `"+33.9 S 151.2 W"`.
    - `no_latitude_or_longitude_gives_nothing`: none of the six fields; and once only
      GPSLatitudeRef `N`, GPSAltitude "12.5" and GPSAltitudeRef `0`: no Location Created, no
      notice.
    - `a_refused_location_created_is_not_replaced`: an XMP Location Created the model refuses
      (for example an entry with GPSLatitude `"abc"`) and the position: no Location Created, the
      `Refused` notice only.
  - `reconcile.rs`, section `// --- RAW ---`:
    - `a_raw_file_gps_fills_an_absent_location_created_and_conflicts_with_rule_xmp`: a
      `FileKind::Raw` reading: with no Location Created in the sidecar Exif fills; with other
      coordinates a conflict with rule `Xmp`.
  - `tests/read.rs` (real ExifTool, as `exif_artist_and_image_description_fill_creator_and_description`;
    find the fields in the entry with a `match` on the `StructList`):
    - `exif_gps_fills_location_created`: a JPEG written with `-GPS:GPSLatitude=33.9`,
      `-GPS:GPSLatitudeRef=S`, `-GPS:GPSLongitude=151.2`, `-GPS:GPSLongitudeRef=W`,
      `-GPS:GPSAltitude=12.5`, `-GPS:GPSAltitudeRef#=0` reads Location Created with one entry,
      origin `Exif`; its GPSLatitude and GPSLongitude parse to −33.9 and −151.2 (within 1e-9),
      GPSAltitude to 12.5 and GPSAltitudeRef is "0".
    - `exif_gps_is_added_to_the_city_of_location_created`: the same file with also
      `-XMP-iptcExt:LocationCreated={City=Lund}`: one entry with City "Lund" and the four GPS
      fields, origin `XmpEmbedded`, no conflict.
- **Not in this task:**
  - Writing GPS anywhere (§6.3), editing the coordinates in the window (ADR 0017: read-only
    until the GPS ADR), Location Shown (never from GPS, §6.1).
  - City, country and location from a position, GPX tracks and a map (the GPS ADR).
  - `tests/level_c.rs`: the cold read expects no assertion to change (only `xiaomi_jpg.jpg` has
    camera GPS; its Location Created has no coordinates and gets them, origin unchanged). If an
    assertion does change, do not change it; stop and report which one and why.
- **Hand check (owner, after review):** open the window on a camera JPEG with GPS and no
  Location Created: Location Created shows the coordinates. On an image with a City from the
  import template and camera GPS: the City and the coordinates show; change the City, OK, open
  again: the coordinates are still there and are now in the XMP (the window keeps the read-only
  GPS fields of the entry; ExifTool accepts the signed values, checked in the cold read).
- **Rules:** `docs/tasks/README.md`; ADR 0007 §6.7 is the specification; no user-visible text is
  added (the info panel already shows the `Unreadable` notice).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  and this task's file.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-18 | Blocked | Added (ADR 0007 revision 22, §6.7; the owner chose the same day that the coordinates are added to a Location Created without coordinates, keeping its origin); waits for T192 |
| 2026-11-18 | Blocked | Cold read: a fresh agent confirmed names, lines, the GPS field order, how ExifTool gives the values (also that `tag()` cannot pick an XMP or Composite GPS tag), that T196 fits on top of T192, that no existing test reads GPS, and that ExifTool writes the signed coordinates back. Written in from its findings: an empty list is filled with origin Exif (the text contradicted itself); "has coordinates" is the field's presence (the model refuses a GPS field that is not a number, so "does not parse" cannot happen); a refused Location Created is not replaced (reviewer's choice); no second GPSAltitudeRef; how a field is set (in place, else appended); the notice's text and order, no notice without latitude and longitude, a leading `+`/`-` refused; "three decimals" corrected; the expected values of every test; the test helpers; ADR §6.7 gained "altitude not compared" and "any Exif GPS counts as the camera's" |
| 2026-11-18 | Open | T192 is Done, so the task is no longer blocked; its line numbers are still from before T192 |
| 2026-11-19 | Ready for review | Done as written: read.rs reads the six GPS tags into six new `ExifFields` fields; reconcile.rs `reconcile_camera_gps` (after the GUID) with `camera_position`, the constants and the helpers; comments updated; 12 unit tests and 2 tests in `tests/read.rs`; `docs/test-inventory.md` updated. `.\tools\check.ps1` all OK (1931 passed, 72 ignored, 0 failed). `tests/level_c.rs` run with `--ignored` and `PW_TESTDATA`: 10 passed, no assertion changed |
| 2026-11-19 | Ready for review | Review: every point and test as written (the position's rules, the fill / add / same / conflict / refused cases, only the first entry, the altitude kept, no panic path in non-test code), `write_plan.rs` unchanged (a value of origin Exif is written only when changed; `…_gps_are_never_written` still holds), `git diff --stat` only the named files, `.\tools\check.ps1` run by the reviewer all OK (1931 passed, 72 ignored, 0 failed, the sum of every `test result:`; no clippy warnings); `tests/level_c.rs` not run again by the reviewer (needs `PW_TESTDATA`). The decisions of the task not yet in the ADR (the first entry keeps its origin, only the first entry, a refused Location Created not replaced, the notice) written into ADR 0007 §6.7, revision 25 |
| 2026-11-19 | Done | Approved by the owner. The owner's hand check (a camera JPEG with GPS; an image with a City and camera GPS, the City changed and the coordinates written to XMP) follows |
