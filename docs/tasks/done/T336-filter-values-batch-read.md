# T336. ADR 0022 S2a: the filter's IPTC values, read for many images at once — Done

- **Why:** ADR 0022's filter needs each image's Title, Headline, Description, Keywords, Person
  Shown, Creator, Copyright Notice, City, Country and Location. The owner decided (ADR 0022 §3.1,
  S2-Q1, 2026-11-25) that they are read by a **new request** of the metadata worker beside
  `read_ratings` (which stays as it is), **the same way the metadata window reads them** (the
  reading and reconciliation of `ExifTool::read_metadata`, ADR 0007), but for many images per
  ExifTool call, so the filter sees exactly what the window shows. Every image is read, also a
  manufacturer RAW without a sidecar (S2-Q2 as amended). This task is the `pw-metadata` half of
  step S2 (`docs/tasks/plans/adr-0018-0025-steps.md`, row S2); T337 (S2b) uses it in the tabs.
  **Nothing in the program changes:** nothing sends the new request yet.
- **Crates:** `pw-metadata`, and one line in `pw-loupe`. **Files:**
  `crates/pw-metadata/src/filter_values.rs` (new, with unit tests in a `#[cfg(test)] mod tests`),
  `crates/pw-metadata/src/read.rs`, `crates/pw-metadata/src/lib.rs`,
  `crates/pw-metadata/src/worker.rs`, `crates/pw-metadata/tests/read.rs`,
  `crates/pw-loupe/src/metadata.rs` (one match arm, point 6; nothing else), and this task's own
  file (status and log).
- **Model:** Codex Astra — more than one crate, the metadata worker's request queue, and a change
  to the reader the metadata window uses (README, "Model" and "Codex tasks").
- **Read first:** `AGENTS.md`, `docs/tasks/README.md` (How to work on a task), `docs/TESTING.md`
  (§4.5: tests that need ExifTool), ADR 0022 §2.4 and §3.1 (read, do not edit), and
  `crates/pw-metadata/src/read.rs` as a whole: its module comment says where the metadata of each
  kind of file is (a RAW's sidecar, the file itself for DNG, JPEG and TIFF) and why the values are
  read as quoted JSON.
- **What:**
  1. **`read.rs`, the shared part.** Split `read_metadata_with_values` so that everything after
     the two ExifTool answers (the image's JSON object, and the sidecar's object for a RAW that
     has one) is a private function that takes the image's path (`VariableValues::of` needs it),
     those objects, the `Sidecars` found, the `FileKind` and `fallback`, and returns
     `(MetadataRead, VariableValues)`. The file notices, `qualified_lists` and `xmp_present` keep
     coming from exactly the objects they come from today.
     `read_metadata_with_values` then does exactly what it does today: `file_kind`, one
     `read_json` for the image, `find_sidecars`, one `read_json` for a RAW's plain sidecar, and
     the shared function. Its behaviour must not change: every existing test of `tests/read.rs`
     and of the crate passes unchanged.
  2. **`read.rs`, the batch read.** A new public method on `ExifTool`:
     `pub fn read_metadata_many(&mut self, images: &[PathBuf], fallback: Fallback) ->
     Vec<(PathBuf, Result<MetadataRead>)>`, one entry per image, in the order given, each
     image's path as given. It does:
     - an empty `images`: an empty answer, no ExifTool call;
     - per image, `file_kind` and `find_sidecars` as today (here before the call, since the
       sidecar call needs the sidecars; so a missing file may give `find_sidecars`' error instead
       of ExifTool's "not found" on systems other than Windows, which is accepted); an image whose
       kind or sidecar lookup fails gets that error and is left out of the calls. A path given
       twice is sent once and its answer given at both places;
     - **one** ExifTool call with `JSON_OPTIONS`, `IMAGE_TAGS` and every remaining image's path
       (as `image_args` builds it for one image), and **one** call with `JSON_OPTIONS` and the
       plain sidecar of every RAW image that has one (as `sidecar_args`), skipped when there is
       none;
     - each call's answer is split at `STDERR_MARKER` as `read_json` does and its standard output
       parsed as one JSON array; an empty standard output (ExifTool prints nothing when no file
       could be read) counts as an empty array. Each object is matched to its file by its `SourceFile` value
       (ExifTool writes it with `/` where the path had `\`): compare with every `\` turned into `/`
       and, on Windows, without regard to ASCII case. An object that matches no file sent is
       ignored;
     - an image whose image object is missing gets `Error::ExifTool` with the lines of the call's
       standard error that contain its whole path as sent (compared as `SourceFile` is; the
       whole path, not the file name, so the error of a missing `a.jpg` does not take the line of
       a missing `aa.jpg`; changed at the review, 2026-11-25), joined with a line feed, else
       `"no answer for {path}"`; a RAW whose sidecar was
       sent but whose sidecar object is missing gets the same kind of error. A failed call
       (`execute` returns an error, or the standard output is not a JSON array) gives every image
       of that call the error as text in `Error::ExifTool` (both calls' images when the first
       fails; the second call is then not made); for an `Error::ExifTool(m)` the text is `m`
       itself, so it does not read "ExifTool: ExifTool: …", for any other error its `Display`
       text;
     - every image with its objects goes through the shared function of point 1, so its
       `MetadataRead` is the one `read_metadata` would give (the `VariableValues` are dropped).
  3. **`filter_values.rs`** (new; module doc comment: "the values ADR 0022's filter reads, taken
     from a `MetadataRead`"): `pub struct FilterValues` with ten public fields of type
     `Vec<String>`: `title`, `headline`, `description`, `keywords`, `person_shown`, `creator`,
     `copyright_notice`, `city`, `country`, `location`; derives `Debug, Clone, Default,
     PartialEq, Eq`; and `pub fn from_metadata(read: &MetadataRead) -> FilterValues`. Each field
     is taken from `read.reconciled.metadata.get(<xmp id>)`'s `.value` (the value the metadata
     window shows; a conflict's other values are not used). `Location` is ADR 0022's "Location
     (Sublocation)": the table's "Sublocation (legacy)"; Person Shown is `PersonInImage` only
     (not `PersonInImageWDetails`):

     | Field | XMP id |
     |---|---|
     | `title` | `dc:title` |
     | `headline` | `photoshop:Headline` |
     | `description` | `dc:description` |
     | `keywords` | `dc:subject` |
     | `person_shown` | `Iptc4xmpExt:PersonInImage` |
     | `creator` | `dc:creator` |
     | `copyright_notice` | `dc:rights` |
     | `city` | `photoshop:City` |
     | `country` | `photoshop:Country` |
     | `location` | `Iptc4xmpCore:Location` |

     A `Value::Text` gives one element; a `Value::LangAlt` gives the text of every entry, in the
     entries' order; a `Value::List` gives its items in order; any other `Value` gives nothing.
     Texts are kept as stored (not trimmed); a text that is empty after trimming is left out. An
     absent property gives an empty `Vec`. No panics.
  4. **`lib.rs`:** `pub mod filter_values;` in its alphabetical place, and a new line
     `pub use filter_values::FilterValues;` beside the worker's `pub use`.
  5. **`worker.rs`:** a new request `Request::ReadFilterValues { id, images: Vec<PathBuf> }`, sent
     by `pub fn read_filter_values(&mut self, images: Vec<PathBuf>) -> Result<RequestId,
     SubmitError>` (as `read_ratings`, with a doc comment naming ADR 0022 S2-Q1), and answered by
     a new `Event::FilterValuesRead { id: RequestId, values: Vec<(PathBuf, Result<FilterValues,
     String>)> }` (doc comment: one entry per image sent, in order; an image's error as text).
     `handle` runs `read_metadata_many` with the worker's `options.fallback` (as
     `Request::ReadMetadata` does) and maps each `Ok` through `FilterValues::from_metadata`; when
     ExifTool is not running, every image gets the `unavailable()` text. `handle`'s `match` on
     `Request` gets the new arm. The module comment's list of requests gets one line for it. The
     two `match`es on `Event` in this file's tests that list the variants (in
     `reports_exiftool_start_errors_asynchronously` and in the helper `id_of`) get the new one.
  6. **`crates/pw-loupe/src/metadata.rs`:** in `MetadataCache::apply`, one arm
     `WorkerEvent::FilterValuesRead { .. } => {}` next to `WorkerEvent::RatingsRead { .. } => {}`,
     with the comment `// ADR 0022 S2: used from T337.` Nothing else in `pw-loupe`.
  7. **No panics** outside `#[cfg(test)]`: no `.unwrap()`, `.expect()`, `panic!`,
     `unreachable!` or direct indexing (`AGENTS.md` §3).
- **Tests to add:**
  - In `filter_values.rs` (no ExifTool; build a `MetadataRead` by `reconcile` from `Readings`, as
    `reconcile.rs`'s tests build them; `MetadataRead`'s fields are public and `FileState` has no
    `Default`, so fill it as for a JPEG with no IIM: `FileKind::Jpeg`, `CodedCharset::Absent`,
    empty values, `IimDigest::Missing`, `ExifPresent::default()`, `XmpPresent::default()`):
    1. every row of the table: a value set in the XMP gives that field and only that field;
    2. a Lang Alt with two languages gives both texts in order; a list gives its items in order;
    3. an empty text and a list item of spaces are left out; an absent property gives an empty
       `Vec`. (A `Value` of another kind cannot be stored for these properties, since the
       reconciliation refuses it; it is not tested.)
  - In `tests/read.rs` (they need ExifTool, as the file's other tests; use its helpers
    `exiftool`, `temp_dir`, `jpeg`, `write`):
    4. `read_metadata_many` over five images in **one** folder (made with `temp_dir`, the small
       JPEG written there with `std::fs::write(.., SMALL)`, since `jpeg()` makes a new folder per
       call) gives, for each, a `MetadataRead`
       equal to `read_metadata`'s for the same image: a JPEG with XMP values (Title, Keywords,
       City); a JPEG with **IIM values only** (written with `-IPTC:` tags, for example
       `-IPTC:City=Malmö` and `-IPTC:Keywords=a`); a RAW (`TN0001.ARW`, a few bytes, as
       `a_raw_file_reads_its_sidecar_and_ignores_a_sidecar_with_the_extension` makes it) with a
       sidecar; a RAW without a sidecar (`TN0002.ARW`); and a file name with `ä` and a space;
    5. a missing file among three: that image is `Err`, the two others are `Ok` and equal to
       `read_metadata`'s; an image with the extension `.txt` is `Err` (unsupported) and the
       others are read; two missing files only: both `Err`; two missing files whose names contain
       one another (`a.jpg` and `aa.jpg`, with a good image between them): each error is
       ExifTool's message (not `"no answer for …"`) and names only its own file; the same path
       twice: two equal answers; an empty list gives an empty answer;
    6. `FilterValues::from_metadata` of the IIM-only JPEG has the city and the keyword (the
       reconciliation's work reaches the filter);
    7. **timing, ignored** (`#[ignore = "timing: needs PW_FILTER_TIMING_DIR; run with --ignored
       --nocapture"]`), `the_filter_read_of_a_real_folder`: fails with an explanation when the
       environment variable `PW_FILTER_TIMING_DIR` is not set. It lists that folder's files
       (sorted by name) that `pw_metadata::supports_rating` accepts, **never writes there**, and
       prints: (a) for the first 50, the time of one `read_metadata_many` call (median of three)
       and per image; (b) for the same 50, the time of `read_metadata` one by one, in total and
       per image; (c) the whole folder in chunks of 50: the number of images, the total time and
       the slowest chunk, and the number of images that failed, each with its error (a failed
       image is counted and printed, never a panic). It asserts that (a) and (b) give equal
       `FilterValues` for every one of the 50 images; an image that fails in both counts as equal,
       one that fails in only one of them is a test failure.
  - In `worker.rs`'s tests: 8. `read_filter_values` with a missing ExifTool answers
    `Event::FilterValuesRead` with the request's id and every image `Err` containing
    `"could not start"` (as `reports_exiftool_start_errors_asynchronously` does for the ratings);
    an empty list answers with no entries.
- **Not in this task:** sending the request from the program, the tabs' values, the Exif values
  from `pw_raw::inspect`, the reading again after a write: T337. The chunk size (50) is T337's
  constant; this task's method takes any number. The search box and the status line: S3. An
  ExifTool that hangs on a file is not handled here (`execute` has no read timeout, as for every
  other request); T337 sends a file that failed once alone, so a crash costs only that file.
- **Do not touch:** any other file. In particular `read_ratings` and `read_folder_values` in
  `lib.rs` (unchanged), `pw-loupe` beyond point 6, `crates/pw-loupe/src/meta_filter.rs`, any ADR
  or `AGENTS.md`, `tools/`, `.github/`, every `Cargo.toml` (**no new dependency**) and
  `Cargo.lock`, `CHANGELOG.md`, and `docs/test-inventory.md` (**an exception to README rule 12:**
  the reviewer updates it after the merge; put the new tests' count and a one-line description
  of each in your report). If another file seems to need a change, stop and ask.
- **Rules:** README rules 1–11 and 13 apply, except that **this task overrides rule 1**: you work
  in the worktree `C:\Development\PhotoWorkflow-Codex` on the branch `codex`, not on
  `feature/gui-workspace`. **Never run `git commit`, `git add`, `git stash`, `git reset` or
  `git checkout`, and never create a branch or a checkpoint;** the reviewer brings your work over
  and commits. **Stop and ask the owner whenever something is unclear, contradicts the code or
  ADR 0022, or needs a file outside the list;** do not guess and do not leave a point out. Do not
  weaken a test to pass it. Build only in the worktree's own `target` folder. No personal data
  (user folders, e-mail addresses) in any file.
- **Report (as README rule 9, plus):** every file changed and why; the exact summary from
  `test-output.txt` after `.\tools\check.ps1` (if it cannot run in your sandbox, for example
  ExifTool not starting, say so, run `cargo fmt --all -- --check`, `cargo build --workspace` and
  `cargo test --workspace`, paste their summaries and which tests failed only for want of
  ExifTool, and say that `check.ps1` is left to the reviewer); the tests added with a one-line
  description each; any `cargo clippy` warning; every doubt and each place where you chose. The
  timing test is run by the reviewer on one of the owner's folders, not by you.
- **Done when:** `.\tools\check.ps1` is all OK (or as the report says), `git status` shows only
  the files above, and `git diff docs/tasks/` shows only the status and one new Log row. At the
  review: the reviewer runs test 7 on the owner's folder and brings the numbers to the owner,
  who confirms the chunk of 50 or chooses a smaller one for T337 (a slow chunk holds back the
  metadata window's read of the next image, since the worker answers one request at a time).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added; waits for the cold read and the chain review with T337. Cold read: the files suffice (the only exhaustive `WorkerEvent` match outside `pw-metadata` is `apply`; `batch_timing.rs` has a catch-all). Fixed: the shared function also takes the image's path; empty standard output is an empty array; a path given twice; which standard error lines name a file; `.value` of `get`; Location is "Sublocation (legacy)", Person Shown `PersonInImage`; the `pub use` as its own line; `handle`'s arm and the two test matches named; test 3's case of another `Value` kind dropped (the reconciliation refuses it); `FileState` filled by hand in unit tests; test 4 in one folder with `TN0002.ARW`; the sidecar lookup before the call accepted |
| 2026-11-25 | Blocked | Chain review (T336 → T337): the hand-over matches; T337 fixed (see its log). Here only the note that a hang is not handled |
| 2026-11-25 | Open | Cold reads and the chain review done; the Codex worktree moved to the tip of `feature/gui-workspace` |
| 2026-11-25 | Ready for review | Added the batch reader, reconciled filter values, worker request and tests. The reviewer runs the folder timing before choosing T337's chunk size. |
| 2026-11-25 | Review | Reviewer's review: brought over as a patch from the Codex worktree (`git apply --3way`, no conflict). `read_metadata_with_values` unchanged in behaviour (the shared `metadata_from_objects`); `read_metadata_many` makes at most two ExifTool calls, matches `SourceFile` as the task says, keeps an error to its own image (a failed first call fails all and skips the second), sends a path given twice once; `FilterValues` as the table; the worker request, its event and the `pw-loupe` arm as points 5–6; no panic paths outside tests. Test 6 is inside test 4 (accepted). `FilterValues` has no doc comments (accepted). `.\tools\check.ps1` all OK, run with T332's uncommitted work in the same tree (2765 passed, 98 ignored, 0 failed; clippy without warnings besides `raw-decode-bench`'s build note). Test 7 on the owner's folder `D:\PhotoWorkflowData\RAW` (79 images): 50 in one call 1.43 s (median of three), 28.7 ms per image; one by one 1.52 s, 30.3 ms per image; equal `FilterValues` for all 50; the whole folder in chunks of 50 2.41 s, slowest chunk 1.30 s. The owner chose a chunk of **10** (ADR 0022 S2-Q2 as amended, 2026-11-25; written into T337). Extra code review (one fresh review agent, read-only): nothing serious; the split faithful, the batch's `MetadataRead` equal for every kind of file. Its findings: (1) a standard error line is matched by the file name, so a missing `a.jpg` also takes the line of a missing `aa.jpg`, and test 5 cannot see it (its names do not contain one another, and the missing file's assert also passes on the fallback text); (2) a failed call's text is `Error::to_string()` wrapped in `Error::ExifTool` again: "ExifTool: ExifTool: …"; (3) test 7 panics on the first image that fails instead of printing; (4) `execute` waits 2 ms for standard error, so a late line (a batch's summary, a "File not found") can land in the next command's answer: an old race, harmless for the results (`known-limits.md`, ADR 0022). (1)–(3) are sent back; the task's point 2 and tests 5 and 7 are amended to say them. The patch was taken out of `feature/gui-workspace` again (nothing committed) |
| 2026-11-25 | Open | Sent back by the owner's choice for the review's findings (1)–(3), written into point 2 and tests 5 and 7 (each marked or worded as amended). **Keep your work in the Codex worktree** (it was not moved) and change only what those three need; then check, report and set "Ready for review" as before |
| 2026-11-25 | Ready for review | Addressed review findings 1–3: match diagnostics by the sent path, preserve ExifTool's error text, test overlapping names, and report folder read failures while comparing batch and individual outcomes. |
| 2026-11-25 | Review | Reviewer: approved. Only `crates/` brought over as a patch (`git apply --3way`, no conflict; the task file's rows copied by hand). (1) a standard error line belongs to an image when its part after the first `" - "` (ExifTool's "Error: … - <path>") equals the image's path as sent, compared as `SourceFile` is; a line without `" - "` falls back to containing the path: stricter than point 2 asked (a message with `" - "` of its own before the path would fall back to `"no answer for …"`, `known-limits.md`); test 5's `a.jpg`/`aa.jpg` case checks ExifTool's message and that each names only its own path; (2) an `Error::ExifTool(m)` keeps `m`; (3) test 7 counts and prints failures and compares (a) with (b) as results. `.\tools\check.ps1` all OK: 2765 passed, 98 ignored, 0 failed; clippy without warnings besides `raw-decode-bench`'s build note. Test 7 again on `D:\PhotoWorkflowData\RAW` (79 images): 50 in one call 1.32 s, 26.3 ms per image; one by one 1.48 s, 29.5 ms per image; equal `FilterValues`; the whole folder 2.18 s, slowest chunk of 50 1.26 s, 0 failed |
| 2026-11-25 | Done | Committed. Leftovers: the late standard error line and a message with `" - "` of its own in `known-limits.md` (Filter on text and metadata); no doc comments on `FilterValues` accepted; `docs/test-inventory.md` updated by the reviewer (unit 568, `tests/read.rs` 22 with 3 ignored). No `CHANGELOG.md` line: nothing a user sees changes |
