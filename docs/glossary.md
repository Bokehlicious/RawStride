# Glossary: the user interface in English

- Module: `pw-loupe` (the texts the user sees), with the property names of `pw-metadata`
- Question: which English word replaces each Swedish UI term, so that English can become the
  interface's base language (owner, 2026-11-13) and every later task translates the same way.
- Status: **approved by the owner (2026-11-13)**, with G1–G8 (§8) as proposed. English is the
  interface's base language (GUI_REQUIREMENTS GR14); Swedish texts are translated with these terms.
- Kept up to date: a task that adds a UI term not listed here adds it to this file
  (`docs/tasks/README.md`, rule 13).
- Source: every string literal in the non-test code of `crates/pw-loupe/src/` (commit `76f99f2`),
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
| `…:Sublocation` | – | Sublocation | Sublocation (T136; in a Location Created or Location Shown entry) |
| `…:City` | – | City | City (T136; in a location entry) |
| `…:ProvinceState` | – | Province or State | Province or State (T136; in a location entry) |
| `…:CountryName` | – | Country Name | Country Name (T136) |
| `…:CountryCode` (location) | – | Country ISO-Code | Country ISO-Code (T136) |
| `…:WorldRegion` | – | World Region | World Region (T136) |
| `…:LocationId` | – | Location Identifier | Location Identifier (T136; its URIs on one line separated by commas) |
| `plus:ImageCreator` | Bildskapare | Image Creator | Image Creator |
| `plus:CopyrightOwner` | Upphovsrättsinnehavare | Copyright Owner | Copyright Owner |
| `plus:Licensor` | Licensgivare | Licensor | Licensor |
| `Iptc4xmpExt:Genre` | – | Genre | Genre (T139; a list of CV terms in Assignment) |
| `…:CvTermName`, `…:CvTermId`, `…:CvId` | – | CV-Term name, CV-Term ID, CV-Term CV ID | the same (T139; in a Genre entry; IPTC's table writes "name" in lower case) |
| `plus:ImageSupplier` | – | Image Supplier | Image Supplier (T139) |
| `plus:ImageCreatorName`, `…ID`; `plus:CopyrightOwnerName`, `…ID`; `plus:ImageSupplierName`, `…ID` | – | Image Creator Name, Image Creator ID; Copyright Owner Name, Copyright Owner ID; Image Supplier Name, Image Supplier ID | the same (T139; in an entry of the list) |
| `plus:LicensorName`, `…ID`, `…Email`, `…URL`, `…Telephone1`, `…TelephoneType1`, `…Telephone2`, `…TelephoneType2` | – | Licensor Name, Licensor ID, Licensor Email, Licensor URL, Licensor Telephone 1, Licensor Telephone Type 1, Licensor Telephone 2, Licensor Telephone Type 2 | the same (T139; in a Licensor entry) |
| `plus:LicensorStreetAddress`, `…ExtendedAddress`, `…City`, `…Region`, `…PostalCode`, `…Country` | – | Licensor Address, Licensor Adress Detail, Licensor City, Licensor State or Province, Licensor Postal Code, Licensor Country | the same (T139; under "Address" in a Licensor entry; "Adress" is the spelling of IPTC's table) |
| `Iptc4xmpExt:PersonInImage` | Person i bilden | Person Shown in the Image | Person Shown in the Image |
| `Iptc4xmpExt:PersonInImageWDetails` | – | Person Shown in the Image with Details | the same (T143; a list in Content after Person Shown) |
| `…:PersonName`, `…:PersonId`, `…:PersonDescription` | – | Name, Identifier, Description | the same (T143; in a Person Shown with Details entry; its Characteristics are not shown and kept) |
| `Iptc4xmpExt:ProductInImage` | – | Product Shown in the Image | the same (T143; a list in Content) |
| `…:ProductName`, `…:ProductGTIN`, `…:ProductId`, `…:ProductDescription` | – | Name, GTIN, Identifier, Description | the same (T143; in a Product Shown entry) |
| `Iptc4xmpExt:ArtworkOrObject` | – | Artwork or Object in the Image | the same (T143; a list in Content) |
| `…:AOTitle`, `…:AOCreator`, `…:AODateCreated`, `…:AOSource`, `…:AOSourceInvNo`, `…:AOCopyrightNotice` | – | Title, Creator, Date Created, Source, Source Inventory Number, Copyright Notice | the same (T143; open in an Artwork or Object entry) |
| `…:AOCircaDateCreated`, `…:AOStylePeriod`, `…:AOContentDescription`, `…:AOContributionDescription`, `…:AOPhysicalDescription`, `…:AOCreatorId`, `…:AOCurrentCopyrightOwnerName`, `…Id`, `…:AOCurrentLicensorName`, `…Id`, `…:AOSourceInvURL` | – | Circa Date Created, Style Period, Content Description, Contribution Description, Physical Description, Creator ID, Current Copyright Owner Name, Current Copyright Owner ID, Current Licensor Name, Current Licensor ID, Source Inventory URL | the same (T143; under "More" in an Artwork or Object entry) |
| `Iptc4xmpExt:Contributor` | – | Contributor | the same (T143; a list at the end of Creator and Rights) |
| `…:Name`, `…:Role`, `xmp:Identifier` | – | Name, Role, Identifier | the same (T143; in a Contributor entry) |
| `Iptc4xmpExt:AboutCvTerm` | – | CV-Term About Image | the same (T143; a list in Assignment right after Genre, with Genre's three sub-fields) |
| `Iptc4xmpExt:RegistryId` | – | Image Registry Entry | the same (T143; a list at the end of Assignment) |
| `…:RegOrgId`, `…:RegItemId`, `…:RegEntryRole` | – | Organisation Id, Item Id, Role | the same (T143; in an Image Registry Entry; IPTC's names, U11c-Q1; the warning of T117 keeps "item ID" and "organization ID") |
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
| – | Sort By, Sort: {key}; File Name, Capture Time, Modified, File Type, Rating; Ascending / Descending (↑ / ↓) | the sort order in View and the command bar (ADR 0017 U14-Q1, U14-Q7); added by T138 |
| – | Sorting by capture time: {i} of {n} | the status bar while a tab's capture times are read (U14-Q4); added by T138 |

## 3. Window, menus and commands

| Swedish | English | Note |
|---|---|---|
| Arkiv, Redigera, Bild, Visa | File, Edit, Image, View | menu titles (Windows convention) |
| Öppna mapp… / Importera… / Stäng flik / Inställningar… / Avsluta | Open Folder… / Import… / Close Tab / Settings… / Exit | |
| – | Help; About RawStride… | the fifth menu and its one item, which opens the window "About RawStride" (ADR 0031 RL-Q9); added by T225 |
| Nästa flik / Föregående flik | Next Tab / Previous Tab | |
| Bibliotek, Uppgifter, Inspektör | Library, Tasks, Inspector | column titles |
| Sidokolumner, Vänster kolumn | Side Columns, Left Column | |
| Fäll ihop kolumnen / Visa kolumnen | Collapse Column / Show Column | |
| Helskärm | Full Screen | |
| – | Buttons on Thumbnails | the View menu's check item after Full Screen, on by default, and the four buttons on a thumbnail in the contact sheet it shows (Rotate Left, Rotate Right, Metadata…, Preview; ADR 0017 §2.8); added by T245 |
| Tema, Mörkt, Ljust | Theme, Dark, Light | |
| Tom (flik) | Empty | |
| Läge: RAW / JPEG | Mode: RAW / JPEG | |
| Storlek | Size | thumbnail size |
| mapp | folder | |
| – | Folders, Favourites, Recent | the Library's sections in the left column (ADR 0017 U4-Q7); added by T135 |
| – | Add to Favourites / Remove from Favourites / Clear Recent Folders | the right-click menus of a folder in the tree, a folder tab, a favourite and the Recent section, and the File menu item (U4-Q3, U4-Q4); added by T135 |
| – | Refresh | reads the drives again (also F5 with the pointer over the left column; F5 elsewhere is Reload Folder); added by T135 |
| – | Reload Folder | the File menu item and F5 elsewhere than over the left column: the active tab's folder listed and read again (ADR 0026 F1); added by T241 |
| – | {letter}: (no disk) | an empty card reader or DVD drive in the folder tree (U4-Q1); added by T135 |
| – | Folder not found | the hover text of a favourite or recent folder that no longer exists, and an expand that fails because the folder is gone; added by T135 |
| – | No tasks, Stop, ×, Errors ({n}), Clear | the Tasks area under the Library (ADR 0017 U5-Q1–U5-Q5): the line with nothing to show (the text is still "No tasks in progress."), the running import's button, the finished import's button (hover "Remove this line"), the error list's heading and its button; added by T140 |
| – | {source} → {destination}; Importing {i} of {n}: {file}; Import done: {n} files, {f} failed / Import cancelled: {n} files, {f} failed | the import's row in the Tasks area (U5-Q2; hover "Show the import window"); without failures "Import done: {n} files" as before; added by T140 |
| – | Writing ratings: {n}; Writing rotations: {n}; Saving metadata: {i} of {n}; Saving metadata | the pending writes in the Tasks area: rating writes, rotation writes, "Apply to All Marked" and the metadata window's own write (U5-Q4); added by T140; changed by T239 |
| – | Recycle Bin | Windows' own name, not translated by RawStride (ADR 0018); added by T187 |
| – | Delete to Recycle Bin | the menu item (key Delete) and the title of its confirmation (ADR 0018 B4, T189); added by T187 |
| – | Delete Rejected Images | the menu item that deletes the rejected images of the active tab's folder (ADR 0018 B5, T190); added by T187 |
| – | RAW+JPEG pair | an image's RAW and JPEG files of the same name, deleted together (ADR 0018 B-Q3); added by T187 |
| – | Delete | the confirmation's button that deletes (ADR 0018); its other button is "Cancel" (§6); added by T187 |
| – | Open in {name} | the Image menu's item per program in the list of external programs, opening the chosen images in that program (ADR 0019 §2.3, E-Q3); added by T201 |
| – | Open in Another Program… | the Image menu's item while no external program is set up (ADR 0019); added by T201 |
| – | Waiting for writes … | the status line while "Open in {name}" waits for the images' rating or metadata writes (ADR 0019 §2.4, E-Q6); added by T201 |
| – | Open | the button of the question asked before more than 100 images are opened in a program (ADR 0019 E-Q8); its other button is "Cancel" (§6); added by T201 |
| – | command line | the program's path and the images' paths it is started with; Windows' own limit is 32 767 characters, and "Open in {name}" says so when the images do not fit ("… the command line would be longer than Windows allows.", ADR 0019); added by T201 |
| – | Show in File Explorer | the Image menu's item, no key, opening the current image's folder in File Explorer with the file selected (ADR 0019 E3, E-Q11); it uses Windows' program's name, "File Explorer" (§5; owner, 2026-11-19, ADR 0019 §2.8); added by T203 |

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
| – | Rotate Left / Rotate Right (turn the image 90° anticlockwise / clockwise; for a RAW the turn is written to its sidecar, for a JPEG, TIFF or DNG to the file's Exif orientation; the pixels are never changed; ADR 0024; added by T236) |
| – | rotation (the turn RawStride shows and writes: "Could not save the rotation for {name}: {error}", "Rotations cannot be saved for this file type."; ADR 0024; added by T236) |

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
| Dölj, Stäng | Hide, Close | |
| IPTC-mall, Mallen, Variabler | IPTC Template, the template, Variables | added by T117 |
| – | Metadata Template | the template window's title ("Metadata Template – {file}" once a file is loaded or saved) and the Save and Load dialogs' file type (ADR 0017 U13c); added by T291 |
| – | Clear Form, Load…, Save…, Close | the template window's buttons (ADR 0017 U13c); added by T291 |
| – | the + box ("+") | the box beside a checked list of plain text values in the template window: "Add the template's entries after the image's own instead of replacing the list." (U13-Q12); the check box of each field: "Use this field in the template."; added by T291 |
| – | Discard the changes to the template?, Discard, Cancel; {file} already exists. Replace it?, Replace | the template window's questions before a Load or Close loses changes (U13-Q15) and before a save over a file whose name got `.xmp` added; added by T291 |
| – | Metadata Template:, Choose…, Edit…, None; Choose Metadata Template; Reading the metadata template…; the template reader | the import dialog's template row and its buttons (`None` also the row's text without a template), the open dialog's title, the line shown while the file is read at Start, and the thread that reads it ("The metadata template {file} could not be read: the template reader has stopped."; ADR 0017 U13c, U13-Q6, U13-Q17); added by T295 |
| – | Use for This Import; {file} (changed for this import); Not saved (this import only) | the template window's button, shown when it was opened from the import dialog, that hands the form to that import without saving a file, and the import dialog's template row while such a change applies, with the chosen file's name or without a file (ADR 0017 U13-Q21); added by T298 |
| enhet | drive | "The source is not a drive"; added by T118 |
| mata ut, utmatat | eject, ejected | "E: has been ejected and can be removed."; added by T118 |
| Säker borttagning av maskinvara | Safely Remove Hardware | Windows' own name; "Use Safely Remove Hardware in the taskbar."; added by T156 |
| Utforskaren | File Explorer | Windows' own name; added by T118 |
| fullständig sökväg | full path | "The source must be a full path: …"; added by T118 |
| Kopiera bara nya bilder / importerade tidigare | Copy new images only / imported earlier | the import dialog's check box and the files the card's ingest register leaves out ("3 files imported earlier were left out."); added by T158 |
| Markera kortet som importerat | Mark Card as Imported | the start button when nothing is copied (T166) |

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
| – | Load Template…, Save as Template… | the metadata window's buttons after Paste All Fields (ADR 0017 U13-Q22); added by T301. A refused save: "No field shown has a value to save as a template." / "Not saved as a template: {reason}" |
| Använd på alla markerade | Apply to {n} Marked; Apply to All Marked | the metadata window's button with two or more images marked, and the title of its confirmation (ADR 0017 §2.3.1, U12c); added by T126. Under the window's title: "1 image marked" / "{n} images marked" |
| – | Write, Writing {i} of {n} … | the confirmation's button and the progress of "Apply to All Marked"; added by T126. The question: "Write {k} changed fields to {n} images?" (singular "1 changed field", "1 image") |
| – | Changed: …, Emptied: …, Emptied only on this image: …, Skipped: {m} files | the confirmation's lines (U12c-Q4); added by T126. "Skipped: 1 file" in singular |
| – | Written to {w} of {n} images.; Unresolved conflict kept: {file} ({property}) | the summary of "Apply to All Marked" (U12c-Q6); added by T126. "Written to {w} of 1 image." in singular |
| – | Add Location, Remove, Copy from Location Created | the buttons of the location lists in Time and Place: under each list, on each entry, and under Location Shown (ADR 0017 U11a-Q2, U11a-Q4, U11a-Q6); added by T136. "Copy from {list}" takes the list's name from `property_label` |
| – | Add {list}, At most {n} entries, Copy from Creator, Address | the structure lists of T139 (ADR 0017 U11b-Q6): "Add Genre", "Add Image Creator", … under each list (the locations keep "Add Location"; "Remove" on each entry as there); the tooltip of a disabled Add at the list's limit ("At most 1 entry" in singular); the button under Image Creator (U11b-Q4; "Creator" from `property_label`); the collapsed line over a Licensor entry's address (U11b-Q3); added by T139 |
| – | Work, Cell, Fax, Home, Pager | the telephone types of a Licensor entry (PLUS `work`, `cell`, `fax`, `home`, `pager`), the LDF file's labels with a capital letter; an unknown value is "Unknown value: {uri}"; added by T139 |
| – | Add {list}, More, Copy from Person Shown | the structure lists of T143 (ADR 0017 U11c-Q8): "Add Person Shown in the Image with Details", "Add Product Shown in the Image", "Add Artwork or Object in the Image", "Add Contributor", "Add CV-Term About Image", "Add Image Registry Entry" under each list; the collapsed line over an Artwork or Object entry's eleven further sub-fields (U11c-Q3); the button under Person Shown with Details (U11c-Q6), shown as "Copy from Person Shown in the Image" because the name comes from `property_label`, as "Copy from Creator"; added by T143 |
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
| – | Recent, My Values, Add to My Values, No values yet, ▼, × | the list of recent and saved values beside a one-line field of the metadata window (ADR 0017 U12a-Q5, U12a-Q7): its two headings, its last entry and its line with no values; the button "▼" (tooltip "Recent and saved values", also opened with Alt+↓) and "×" beside an own value, which removes it; added by T141 |
| – | Histogram, RAW data, Embedded JPEG, TIFF, Clipped: R {r} %  G {g} %  B {b} %, Deep shadows: {s} %, No histogram for this file | the Histogram section of the preview's right column and of the inspector (ADR 0017 §2.6, U7-Q7, U7-Q8): its heading; under the histogram where the numbers come from (the unpacked RAW data, the embedded JPEG or a JPEG file, the TIFF file's image), the share of each channel's pixels at the clipping level and the share of the pixels in the deepest shadows, in percent with two decimals; the line for a file with no statistics. While the statistics are read the section shows "Reading …" (§7); added by T142 |
| – | Clipping, Clipped highlights (J), Deep shadows (Shift+J), No clipping map for this file; Show Clipped Highlights, Show Deep Shadows | the clipping overlays of the preview (ADR 0017 §2.6, U7b-Q5, U7b-Q9, U7b-Q10): the Clipping section of the preview's right column (its heading, the check boxes of the two overlays with their keys, and the line for a file with no clipping map; while the map is made the section shows "Reading …"), and the two items of the View menu (J and Shift+J). "Clipped highlights" are the pixels at the clipping level and "Deep shadows" the deepest shadows, as in the Histogram section (T142's row above); the "clipping map" is the map of both that the marks on the image are drawn from; added by T148 |
| – | Focus marking, Show Focus Marking | edges of high local contrast marked in green on the image in the preview (ADR 0017 §2.6, U9-Q1–U9-Q4); "Show Focus Marking" (E) in the View menu, ticked when on; added by T168 |
| – | Compare, Held image | comparing two images in the preview (ADR 0017 §2.2, U6; U6-Q4–U6-Q6, U6-Q12): "Compare" is the mode that shows two images side by side or above and below, and its item of the View menu (C); the "held image" is the one that stays in place (left or on top) while the current image (right or below) is stepped, rated and labelled as in the preview. Each side has a line "{file} · {source} · {zoom}%" at its top, with the source named as in the status line (RAW rendering, Embedded JPEG, JPEG file, TIFF file) and "(Fit)" after the percent at fit; added by T151 |
| – | Hold Current Image, Swap Images, Above and Below | the compare's other items of the View menu (ADR 0017 §2.2, U6-Q6, U6-Q7, U6-Q12), after "Compare" (C): "Hold Current Image" (K) makes the current image the held image; "Swap Images" (Shift+K) lets the held and the current image change places; "Above and Below" (V) shows the two images above and below each other instead of side by side, ticked when on. The three are greyed outside the compare; added by T153 |
| – | Lock Zoom and Position | the compare's lock, an item of the View menu (ADR 0017 §2.2, U6-Q8, U6-Q12) after "Above and Below" (L): when on, the held image shows the same zoom level and the same point of the image as the current image, and the mouse on either side moves both; ticked when on, remembered, greyed outside the compare; added by T154 |
| – | `_rejected`, `_picked` | the subfolders, in the image's own folder, that rejected (X) and picked (P) images are moved to with their companion files when "Move Rejected and Picked Images to Subfolders" is on (ADR 0017 §2.6, U9-Q5–U9-Q9); folder names, not translated; in a folder of either name nothing is moved (U9-Q9); added by T172 |
| – | move back | what an undo does to the files of a move (ADR 0017 U9-Q5–U9-Q9; the undo is U9-Q8): "1 image moved back.", "{count} images moved back.", "{file} could not be moved back: {reason}", "There is no move to undo."; a failed move is "{file} could not be moved: {reason}"; added by T172 |
| – | Move Rejected and Picked Images to Subfolders, Move to Picked, Undo Move | the moves' menu items (ADR 0017 U9-Q5–U9-Q9): in the Image menu, the check box "Move Rejected and Picked Images to Subfolders" (off by default, remembered); "Move to Picked" (P), greyed while the check box is off; "Undo Move" (Ctrl+Z), the first item of the Edit menu, which undoes moves only. Named by T172, the items come with T174 |
| – | qualifier | an XMP qualifier on a value, for example on one keyword of Keywords (not the language `xml:lang` of a Lang Alt entry); a list whose items have qualifiers is shown but never written (ADR 0007 §8): "{field} has qualifiers RawStride cannot keep, so it is shown but never changed" in the info panel, "{field} was not written: it has qualifiers RawStride cannot keep" after a write; added by T193 |
| – | External Programs | the heading of the Settings window's section with the list of programs the chosen images can be opened in, at most five, the first the default program (ADR 0019 §2.1, E-Q1, E-Q2); added by T200. Also the name of the Settings window's page with that list, which Ctrl+E opens while the list is empty (ADR 0017 U10b-Q6); added by T278 |
| – | Add… | the button under the list of external programs in the Settings window, which opens the file dialog "Choose a Program" (ADR 0019 E-Q2); added by T200 |
| – | Move Up | the button on each row of the list of external programs in the Settings window, which moves the program one place up (greyed on the first row; ADR 0019 E-Q2); added by T200. Also the button before each section and each item on the Settings window's page Metadata Window, which moves it one place up (an item inside its section; greyed on the first; ADR 0017 U10b-Q6); added by T278 |
| – | No programs added. | the line in the Settings window's section External Programs while the list is empty (ADR 0019 E-Q2); added by T200 |
| – | Choose a Program | the title of the file dialog opened by "Add…" in the section External Programs (ADR 0019 E-Q2); added by T200 |
| – | Programs | the name of the `.exe` filter of the file dialog "Choose a Program" (ADR 0019 E-Q2); added by T200 |
| – | At most {n} programs | the tooltip of the disabled "Add…" when the list of external programs is full ("At most 1 program" in singular; ADR 0019 E-Q1); added by T200 |
| – | General | the Settings window's first page, with the IIM check box, the encoding and the theme; the window opens on it from Settings… (ADR 0017 U10b-Q6); added by T278. The field Companion File Suffixes added by T281 |
| – | Metadata Window | the Settings window's page where the user chooses which sections and items of the metadata window are shown, and in which order; what is hidden is shown read-only under Other Metadata (ADR 0017 §2.3.1, U10b-Q2–U10b-Q6); added by T278 |
| – | Restore Defaults | the button under the Settings window's page Metadata Window that sets every section and item back to shown in the default order (not the sections' open or collapsed state); applied by Save (ADR 0017 U10b-Q6); added by T278. Also the button under the page Keyboard Shortcuts that gives every action its default keys back (the whole map), applied by Save (ADR 0017 U8-Q7); added by T281 |
| – | Keyboard Shortcuts | the Settings window's fourth page, where each changeable action's two keys are changed or cleared, the actions under the names of their menus (File, Edit, Image, View); its names are the menu texts, "Open in First Program" for the first external program's key and "Marked only" (ADR 0017 §2.4, U8-Q7); added by T281 |
| – | Action, First Key, Second Key | the headings of the page Keyboard Shortcuts' three columns: the action's name, then each of its two key slots with Change and Clear. The first key is the one the menus show (U8-Q2); the Action column's width is changed by dragging the line between Action and First Key and kept between sessions (ADR 0017 U8-Q15); added by T286 |
| – | Change, Press a key… | the button of a key slot on the page Keyboard Shortcuts, which waits for the next key press; while it waits the slot shows "Press a key…". Escape, a click elsewhere, another page, Save and Cancel end the wait. A refused key is named under the row: "Ctrl+Alt is not allowed", "Shift, Ctrl or Alt alone is not allowed", "This key cannot be changed", "Used by {action}" (U8-Q4, U8-Q5, U8-Q7); added by T281 |
| – | Clear (a key) | the button of a key slot on the page Keyboard Shortcuts, which takes the slot's key away; an empty slot shows "–" (U8-Q7). Not the clearing of a field, a filter or a marking; added by T281 |
| – | The keyboard shortcuts could not be saved: {error} | the warning when `keys.txt` cannot be written; the keys still apply until RawStride closes (U8-Q6); added by T281 |
| – | Companion File Suffixes | the field on the Settings window's page General with the suffixes, separated by commas, of the companion files that move with an image; the note under it: "Files with these suffixes move with an image; empty means the default" (`dop, pp3, on1`; ADR 0017 U8-Q10, U9-Q10); added by T281 |

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
