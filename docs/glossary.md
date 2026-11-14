# Glossary: the user interface in English

- Module: `pw-loupe` (the texts the user sees), with the property names of `pw-metadata`
- Question: which English word replaces each Swedish UI term, so that English can become the
  interface's base language (owner, 2026-11-13) and every later task translates the same way.
- Status: **approved by the owner (2026-11-13)**, with G1–G8 (§8) as proposed. English is the
  interface's base language (GUI_REQUIREMENTS GR14); Swedish texts are translated with these terms.
- Kept up to date: a task that adds a UI term not listed here adds it to this file
  (`docs/tasks/README.md`, rule 13).
- Source: every string literal in the non-test code of `crates/pw-loupe/src/` (commit `241263c`),
  about 300 user-visible texts; IPTC names from `pw-metadata`'s property table, checked against
  IPTC Photo Metadata Standard 2025.1 (`D:\PhotoWorkflowData\references\`).

This lists **terms**, not every sentence. Task B translates the sentences with these terms; whole
sentences stay whole (rule 13).

## 1. IPTC and other metadata properties

English mode already shows IPTC's name from the property table (`property_label`,
`Language::En`, ADR 0007 §17.1). The column "Proposed label" is what the UI should show (G1).

| Property | Swedish (D11) | IPTC 2025.1 official name | Proposed label |
|---|---|---|---|
| `dc:title` | Titel | Title | Title |
| `photoshop:Headline` | Rubrik | Headline | Headline |
| `dc:description` | Beskrivning | Description | Description |
| `Iptc4xmpCore:AltTextAccessibility` | Alternativtext | Alt Text (Accessibility) | Alt Text (Accessibility) |
| `Iptc4xmpCore:ExtDescrAccessibility` | – | Extended Description (Accessibility) | Extended Description (Accessibility) (T123) |
| `dc:subject` | Nyckelord | Keywords | Keywords |
| `dc:creator` | Fotograf | Creator | Creator |
| `photoshop:AuthorsPosition` | Yrkestitel | Creator's jobtitle | Creator's jobtitle |
| `Iptc4xmpCore:CreatorContactInfo` | Kontaktuppgifter | Creator's Contact Info | Creator's Contact Info |
| `…:CiAdrExtadr` | – | Address | Address (T123) |
| `…:CiAdrCity` | – | City | City (T123; under Creator's Contact Info) |
| `…:CiAdrRegion` | – | State/Province | State/Province (T123) |
| `…:CiAdrPcode` | – | Postal Code | Postal Code (T123) |
| `…:CiAdrCtry` | – | Country | Country (T123; under Creator's Contact Info) |
| `…:CiEmailWork` | E-post | Email address(es) | Email (G1) |
| `…:CiTelWork` | Telefon | Phone number(s) | Phone (G1) |
| `…:CiUrlWork` | Webb | Web URL(s) | Web URL (G1) |
| `dc:rights` | Copyright | Copyright Notice | Copyright Notice |
| `photoshop:Credit` | Credit | Credit Line | Credit Line |
| `photoshop:Source` | Källa | Source (Supply Chain) | Source (Supply Chain) |
| `xmpRights:UsageTerms` | Användarvillkor | Rights Usage Terms | Rights Usage Terms |
| `xmpRights:WebStatement` | Webbadress för rättigheter | Web Statement of Rights | Web Statement of Rights |
| `photoshop:DateCreated` | Skapad | Date Created | Date Created |
| `Iptc4xmpCore:Location` | Plats | Sublocation (legacy) | Sublocation (G1) |
| `photoshop:City` | Stad | City (legacy) | City (G1) |
| `photoshop:State` | Län/delstat | Province or State (legacy) | Province or State (G1) |
| `photoshop:Country` | Land | Country (legacy) | Country (G1) |
| `Iptc4xmpCore:CountryCode` | Landskod | Country Code (legacy) | Country Code (G1) |
| `photoshop:Instructions` | Instruktioner | Instructions | Instructions |
| `photoshop:TransmissionReference` | Jobb-id | Job Id | Job Id |
| `Iptc4xmpCore:SubjectCode` | – | Subject Code (legacy) | Subject Code (G1; T123) |
| `Iptc4xmpCore:Scene` | – | Scene Code | Scene Code (T123) |
| `photoshop:CaptionWriter` | Beskrivningens författare | Description Writer | Description Writer |
| `xmp:Rating` | Betyg | Image Rating | Rating (G1) |
| `Iptc4xmpExt:LocationCreated` | Plats där bilden togs | Location Created | Location Created |
| `Iptc4xmpExt:LocationShown` | Plats som visas | Location Shown in the Image | Location Shown in the Image |
| `plus:ImageCreator` | Bildskapare | Image Creator | Image Creator |
| `plus:CopyrightOwner` | Upphovsrättsinnehavare | Copyright Owner | Copyright Owner |
| `plus:Licensor` | Licensgivare | Licensor | Licensor |
| `Iptc4xmpExt:PersonInImage` | Person i bilden | Person Shown in the Image | Person Shown in the Image |
| `Iptc4xmpExt:OrganisationInImageName` | – | Name of Organisation Featured in the Image | Name of Organisation Featured in the Image (T123) |
| `Iptc4xmpExt:OrganisationInImageCode` | – | Code of Organisation Featured in the Image | Code of Organisation Featured in the Image (T123) |
| `Iptc4xmpExt:Event` | Händelse | Event Name | Event Name |
| `Iptc4xmpExt:DigitalSourceType` | Digital källtyp | Digital Source Type | Digital Source Type |
| `xmp:Label` | Etikett | – (not IPTC; XMP Basic "Label") | Label |
| `xmpRights:Marked` | Upphovsrättsskyddad | – (not IPTC; XMP Rights "Marked") | Copyrighted (G2) |
| `photoshop:Urgency` | Prioritet | – (deprecated; IIM 2:10 "Urgency") | Urgency |
| `photoshop:Category` | Kategori | – (deprecated; IIM 2:15 "Category") | Category |
| `photoshop:SupplementalCategories` | Tilläggskategorier | – (deprecated; IIM 2:20 "Supplemental Category") | Supplemental Categories |

Every other property already uses IPTC's English name in both languages.

## 2. Ratings, labels, marking and filters

| Swedish | English | Note |
|---|---|---|
| betyg, Betyg: … | rating, Rating: … | |
| 1 stjärna / {n} stjärnor | 1 star / {n} stars | singular/plural in one function (rule 13) |
| Inga stjärnor | No stars | menu item |
| Ej betygsatt | Unrated | |
| Avvisa / Avvisad / Avvisade | Reject / Rejected / Rejected | rating −1 (G3) |
| Alla betyg | All ratings | |
| Alla utom avvisade | All but rejected | |
| etikett / Ingen etikett / Alla etiketter / {n} etiketter | label / No label / All labels / {n} labels | |
| Röd, Gul, Grön, Blå, Lila | Red, Yellow, Green, Blue, Purple | the stored `xmp:Label` values are already these English words |
| markera, markerad, Markera alla, Avmarkera | mark, marked, Mark All, Unmark All | the set chosen with the mouse/keys (G4) |
| vald (Ingen bild vald) | selected (No image selected) | the current image (G4) |
| Bara markerade | Marked only | |
| Filter, Visa alla | Filter, Show All | |

## 3. Window, menus and commands

| Swedish | English | Note |
|---|---|---|
| Arkiv, Redigera, Bild, Visa | File, Edit, Image, View | menu titles (Windows convention) |
| Öppna mapp… / Importera… / Stäng flik / Inställningar… / Avsluta | Open Folder… / Import… / Close Tab / Settings… / Exit | |
| Nästa flik / Föregående flik | Next Tab / Previous Tab | |
| Bibliotek, Uppgifter, Inspektör | Library, Tasks, Inspector | column titles |
| Sidokolumner, Vänster kolumn | Side Columns, Left Column | |
| Fäll ihop kolumnen / Visa kolumnen | Collapse Column / Show Column | |
| Helskärm | Full Screen | |
| Tema, Mörkt, Ljust | Theme, Dark, Light | |
| Tom (flik) | Empty | |
| Läge: RAW / JPEG | Mode: RAW / JPEG | |
| Storlek | Size | thumbnail size |
| mapp | folder | |

## 4. Viewer and zoom

| Swedish | English |
|---|---|
| Anpassa / Anpassad | Fit / Fit (as an adjective: "Zoom: Fit") |
| Anpassa ↔ 100 % | Fit ↔ 100% |
| Zooma in / Zooma ut | Zoom In / Zoom Out |
| Zooma vid pekaren / zoom mot pekaren / zoom mot mitten | Zoom at Pointer / zoom to pointer / zoom to centre (G5) |
| Förbereder RAW … | Preparing RAW … |
| RAW-rendering, Inbäddad JPEG (kamerans rendering) | RAW rendering, Embedded JPEG (camera rendering) |
| förhandsbild / förhandsvisning | preview |
| miniatyr | thumbnail |
| färgprofil, skärmprofil | colour profile, display profile (G5) |
| minnesbudget | memory budget ("the TIFF file is too large for the memory budget"; added by T118) |
| kamerainformation | camera information |
| Kamera, Objektiv, Brännvidd, Bländare, Exponeringstid, ISO, Tagen | Camera, Lens, Focal Length, Aperture, Exposure Time, ISO, Taken |

## 5. Import

| Swedish | English | Note |
|---|---|---|
| Importera, Starta import, Ny import | Import, Start Import, New Import | |
| Källa, Mål, Backup | Source, Destination, Backup | |
| Välj…, Välj källa / mål / backupmål | Choose…, Choose Source / Destination / Backup Destination | |
| Mappmall, Filnamnsmall | Folder Template, File Name Template | |
| Jobb, Första nr | Job, First Number | |
| Förhandsvisning, Förhandsvisar …, Sök igen | Preview, Previewing …, Search Again | |
| Kopierar och verifierar | Copying and verifying | |
| namnkrock, finns redan, bara backup | name conflict, already exists, backup only | |
| Mata ut kortet … (inget raderas från kortet) | Eject the card … (nothing is deleted from the card) | |
| Klar / Avbruten / Misslyckades | Done / Cancelled / Failed | |
| Behåll, Ersätt, Lägg till efter, Lägg till före, Rensa | Keep, Replace, Append, Prefix, Clear | the template modes; the same words as ADR 0007 §10 |
| Rensa och importera | Clear and Import | |
| Dölj, Stäng | Hide, Close | |
| IPTC-mall, Mallen, Variabler | IPTC Template, the template, Variables | added by T117 |
| enhet | drive | "The source is not a drive"; added by T118 |
| mata ut, utmatat | eject, ejected | "E: has been ejected and can be removed."; added by T118 |
| Utforskaren | File Explorer | Windows' own name; added by T118 |
| fullständig sökväg | full path | "The source must be a full path: …"; added by T118 |

## 6. Metadata window, info and settings

| Swedish | English | Note |
|---|---|---|
| Metadata – {fil} | Metadata – {file} | |
| Olösta konflikter / Konflikter / olöst | Unresolved Conflicts / Conflicts / unresolved | |
| Varningar, Meddelanden | Warnings, Notices | |
| Övrig metadata | Other Metadata | |
| – | Content, Creator and Rights, Time and Place, Assignment | the metadata window's section headings (ADR 0017 U10-Q1); added by T123 |
| – | Rating and Label | the sixth section heading of the metadata window (ADR 0017 U10-Q1, U10c); added by T124. Its buttons use the terms of §2 (No stars, 1 star / {n} stars, Reject, the label names, No label) |
| – | Licence (PLUS) | the section heading of the PLUS release fields, Data Mining and Digital Source Type, between Assignment and Rating and Label (ADR 0017 U10-Q1, U10d-Q6); added by T128. Its field names are IPTC's (§1, `property_label`) |
| – | Unknown value: {value} | the extra entry of a choice list for a value of the file outside its vocabulary (U10d-Q4); added by T128 |
| – | {field} {value} is not in the PLUS list | the warning of a PLUS choice-list value outside its vocabulary (ADR 0007 §11); added by T128 |
| – | None, Not Applicable, Unlimited Model Releases, Limited or Incomplete Model Releases | the choices of Model Release Status (`MR-NON`, `MR-NAP`, `MR-UMR`, `MR-LMR`), the PLUS vocabulary's own labels (U10d-Q3); added by T128 |
| – | None, Not Applicable, Unlimited Property Releases, Limited or Incomplete Property Releases | the choices of Property Release Status (`PR-NON`, `PR-NAP`, `PR-UPR`, `PR-LPR`); added by T128 |
| – | Age Unknown, Age 25 or Over, Age 24 … Age 15, Age 14 or Under | the choices of Minor Model Age Disclosure (`AG-UNK`, `AG-A25`, `AG-A24` … `AG-A15`, `AG-U14`); added by T128 |
| – | Unspecified - no prohibition defined, Allowed, Prohibited for AI/ML training, Prohibited for Generative AI/ML training, Prohibited except for search engine indexing, Prohibited, Prohibited, see Other Constraints property, Prohibited, see Embedded Encoded Rights Expression property, Prohibited, see Linked Encoded Rights Expression property | the choices of Data Mining (the nine `DMI-` codes, in this order); added by T128 |
| – | Digital capture sampled from real life, Multi-frame computational capture sampled from real life, Digitised from a transparent negative, Digitised from a transparent positive, Digitised from a non-transparent medium, Human-edited media, Edited using Generative AI, Algorithmically-altered media, Digital creation, Data-driven media, Created using Generative AI, Pure algorithmic media, Screen capture, Virtual event recording, Composite of elements, Composite of captured elements, Composite including generative AI elements | the choices of Digital Source Type (the 17 current IPTC NewsCodes, in the vocabulary's order), IPTC's own English labels; added by T128 |
| – | Original media with minor human edits, Created by software, Digital art | the retired Digital Source Types (`minorHumanEdits`, `softwareImage`, `digitalArt`), shown only when a file has one; added by T128 |
| OK, Avbryt, Spara, Släng | OK, Cancel, Save, Discard | |
| Föregående, Nästa, Spara och nästa | Previous, Next, Save and Next | |
| Kopiera alla fält, Klistra in alla fält | Copy All Fields, Paste All Fields | the metadata window's buttons (ADR 0017 §2.3.1, U12b-Q5); added by T125. The Paste tooltip: "1 field from {file}" / "{n} fields from {file}" |
| Använd på alla markerade | Apply to {n} Marked; Apply to All Marked | the metadata window's button with two or more images marked, and the title of its confirmation (ADR 0017 §2.3.1, U12c); added by T126. Under the window's title: "1 image marked" / "{n} images marked" |
| – | Write, Writing {i} of {n} … | the confirmation's button and the progress of "Apply to All Marked"; added by T126. The question: "Write {k} changed fields to {n} images?" (singular "1 changed field", "1 image") |
| – | Changed: …, Emptied: …, Emptied only on this image: …, Skipped: {m} files | the confirmation's lines (U12c-Q4); added by T126. "Skipped: 1 file" in singular |
| – | Written to {w} of {n} images.; Unresolved conflict kept: {file} ({property}) | the summary of "Apply to All Marked" (U12c-Q6); added by T126. "Written to {w} of 1 image." in singular |
| Spara ändringarna? / Ändringar som inte sparades | Save changes? / Unsaved changes | |
| Sparar … / Sparat. | Saving … / Saved. | |
| Ändrad | Changed | |
| kontrollsumma | checksum | IIM digest (ADR 0007 §5.1) |
| teckenkodning | character encoding | |
| avkortat / kortades | truncated | |
| säkerhetskopia | backup | the `.pwbak` file (ADR 0007 §9.4) |
| skrivskyddad | read-only | |
| Inställningar | Settings | |
| Skapa IPTC-IIM för kompatibilitet med äldre system | Create IPTC-IIM for compatibility with older systems | ADR 0007 §5.2 quotes the Swedish text; it changes with the translation |
| Teckenkodning för äldre IPTC- och Exif-text | Character encoding for older IPTC and Exif text | |
| västeuropeisk | Western European | |
| sidofil, XMP-sidofil | sidecar, XMP sidecar | the `.xmp` file next to a RAW file; added by T117 |
| skrivlogg | write log | the write journal (T91); added by T117 |
| slutkontroll | final check | the check after a safe write (ADR 0007 §9.5); added by T117 |
| Registerpost, post-id, organisation | Registry entry, item ID, organization ID | the IPTC Registry Entry; added by T117 |
| Förslag till … | Suggested … | "Suggested Date Created from the camera"; added by T117 |
| Metadatafel | Metadata error | added by T117 |

## 7. Recurring sentence parts

| Swedish | English |
|---|---|
| Kunde inte … / kunde inte läsas / sparas / skrivas | Could not … / could not be read / saved / written |
| … saknas | … is missing |
| … är för gammal (minst {v}) | … is too old (at least {v} is needed) |
| … har stannat / har stoppats | … has stopped |
| … kön är full | … queue is full |
| Läser … | Reading … |
| arbetare (Importarbetaren, metadataarbetaren) | worker → in the UI "the import" / "metadata writing" (G6) |
| Exif-läsaren, Förhandsvisningsarbetaren | in the UI "Exif reading", "thumbnail loading" (G6; added by T117) |

## 8. Owner decisions G1–G8 (approved as proposed, 2026-11-13)

- **G1, IPTC names:** show IPTC's official names exactly, or the short forms above? Proposal:
  the official name, except that " (legacy)" is left out, the contact fields are Email, Phone and
  Web URL, and `xmp:Rating` is "Rating". Reason: "(legacy)" only says that a newer structure
  exists (Location Created), which the user cannot act on; "Email address(es)" is long for a
  field holding one address. The full name can be shown as a tooltip.
- **G2, `xmpRights:Marked`:** "Copyrighted" (Lightroom and Photo Mechanic; ADR 0007 D6) or
  "Copyright Status" (Adobe Bridge)? Proposal: Copyrighted.
- **G3, rating −1:** "Rejected" (proposal; Lightroom's word) or "Reject"?
- **G4, marked and selected:** "mark" for the set (Mark All, Unmark All, Marked only) and
  "selected" only for the current image? Proposal: yes, as today's Swedish keeps them apart.
  The alternative "select" for the set (as in most programs) would need another word for the
  current image.
- **G5, spelling:** British (colour, centre) or American (color, center)? Proposal: American,
  the more common UI convention, and the spelling of the IPTC and Adobe names.
- **G6, the word "worker":** error texts name internal parts ("Importarbetaren har stoppat").
  Proposal: say what stopped for the user ("Import has stopped") instead of "worker".
- **G7, keyboard names:** Ctrl, Shift, Alt, Esc are already English; no change.
- **G8, where the glossary lives:** `docs/glossary.md` (this file), referred to by rule 13 and
  `AGENTS.md`, and kept up to date by every task that adds a UI term.

## 9. Follow-up

Tasks T116 (move the remaining hard-coded texts into the text modules), T117 (translate the text
modules and update the tests) and T118 (uniform English error messages in the lower crates).
There is no language choice in the program for now; Swedish may come back later as a second
language through the same text functions.
