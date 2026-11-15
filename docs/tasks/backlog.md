# Not for agents yet

These need design decisions, new dependencies or platform-specific code, and stay with the owner
and reviewer:

- A timeout when ExifTool hangs (process supervision across threads). Today only the shutdown has a
  timeout (`SHUTDOWN_TIMEOUT` in `pw-metadata`'s `exiftool.rs`).
- ADR 0007 (IPTC metadata): milestone 8 is done (I1–I13, 2026-11-14; what is left is in ADR 0007
  §18). The `Cleared` task waits for D1 (ADR 0007 §17). Also later:
  Exif ImageEditor ↔ Contributor and GPS → Location Created (§6.1), left out of T81; keyword
  qualifiers (§8), left out of T83; from §10, left out of T88: template sub-fields of lists of
  structures (Location Created, Image Creator, Copyright Owner, Licensor), variables (`{date}` …),
  template files in XMP with the modes in a RawStride namespace (needs the same kind of namespace
  decision as D1). Left out of I10: the other controlled vocabularies of §11; editing several
  images at once and lists of structures are now ADR 0017 U12 and U11.
- **Future improvement (owner, 2026-11-12, after D11):** Swedish names for **every** IPTC property
  and structure field in T102's name table, so the Swedish info panel and editor show no English
  IPTC names. Today D11 names about 35 of them; the rest fall back to IPTC's English name. Needs
  the owner's approval of the full list.
- **CI before the repository becomes public** (owner, 2026-11-14): a GitHub Actions workflow
  (`.github/workflows/ci.yml`) on `windows-latest` that installs the pinned Rust toolchain, runs
  `vcpkg install` with vcpkg's binary cache (LibRaw built from source takes long), puts ExifTool
  (Windows package, 12.67 or newer) on `PATH`, and runs `.\tools\check.ps1` without `-Ignored`;
  `test-output.txt` kept as an artifact. Wait while the repository is private: Windows minutes
  count double against the free quota, and `check.ps1` already runs before every commit. While
  private, run it only by hand (`workflow_dispatch`) and on pull requests to `main`; on every push
  once public. The first run also tests `docs/building.md` on a clean machine.
- **Review the git history before the repository becomes public** (owner, 2026-11-14): go
  through every commit on every branch to be published (not only the current files) for things
  that should not be public: personal data (names, e-mail addresses, paths with user names),
  keys, tokens or passwords, private test images and their metadata, copyrighted reference specs
  (for example the IPTC files kept outside the repository), and large binaries. Decide per finding
  whether to remove it from the history (a history rewrite of all branches, which changes every
  commit hash and needs the owner's approval) or to leave it; a tool such as `gitleaks` for the
  secrets part would be a new dependency. Do this before the CI item above makes the repository
  public.
- **A second language: a text catalog with keys** (owner, 2026-11-14). Only when a second language
  is wanted (GR14: no language choice for now). Done already by T116–T121: every text the user
  sees is made by a function in a text module (rule 13), whole sentences from one template, the
  tests check the texts through those functions, and one test keeps Swedish letters out of the
  code. Left: one catalog with keys in place of the text modules; typed kinds instead of finished
  English sentences from the lower crates (`pw-ingest`'s form checks and eject results, the error
  texts of `pw-metadata`, `pw-viewcore`, `pw-thumbcache`); the tests then check the key or the
  catalog, no weaker than today. Needs an ADR first: a library (Fluent, rust-i18n; a new
  dependency, the owner's approval) or our own simple table. The Swedish names of the item above
  would become the second language's catalog.
- **Leftovers from the review of T142–T145 (2026-11-15):**
  - *A wrong Artwork Date Created* (T143, noted, not changed): with a date that is not a valid IPTC
    date, OK writes nothing and the reason is shown only under that box; "Copy All Fields" then
    copies the file's earlier date. Needs the owner's view on how the window should tell it.
  - *RAW statistics test files* (T142): no test file covers X-Trans, a black level per position,
    or three- or four-colour RAW data. Needs sample files from the owner in `PW_TESTDATA`
    (`docs/test-inventory.md`, `statistics.rs`).
  - *The 100 % view first, measured* (T144, ADR 0017 U7c-Q5): `latency.ps1`'s 100 % run steps to a
    neighbour whose 100 % view is already prefetched, so it cannot show the gain of U7c-Q5. A
    measurement of Z pressed right after an image is opened would need a change to the script, so
    a task first (tool changes go through `docs/tasks/`).
- **A filter bar of toggle buttons instead of drop-down lists** (owner idea, 2026-11-10, after T61;
  ADR 0014 §15): a row "0★ 1★ 2★ 3★ 4★ 5★" and a row of colour swatches, each clicked on and off,
  with a check box per row that turns the row on or off. Needs an amendment of ADR 0014 first: the
  star part would become a set of star values instead of the ≥ / = / ≤ condition (T60).

Removed on 2026-11-13 because they are done or planned elsewhere: ADR 0010 steps M3–M7 (done,
T30–T40); ADR 0011 step K4 (done, T26–T29); moving rejected or picked images to `_rejected` /
`_picked` (ADR 0013 §13), now ADR 0017 U9 (§2.6, U-Q6); the import dialog fields for title and description in the Tauri
app's `ui/` (owner: dropped, the Tauri app is retired at the switch, GR8). Removed on 2026-11-14:
"Same folder, two tabs" (done, T119); suppressing the Windows "no disk in drive" dialog (done, T135: `SetErrorMode` at start).
