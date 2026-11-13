# Not for agents yet

These need design decisions, new dependencies or platform-specific code, and stay with the owner
and reviewer:

- A timeout when ExifTool hangs (process supervision across threads).
- Suppressing the Windows "no disk in drive" dialog when scanning empty card readers (needs a
  Windows API call and a new dependency).
- ADR 0007 (IPTC metadata) steps I11–I13 and the `Cleared` task (see "Milestone 8 (ADR 0007, IPTC
  metadata): steps"; I9 is T87–T93, I10 is T94–T106): `Cleared` waits for D1; I11 waits for the PLUS controlled
  vocabulary; I12 waits for the level-C samples. Also later: Exif ImageEditor ↔ Contributor and
  GPS → Location Created (§6.1), left out of T81; keyword qualifiers (§8), left out of T83; from
  §10, left out of T88: template sub-fields of lists of structures (Location Created, Image
  Creator, Copyright Owner, Licensor), variables (`{date}` …), template files in XMP with the
  modes in a RawStride namespace (needs the same kind of namespace decision as D1). Left out of
  I10: editing several images at once, lists of structures in the editor, the other controlled
  vocabularies of §11.
- **Future improvement (owner, 2026-11-12, after D11):** Swedish names for **every** IPTC property
  and structure field in T102's name table, so the Swedish info panel and editor show no English
  IPTC names. Today D11 names about 35 of them; the rest fall back to IPTC's English name. Needs
  the owner's approval of the full list.
- The import dialog fields for title and description in `ui/` (frontend).
- ADR 0010 steps M3–M7 (the rest of the import) wait until the contact sheet (ADR 0011) is done
  (owner decision, 2026-11-08). M4 and M7 are added to this list then.
- ADR 0011 step K4 (drawing the contact sheet) is planned after T23–T25.
- Moving rejected or picked images to `_rejected` / `_picked` folders (ADR 0013 §13): needs its own
  ADR first.
- **A filter bar of toggle buttons instead of drop-down lists** (owner idea, 2026-11-10, after T61;
  ADR 0014 §15): a row "0★ 1★ 2★ 3★ 4★ 5★" and a row of colour swatches, each clicked on and off,
  with a check box per row that turns the row on or off. Needs an amendment of ADR 0014 first: the
  star part would become a set of star values instead of the ≥ / = / ≤ condition (T60).
- Note for M5: when the import opens its folder with `Tabs::open_new`, compare folders by their
  resolved form (`pw_ingest::resolve`), so `C:\Foto` and `c:\foto\` do not open two tabs.
