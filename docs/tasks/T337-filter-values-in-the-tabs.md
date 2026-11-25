# T337. ADR 0022 S2b: the filter's values kept per tab and read when needed — Open

- **Why:** step S2 of ADR 0022 (`docs/tasks/plans/adr-0018-0025-steps.md`, row S2), the
  `pw-loupe` half after T336. Each tab keeps, for its images, the values of S1's filter model
  (`crates/pw-loupe/src/meta_filter.rs`, T330): the Exif values from the reader the sort already
  uses, and the IPTC values from T336's request, read only while the tab's filter needs them and
  read again after RawStride's own writes. Decided by the owner (ADR 0022 §2.4, §3.1, S2-Q1–S2-Q7,
  2026-11-25). **Nothing on screen changes:** nothing sets a tab's new filter until S3 (the
  search box), so the reads never start in use; the tests drive them.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/filter_values.rs` (new, with its tests in
  a `#[cfg(test)] mod tests`), `crates/pw-loupe/src/sort.rs`, `crates/pw-loupe/src/tabs.rs`,
  `crates/pw-loupe/src/browse.rs` (point 3 only), `crates/pw-loupe/src/metadata.rs`, `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/lib.rs`,
  and this task's own file (status and log).
- **Model:** Codex Astra — reads in flight while the user keeps working, a request queue shared
  with the metadata window, and a read that must not overtake a write (README, "Model").
- **Read first:** `AGENTS.md`, `docs/tasks/README.md` (How to work on a task), `docs/TESTING.md`,
  ADR 0022 §2.4 and §3.1 (read, do not edit), `meta_filter.rs` (S1's model: `Field`, `Group`,
  `Value`, `ImageValues`, `Filter::needs`), and in `pw-metadata` T336's
  `FilterValues`, `MetadataWorker::read_filter_values` and `WorkerEvent::FilterValuesRead`.
- **What:**
  1. **`filter_values.rs`** (new; module doc comment: ADR 0022 S2, the values a tab keeps for the
     filter; plain Rust, no I/O):
     - `pub const FILTER_READ_CHUNK: usize = 10;` (S2-Q2 as amended by the owner after T336's
       timing, 2026-11-25).
     - `pub struct ExifSummary { make, model, lens: Option<String>, date_time_original:
       Option<String>, exposure_time: Option<(u32, u32)>, f_number, focal_length: Option<f64>,
       iso: Option<u32> }` (`Debug, Clone, Default, PartialEq`), and `pub fn
       exif_summary(info: &pw_raw::ImageInfo) -> ExifSummary` copying those fields
       (`exposure_time` as `(num, den)`).
     - `pub fn exif_values(path: &Path, summary: Option<&ExifSummary>) -> Vec<(Field, Value)>`
       (S2-Q3): `FileType` always, from the extension: `"RAW"` when
       `pw_metadata::supports_sidecar(path)`, else `"DNG"`, `"JPEG"` (`jpg`, `jpeg`) or `"TIFF"`
       (`tif`, `tiff`), case-blind; any other extension gives no `FileType`. With a summary:
       `Camera` as `Text`: make and model trimmed; both empty or absent → no `Camera`; one of them
       empty → the other; the model alone when it begins with the make (compared case-blind);
       otherwise `"{make} {model}"`. `Lens` as `Text`, trimmed, left out when empty.
       `FocalLength`, `Aperture` (from `f_number`), `Iso` as `Number`, each left out when not
       finite or ≤ 0. `ExposureTime` as `Number(num / den)` in seconds, left out when either is
       0. `CaptureDate` as `Date(seconds)`: the text checked with `sort::CaptureTime::parse`
       (which rejects all zeros; its text has no accessor, so the digits are then read from the
       summary's own text, trimmed, its first 19 bytes), then the seconds since 1970-01-01 00:00:00 computed from its
       year, month, day, hour, minute and second with no time zone (a days-from-civil formula
       in `i64`); a month outside 1–12, a day outside the month, an hour > 23 or a minute or
       second > 59 gives no `CaptureDate`. Without a summary (the file could not be read): only
       `FileType`.
     - `pub fn iptc_values(values: &pw_metadata::FilterValues) -> Vec<(Field, Value)>`: each of
       the ten fields to its `Field` (`title` → `Title` … `location` → `Location`); `keywords`,
       `person_shown` and `creator` always as `Value::List` (when not empty); the others as
       `Value::Text` when they have one element and `Value::List` when they have more; an empty
       field is left out.
     - `pub struct TabValues` (`Debug, Clone, Default, PartialEq`): per file path an
       `ImageValues` (made with the file name, `ImageValues::new`), and per path the IPTC read's
       state: not asked, **in flight**, **failed once**, **could not be read**. Methods, each with
       a doc comment:
       `clear(&mut self)`;
       `retain(&mut self, files: &[PathBuf])` (drops every path not in `files`);
       `values(&self, path: &Path) -> Option<&ImageValues>`;
       `record_exif(&mut self, path: &Path, summary: Option<&ExifSummary>)` (`set_exif` with
       `exif_values`, creating the entry if needed);
       `iptc_chunk(&self, order: &[PathBuf], write_pending: &dyn Fn(&Path) -> bool) ->
       Vec<PathBuf>`: up to `FILTER_READ_CHUNK` paths of `order`, in that order, whose IPTC is
       not read, not in flight, not "could not be read", and with no write pending (a path with
       no entry, such as a file the watcher or an import added, counts as not asked). **A path
       that failed once is sent alone** (if the first eligible path failed once, the chunk is that
       path only; failed-once paths are otherwise left out of a chunk), so a file that breaks
       ExifTool fails only itself, not a whole chunk twice (S2-Q4);
       `mark_in_flight(&mut self, paths: &[PathBuf])` (creates the entry of a path that has none,
       with `ImageValues::new` and the file name by `to_string_lossy`);
       `mark_unreadable(&mut self, paths: &[PathBuf])`: "could not be read" at once (used when the
       worker has stopped);
       `end_in_flight(&mut self, paths: &[PathBuf])`: any of these paths still in flight goes back
       to not asked (a safety step after an answer, see point 5);
       `record_iptc(&mut self, path: &Path, result: Result<&FilterValues, &str>)`: **only for a
       path marked in flight** (any other answer is stale, for example from before F5 or a new
       opening of the folder, and is ignored); `Ok` → `set_iptc(iptc_values(..))`, state cleared; `Err` the first time → back to not asked,
       "failed once"; `Err` again → "could not be read" (S2-Q4: never asked again until `clear`);
       `forget_iptc(&mut self, path: &Path)`: the path's IPTC group is made **unread** again and
       its state cleared, so it is read again (S2-Q5, S2-Q6). `ImageValues` has no "unread"
       setter and no getter of its Exif map: build a new `ImageValues::new(<the path's file
       name>)` and, if the old one `is_read(Group::Exif)`, `set_exif` it with every field of
       `Field::ALL` whose group is `Exif` and whose `value()` is `Some` (cloned). A path with no
       entry: nothing;
       `exif_wanted(&self, order: &[PathBuf]) -> Vec<PathBuf>`: the paths of `order` whose Exif
       group is not read (a path with no entry counts as not read);
       `counts(&self, group: Group, files: &[PathBuf]) -> (usize, usize, usize)`: of `files`, how
       many have that group read, how many in all, and (for `Iptc`) how many "could not be read"
       (S3 shows them; for `Exif` always 0: a file `pw_raw` cannot read counts as read with only
       `FileType`, S2-Q3). A path with no entry counts as not read.
  2. **`sort.rs`:** `CaptureRead` gets `pub exif: Option<ExifSummary>`, filled by
     `CaptureReader`'s thread from the same `pw_raw::inspect` (`None` when it fails); the capture
     time is computed as today. One read per file serves both (S2-Q3).
  3. **`tabs.rs`:** `Tab` gets `pub meta_filter: crate::meta_filter::Filter` (ADR 0022's filter
     of this tab; per tab as the star filter, S2-Q7) and `pub filter_values: TabValues`.
     `Tab::open` resets both (as `filter` and `capture`). `files` is changed in `Tab::open` and in
     `browse.rs` (`replace_files`, which `relist` uses, and `reorder`, which keeps the same
     files): `keep_unread_listed` in `browse.rs` (called by `replace_files`) also calls
     `filter_values.retain(&self.files)` (a deleted or moved image leaves the values, S2-Q6).
     `browse.rs`'s tests that call `record_capture` get the new argument (`None`). `Tabs::record_capture` gets a parameter
     `exif: Option<&ExifSummary>` and also calls `record_exif` in every tab listing the path.
     New: `Tabs::record_iptc(&mut self, cwd: Option<&Path>, path: &Path, result:
     Result<&FilterValues, &str>)` and `Tabs::forget_iptc(&mut self, cwd: Option<&Path>, path:
     &Path)`, each for every tab listing the path. **Paths:** a tab's files may be relative (a
     folder given relative on the command line); the metadata cache and the worker use the
     resolved path (`main.rs`'s `resolve_path`: `cwd.join(file)`). These two methods take the
     resolved `path` and act on the tab file `file` for which `cwd.join(file) == path` (with no
     `cwd`, `file == path`), and record under the tab's own `file`. `Tab` and
     `Tabs` derive `Eq` today; `Filter` and `ImageValues` hold `f64` and are only `PartialEq`:
     remove `Eq` from those two derives. If any code needs `Tab: Eq` or `Tabs: Eq` (the build
     says so), stop and ask.
  4. **`metadata.rs`:**
     - `MetadataSubmit` gets `fn read_filter_values(&mut self, images: Vec<PathBuf>) ->
       Result<RequestId, SubmitError>`, implemented for `MetadataWorker` by its new method and in
       the tests' fake worker as its `read_metadata` is (always `Ok`, a new id each time), keeping
       the images sent so a test can check them.
     - `MetadataCache` gets one filter read in flight at most (its id and its paths):
       `pub(crate) fn request_filter_values(&mut self, worker: &mut impl MetadataSubmit, images:
       Vec<PathBuf>) -> Result<(), SubmitError>`; `Ok` only when sent; refused (and nothing sent)
       while another is in flight (return `Err(SubmitError::QueueFull)` then); and
       `pub(crate) fn filter_read_in_flight(&self) -> bool`.
     - `apply`: T336's empty arm for `WorkerEvent::FilterValuesRead` is replaced: an answer with
       the in-flight id clears the in-flight state and its entries are kept in a list taken by
       `pub(crate) fn take_filter_answers(&mut self) -> Vec<(PathBuf, Result<FilterValues,
       String>)>`; an answer with another id is dropped.
     - When the worker is found disconnected (where `drain` fails the other pending reads), a
       filter read in flight is ended too: its paths are put into the same list, each with
       `submit_error(SubmitError::Stopped)`, so `filter_read_in_flight` does not stay true.
     - The writes: nothing new. Every `EditsWritten` (any result, any source: the editor's save,
       a batch of "Apply to All Marked", a template file) already reaches `take_written`
       (`WriteAnswer`'s `path`, resolved); point 5 uses it (S2-Q5: read again from the file, also
       after a failed write, since a batch stopped halfway has written some).
  5. **`main.rs`:** a new `fn follow_filter_values(&mut self, ctx: &egui::Context)`, called once
     per frame in `draw` right after its `self.follow_sort(&ctx)` (the one after
     `refilter_when_ratings_change`; not at the other calls of `follow_sort`), with a doc comment
     naming ADR 0022 S2. `cwd` below is `std::env::current_dir().ok()`, as elsewhere in `draw`:
     - `follow_sort`'s loop over `capture_reader.try_recv()` passes `read.exif.as_ref()` to
       `record_capture` (the only change to `follow_sort`).
     - In `draw`'s existing loop over `self.metadata.take_written()` (the histograms'), each
       answer's path also goes to `tabs.forget_iptc(cwd, path)`, **first in the loop, before its
       `if !written.succeeded` branch**, since a failed write is read again too (S2-Q5).
     - Every answer of `take_filter_answers` (paths as sent, resolved). After an answer's entries
       are handled, `end_in_flight` is called in every tab with the tab paths of every path the
       answer holds, so a path whose answer matched nothing (for example a changed `cwd`) cannot
       stay in flight. For each entry: if
       `self.metadata.write_pending(path)`, the answer is dropped and `tabs.forget_iptc` called
       (S2-Q6: a read that a write overtook; it is read again after the write). `write_pending`
       is also true for a pending rating or rotation write; the needless read again is accepted.
       Else `tabs.record_iptc(cwd, path, ..)`.
     - For the **active tab only**, when `tab.meta_filter.needs(Group::Exif)`: the paths of
       `exif_wanted` in the order `read_order(tab.files.len(), &tab.visible, tab.current, ..)`
       gives (visible first, S-Q6; it returns indexes, mapped to `tab.files`' paths), not in
       `capture_in_flight`, are asked of `capture_reader`
       and put in `capture_in_flight`, as `request_capture_times` does; a refused request
       records the path with no capture time and no summary, as there.
     - For the active tab, when `tab.meta_filter.needs(Group::Iptc)`, no filter read is in flight
       **and no batch is running** (`self.metadata.batch_running()` and no template batch,
       `template_batch_progress()` is `None`; S2-Q5: the batch's images are read after it, not
       between its writes): `iptc_chunk` over the order `read_order` gives with a skip of its own (nothing
       skipped; `iptc_chunk` does the choosing), with `tab_pending(&self.metadata, cwd)` (it
       resolves tab paths) as `write_pending`; the chunk's paths are resolved for the request; a
       non-empty chunk is sent with `request_filter_values` and, when `Ok`, marked in flight in
       the tab (by its own paths); a `QueueFull` is tried again next frame; a `Stopped` gives
       every path of the chunk `mark_unreadable`. Where an error text is needed for a stopped
       worker (also in point 4's disconnect), use `metadata.rs`'s `submit_error(SubmitError::
       Stopped)`, as `drain` does, instead of a new text.
     - `ctx.request_repaint_after(CAPTURE_POLL_INTERVAL)` while a filter read is in flight, while
       `capture_in_flight` is not empty, and after a `QueueFull` (so the retry comes without
       input). The order and the chunk are computed only while `needs` is true.
     - `reload_active_tab` (F5) also empties the active tab's `filter_values` (the filter itself
       is kept, as the star filter is), **only after the listing succeeded** (it returns early on
       a failed listing), so its values are read again when needed (ADR 0022 §2.4;
       `known-limits.md`, "A sidecar changed by another program"). The other listings (the
       folder watcher's, `relist_tabs` after a batch or an undo) only `retain`; they do not empty
       the values.
  6. **`lib.rs`:** `pub mod filter_values;` in its alphabetical place, and the new items exported
     where the tests or `main.rs` need them, as the other modules are.
  7. **No panics** outside `#[cfg(test)]` (`AGENTS.md` §3).
- **Tests to add:**
  - In `filter_values.rs`:
    1. `exif_values`: `FileType` for `a.ARW` (RAW), `a.dng`, `a.JPG`, `a.jpeg`, `a.tif`, and none
       for `a.png`; without a summary only `FileType`.
    2. Camera: `("Canon", "Canon EOS R5")` → `"Canon EOS R5"`; `("SONY", "ILCE-7RM5")` →
       `"SONY ILCE-7RM5"`; `("  ", "X100V")` → `"X100V"`; both absent → none.
    3. Numbers: `f_number` 2.8 → `Aperture` 2.8; exposure `(1, 250)` → 0.004; `(0, 250)` → none;
       ISO 0 → none; focal length 35.0 → 35.0.
    4. Date: `"2024:03:01 12:00:00"` → the seconds of that time (write the expected number out);
       `"2024:02:30 12:00:00"` → none; `"0000:00:00 00:00:00"` → none; `"1970:01:01 00:00:00"` →
       `0`; two times 60 s apart differ by 60.
    5. `iptc_values`: one title → `Text`; two titles → `List`; keywords with one item → `List`;
       empty fields left out.
    6. `TabValues`: `iptc_chunk` gives at most `FILTER_READ_CHUNK` paths in `order`'s order and
       skips in-flight, read, "could not be read" and write-pending paths; `record_iptc` `Err`
       once → asked again by the next `iptc_chunk`, `Err` twice → never again, counted by
       `counts`; `forget_iptc` keeps the Exif values and makes the IPTC unread; `retain` drops a
       path; `clear` empties all.
  - In `tabs.rs`: 7. `record_capture` with a summary fills the Exif values in every tab listing
    the path and not in another tab; `record_iptc` and `forget_iptc` likewise, also with a
    relative tab file and a `cwd` (the resolved path finds it); `open` empties `filter_values`
    and resets `meta_filter`; a `relist` that drops a file drops its values.
  - In `metadata.rs` (with the fake worker): 8. `request_filter_values` sends once and refuses a
    second while the first is in flight; the answer with the in-flight id is taken by
    `take_filter_answers` and a stale id is dropped; a disconnected worker ends the read in
    flight with an `Err` per path.
  - In `filter_values.rs` also: `record_iptc` for a path not in flight changes nothing (the
    answer from before F5); a path that failed once is sent alone and a chunk never mixes it
    with others; `mark_in_flight` creates a missing entry; `mark_unreadable` and
    `end_in_flight` as described; `counts` with paths that have no entry.
  - In `main.rs`'s tests, as `capture_requests`' tests build a `Tab`: 9. a pure function the
    frame step uses for its order (if you add one) is tested; and a test that an answer for a
    path with a write pending is dropped and the path asked again, through the smallest function
    you split out of `follow_filter_values` for it (the frame step itself needs `egui`).
- **Not in this task:** the search box, Ctrl+F, joining `meta_filter` to `visible`/`refilter`,
  the status line "Reading metadata {i} of {n}" and its text for "could not be read": S3. The
  "Filter…" window and value lists: S4. Values of tabs other than the active one are read when
  that tab becomes active (no reading in the background for inactive tabs). Left to S3: the
  meaning of "Reading metadata {i} of {n}" (which `n`, and how the Exif and IPTC counts join),
  and the "visible first" order once `meta_filter` hides unread images from `tab.visible` (then
  the order should come from the star and label filter's list). Accepted here: two tabs of the
  same folder read the same files each; the Exif requests are queued at once in the capture
  reader's channel, so their order is fixed when queued; an ExifTool that hangs (no read
  timeout in `execute`) blocks the worker, as for every other request.
- **Do not touch:** any other file. In particular `docs/tasks/known-limits.md` (named in point 5,
  not changed), `meta_filter.rs` (S1's model, used as it is; if
  it lacks something, stop and ask), `filter.rs`, `pw-metadata`, `pw-viewcore`, any ADR or
  `AGENTS.md`, `tools/`, `.github/`, every `Cargo.toml` (**no new dependency**) and `Cargo.lock`,
  `CHANGELOG.md`, and `docs/test-inventory.md` (**an exception to README rule 12:** the reviewer
  updates it after the merge; put the new tests' count and a one-line description of each in your
  report). If another file seems to need a change, stop and ask.
- **Rules:** README rules 1–11 and 13 apply, except that **this task overrides rule 1**: you work
  in the worktree `C:\Development\PhotoWorkflow-Codex` on the branch `codex`, not on
  `feature/gui-workspace`. **Never run `git commit`, `git add`, `git stash`, `git reset` or
  `git checkout`, and never create a branch or a checkpoint;** the reviewer brings your work over
  as a patch and commits. **Stop and ask the owner whenever something is unclear, contradicts the
  code or ADR 0022, or needs a file outside the list;** do not guess and do not leave a point out.
  Do not weaken a test to pass it. Build only in the worktree's own `target` folder. No personal
  data in any file.
- **Report (as README rule 9, plus):** every file changed and why; the exact summary from
  `test-output.txt` after `.\tools\check.ps1` (or, if it cannot run in your sandbox, as T336's
  Report says); the tests added with a one-line description each; any `cargo clippy` warning;
  every doubt and each place where you chose.
- **Done when:** `.\tools\check.ps1` is all OK (or as the report says), `git status` shows only
  the files above, and `git diff docs/tasks/` shows only the status and one new Log row.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added; waits for T336 (its request and `FilterValues`), the cold read and the chain review. Cold read: nothing needs `Tab: Eq`; every `Tab` literal uses `..Tab::default()`. Fixed: `browse.rs` added (`replace_files`/`keep_unread_listed` change `files`; its tests call `record_capture`); the resolved paths against the tab's relative ones (`cwd` on `record_iptc`/`forget_iptc`, `tab_pending`); the call site in `draw`; the IPTC order without the Exif skip; `read_order` gives indexes; the existing `take_written` used instead of a new list; a disconnected worker ends the read in flight; a stale answer (before F5 or a new opening) ignored; repaint for the capture reads and after `QueueFull`; `CaptureTime`'s text not accessible; how `forget_iptc` rebuilds `ImageValues`; the fake worker; `write_pending` also true for rating writes (accepted); `known-limits.md` not touched |
| 2026-11-25 | Blocked | Chain review (T336 → T337): the hand-over between the tasks matches (names, the event's shape, paths as sent). Fixed in T337: a stopped worker marked "could not be read" directly (`mark_unreadable`; the double `Err` would have been ignored as stale); its text from `submit_error`; `forget_iptc` before the `!succeeded` branch of the `take_written` loop; no chunk while a batch runs (S2-Q5); a path that failed once is sent alone (S2-Q4: one broken file fails only itself); a path with no entry counts as not asked and `mark_in_flight` creates it; `end_in_flight` after every answer; F5 empties only after a successful listing, other listings only `retain`; left to S3 and accepted, in "Not in this task" |
| 2026-11-25 | Open | T336 done (611b95c); `FILTER_READ_CHUNK` is 10 (ADR 0022 S2-Q2 as amended by the owner after T336's timing). A failed image's error text is ExifTool's own message (or `"no answer for …"`); a late standard error line may land in a later answer (`known-limits.md`). Cold read and chain review done (rows above); the Codex worktree moved to the tip of `feature/gui-workspace` |
