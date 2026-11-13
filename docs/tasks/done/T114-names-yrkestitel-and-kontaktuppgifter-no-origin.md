# T114. Names "Yrkestitel" and "Kontaktuppgifter", no origin line under read-only values — Done

- **Can start after T110 is Done** (it changes the inspector T110 adds, in `main.rs`). Owner's
  decisions of 2026-11-13, after the hand check of T110. Not at the same time as any other task
  that changes `main.rs` (T111, T112).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/info_panel.rs` (`SV_LABELS` and its tests);
  `crates/pw-loupe/src/editor.rs` (tests only, point 4); `crates/pw-loupe/src/main.rs` (points 2
  and 3); `docs/adr/0017-interface-shell.md` (§2.3.1, point 5); `docs/test-inventory.md`.
- **Why:** the owner found two Swedish names too long, and the origin line ("XMP", "XMP-sidofil")
  under every value in the inspector and in the metadata window's "Övrig metadata" more noise
  than help. The origin line is correct (it names
  where that value was read: a RAW file's values come from its sidecar, but a value only the camera
  wrote comes from the XMP inside the RAW, so one image can show both), but the owner does not want
  it under the values; only the conflicts keep it.
- **What:**
  1. **Names** (`info_panel.rs`, `SV_LABELS`; the D11 names they replace):
     - `photoshop:AuthorsPosition`: "Fotografens titel" → **"Yrkestitel"** (not "Titel", which is
       `dc:title`'s name; two fields with one name would be ambiguous);
     - `Iptc4xmpCore:CreatorContactInfo`: "Fotografens kontaktuppgifter" → **"Kontaktuppgifter"**.

     Every place that names these properties through `property_label` follows: the inspector, the
     metadata window's field and its warning ("Yrkestitel utan Fotograf"), "Övrig metadata" and
     the conflicts.
  2. **The metadata window** (`main.rs`): the field "Fotografens titel" is shown as "Yrkestitel"
     (point 1), and a heading **"Kontaktuppgifter"** (from `property_label` of
     `Iptc4xmpCore:CreatorContactInfo`, in the strong style of the window's other headings) is
     drawn above the fields E-post, Telefon and Webb, which keep their names. **"Övrig metadata"**
     follows the same rule as the inspector (owner, 2026-11-13): no origin line under a value. In
     the window's conflicts each value keeps its origin. Nothing else in the window changes.
  3. **The inspector's Metadata section** (`main.rs`, T110's `draw_inspector_metadata`): no origin
     line under a value. In the **conflicts** each value keeps its origin (it is what tells the
     values apart), and the rule text stays.
  4. **Existing tests** that state the old names may be changed for the new names, and only that:
     the label list in `editor.rs` (`"Fotografens titel"` → `"Yrkestitel"`), the warning test in
     `editor.rs` (`"Fotografens titel utan Fotograf"` → `"Yrkestitel utan Fotograf"`), and any
     test in `info_panel.rs` with either old name. No other assertion changes (rule 5).
  5. **ADR 0017 §2.3.1**, section 2 of the list ("Fotograf och rättigheter"): "Fotografens titel,
     kontaktuppgifter" becomes "Yrkestitel, Kontaktuppgifter", and the bullet "The Swedish names
     follow D11" gains "(Yrkestitel and Kontaktuppgifter: owner, 2026-11-13)". Nothing else in the
     ADR changes.
- **Rules:** rules 3–7 and 10–13. No `Cargo.toml` change. Property names only from
  `property_label` (rule 13).
- **Tests to add:** `property_label` gives "Yrkestitel" and "Kontaktuppgifter" in Swedish; the
  English names are unchanged.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: the
  inspector shows "Yrkestitel" and "Kontaktuppgifter" and no "XMP" or "XMP-sidofil" under the
  values (a JPEG and an ARW with a sidecar), while a conflict still shows each value's origin; the
  `M` window shows "Yrkestitel", the heading "Kontaktuppgifter" above E-post, Telefon and Webb,
  and no origin line under the values of "Övrig metadata".
