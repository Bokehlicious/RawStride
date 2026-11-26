# T341. ADR 0022 S3a: the search in the refilter, the reads and the status line — Blocked: waits for the chain review

- **Why:** ADR 0022 S3 (the search box and Ctrl+F), decided by the owner on 2026-11-26 (ADR 0022
  §3.2, S3-Q1–S3-Q12, every proposal approved). S1 (T330) made the filter model
  (`meta_filter.rs`), S2 (T336, T337) the reading of the values per tab (`filter_values.rs`, the
  shell's `follow_filter_values`). Nothing uses the tab's `meta_filter` in the shown list yet.
  This task makes the list, the reads, the marks, the preview and the status line follow an
  active search; T342 adds the box and Ctrl+F that set it. The first task of the chain
  **T341** → T342.
- **What the reviewer checked (2026-11-26):**
  - `crates/pw-loupe/src/meta_filter.rs`: `Filter::set_search`, `is_active`, `needs` (about
    lines 443–493); `Filter::outcome` gives `Outcome::Match`, `NoMatch` or `Unknown` (about
    line 495; `Outcome` about line 310); a search with the IPTC group unread is `Unknown`, also
    when the file name holds the word (`search_outcome`, about line 519).
  - `crates/pw-loupe/src/filter_values.rs`: `ReadState` (`NotAsked`, `InFlight { retry }`,
    `FailedOnce`, `Unreadable`, about line 159) and `Entry { image, state }` (about line 170);
    `TabValues::iptc_chunk` skips an image whose IPTC group is read (about line 215);
    `record_iptc` (about line 269); `forget_iptc` drops the IPTC values at once and keeps
    Exif (about line 286); `counts(group, files) -> (read, listed, unreadable)` (about line 312).
  - `crates/pw-loupe/src/browse.rs`: `Tab::refilter` (about line 138): every file without an
    active star filter; nothing while an active star filter waits for the ratings; otherwise the
    matches plus `keep`; a file of `unread` is hidden under an active filter and keeps its mark
    (ADR 0014 amendment 3); `rated` calls it (about line 189). Its tests use `setup()`, `set()`
    and `status()` (about lines 760–815).
  - `crates/pw-loupe/src/tabs.rs`: `Tab` with `filter`, `meta_filter`, `filter_values`,
    `visible`, `unread` (about lines 20–60); `Tabs::forget_iptc` (about line 398).
  - `crates/pw-loupe/src/sort.rs`: `read_order(count, visible, current, skip)` (about line 314).
  - `crates/pw-loupe/src/main.rs`: `refilter_active` (about line 2245); `set_filter` (about line
    2368); `refilter_when_ratings_change` with `preview_must_leave` (about lines 2376–2395);
    `follow_filter_values` (about line 2481) reads only for the active tab, in the order of
    `filter_read_order` (about line 11750), which starts from `tab.visible`;
    `loupe_follows_visible` (about line 2673) goes back to the grid when nothing is visible;
    `switch_mode` refuses the preview with nothing visible (about line 4031); the grid's empty
    text and the status line's parts (about lines 4500–4565); `nothing_shown_text` (about line
    4577); `take_written` calls `Tabs::forget_iptc` for every written file (about line 9361);
    `apply_filter_answer` calls `forget_iptc` for an answer whose write is pending (about line
    11758); `preview_must_leave` (about line 11794).
  - `crates/pw-loupe/src/loupe_text.rs`: `position_text`, `no_match_text`,
    `READING_RATINGS_TEXT` (about lines 150–175) and their test (about line 615).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/browse.rs`,
  `crates/pw-loupe/src/filter_values.rs`, `crates/pw-loupe/src/loupe_text.rs`,
  `crates/pw-loupe/src/lib.rs` (the re-export of the new texts), `crates/pw-loupe/src/main.rs`,
  `docs/glossary.md`, `docs/test-inventory.md` (rule 12), and this task's own file (status and
  log).
- **Model:** Opus — state over time (the reads, the refilter after each chunk, the preview's
  wait) (README, "Model").
- **What:**
  1. **The refilter with the search** (`Tab::refilter`, `browse.rs`): the tab's whole filter is
     active when `self.filter.is_active() || self.meta_filter.is_active()`. Without either,
     every file is shown, as today. Otherwise a file is shown when it passes the star and label
     filter as today (the ratings wait and `unread` as today) **and**
     `self.meta_filter.outcome(values)` is `Outcome::Match`, where `values` is
     `self.filter_values.values(path)`; a file without an entry there is unknown (`Unknown`).
     `keep` is shown as today. Marks: a file hidden because the outcome is `Unknown` keeps its
     mark (S3-Q7, as `unread` does); a file hidden because it does not pass the star and label
     filter or its outcome is `NoMatch` is unmarked (ADR 0014 §2.8). The current image: as
     today (`nearest_visible`). The doc comment says all of this. `rated` keeps calling
     `refilter`.
  2. **The star and label list** (S3-Q4, S3-Q6): a new `Tab::star_label_list(&self,
     ratings_ready, rating_of) -> Vec<usize>` in `browse.rs`: the file indexes, in display
     order, that the star and label filter lets through, without `meta_filter` and without
     `keep`: every file when the star filter is not active; none while an active star filter
     waits for the ratings; a file of `unread` left out under an active star filter, as in
     `refilter`. `refilter` uses it for its first condition, so the two cannot differ.
  3. **The order of the reads** (S3-Q6): `filter_read_order` in `main.rs` takes the ratings
     (`ratings_ready`, `rating_of`, as `refilter_active` gets them) and gives
     `read_order(tab.files.len(), &tab.star_label_list(…), tab.current, |_| false)`, so the
     reads start at the current image and go forward over the images the search must decide,
     not over `visible` (where they are hidden). The Exif reads of `follow_filter_values` use
     the same order. When the current image is not in the list (for example hidden by the
     stars), `read_order` starts at the list's first image; that is accepted. The existing test
     `filter_reads_start_at_current_then_wrap_before_hidden_files` (`main.rs`, about line
     16325) expects the old order over `visible`; S3-Q6 changes that order, so the test is
     rewritten to it, the one test this task may change for that reason: with no star filter,
     `visible [1, 3]` and current 3 the order is now `[3, 0, 1, 2]` (every file is in the star
     and label list); and a second case with a star filter that hides a file shows that the
     hidden file is not asked for.
  4. **The refilter after each chunk** (S3-Q3): in `follow_filter_values`, when at least one
     answer was delivered (`take_filter_answers` gave something), a chunk ended
     (`end_filter_read`), or a chunk was refused by a stopped worker
     (`SubmitError::Stopped` → `mark_unreadable`, about line 2546), and the active tab's
     `meta_filter` is active: `refilter_active(None)`, and when it changed something or
     `preview_must_leave` (point 6) holds, `loupe_follows_visible()`, as in
     `refilter_when_ratings_change` (about line 2392). The refilter after the answers and
     `end_filter_read` sits before the borrow `self.tabs.active()` (about line 2503), and once
     more after `mark_unreadable`. Images whose answer came are shown or hidden at once; the
     current image stays current while it is shown. No scroll is asked for (as the ratings'
     refilter asks none).
  5. **Read again after RawStride's own write: the old values kept** (S3-Q8, S3-Q12):
     `TabValues::forget_iptc` no longer drops IPTC values that were read. `Entry` gets
     `reread: bool`; `forget_iptc` sets `reread = e.image.is_read(Group::Iptc)` (an image never
     read has nothing to keep: it stays as today, unread) and the state `NotAsked`, and keeps
     `image` as it is (the old IPTC values and Exif). `iptc_chunk` tests `reread` before its
     skip of a read IPTC group (about line 215): an entry with `reread` is chunked like a
     `NotAsked` image (a pending write still held back as today, a retry still alone).
     `record_iptc` with `Ok` sets the new values and clears `reread`; a first `Err` gives
     `FailedOnce` with the old values kept; a second `Err` (the retry) drops the IPTC values and
     gives `Unreadable`, `reread` cleared (S3-Q12). `mark_unreadable` (a stopped worker) does
     the same to a `reread` entry. Dropping is today's body of `forget_iptc` (the name and Exif
     kept, `named_image` plus the Exif copy), moved into a private helper used by these places.
     `end_in_flight` keeps `reread`. `clear` and `retain` as today. The existing test
     `stale_answers_and_listing_changes_preserve_group_state` (`filter_values.rs`, about line
     497) asserts `!image.is_read(Group::Iptc)` after `forget_iptc`; S3-Q8 changes exactly that,
     so this assertion is turned to `image.is_read(Group::Iptc)` with the old values, and the
     rest of the test stays. The doc comments of `forget_iptc`, `iptc_chunk` and
     `record_iptc` say this. `Tabs::forget_iptc` (`tabs.rs`) and the two callers in `main.rs`
     stay as they are: the dropped answer of `apply_filter_answer` (S2-Q6) also keeps the old
     values now (ADR 0022 S3-Q8).
  6. **The preview waits** (S3-Q11): a new `fn search_waiting(tab: &Tab, ratings_ready, rating_of)
     -> bool` in `main.rs` (or a `Tab` method in `browse.rs`): the tab's `meta_filter` is active
     and at least one image of `star_label_list` is neither read in the IPTC group nor
     `Unreadable` (point 7's counts: `done < total`). While it is true and nothing is visible:
     - `loupe_follows_visible` (about line 2673) does not go back to the grid: with no
       position it calls `self.core.set_files(Vec::new(), 0)` and stays in the preview (the
       view core takes an empty list; the preview then has `Status::Empty`);
     - `preview_must_leave` gets a third parameter `search_waiting: bool` and is false while it
       is true; its existing tests (about lines 16436–16450) pass `false`, their meaning
       unchanged;
     - the preview's `Status::Empty` text (about line 10152, where `shown.message` or
       `status_line(&frame, …)` — "No folder open …" — is drawn today) and the first part of
       the preview's status line (about line 9907, `status_line(&frame, …)`) are
       `reading_metadata_text(done, total)` instead. How the text reaches the drawing (a field
       of the shown state or a parameter) is the agent's choice; the drawing code holds no
       literal (rule 13).
     When it turns false with nothing visible, the preview goes back to the grid (ADR 0014
     §2.3) through point 4's check. `switch_mode` keeps refusing to open the preview with
     nothing visible. An extended-filter row (S4) is not counted: `search_waiting` looks at the
     IPTC group only; S4 decides how Exif rows join it.
  7. **The counts** (S3-Q4, S3-Q5): a new
     `TabValues::search_counts(&self, files: &[PathBuf]) -> SearchCounts` in `filter_values.rs`,
     `pub struct SearchCounts { pub done: usize, pub total: usize, pub unreadable: usize }`:
     `total` = `files.len()`, `done` = those whose IPTC group is read (an entry with `reread`
     counts as read: it has values) or whose state is `Unreadable`, `unreadable` = those
     `Unreadable`. The shell passes the paths of `star_label_list`. (`counts` stays for its
     existing callers; the Exif reads are not counted in S3, S3-Q4; how they join is S4's.)
  8. **The texts** (rule 13), in `loupe_text.rs` beside `READING_RATINGS_TEXT`, re-exported in
     `lib.rs`:
     - `pub fn reading_metadata_text(done: usize, total: usize) -> String`:
       `format!("Reading metadata {done} of {total} …")`;
     - `pub fn unreadable_images_text(count: usize) -> Option<String>`: `None` for 0,
       `"1 image could not be read"` for 1, `format!("{count} images could not be read")`
       otherwise.
     `docs/glossary.md` gets one row in §2 (Ratings, labels, marking and filters) for both, in
     the form of the rows around it ("added by T341").
  9. **The status line and the grid** (S3-Q4, S3-Q5), in `main.rs`, only while the active
     tab's `meta_filter` is active:
     - while `search_waiting`: the part `reading_metadata_text(done, total)` is added after the
       grid's position, marked and name parts (about lines 4532–4537), or after the
       nothing-shown text, and before `self.message`; the preview's status line (point 6) shows
       it as its first part only while nothing is shown;
     - when not waiting and `unreadable > 0`: the part `unreadable_images_text(unreadable)` in
       the same place;
     - `nothing_shown_text`: after the read error and the ratings wait (as today), while
       `search_waiting` it gives `reading_metadata_text(done, total)` instead of
       `no_match_text` (it gets what it needs for `search_waiting` and the counts, for example
       `tab_rating`); the status line then shows it once (not twice: the extra part is left
       out when the line's first part is already it).
     A repaint is already asked for while reads are in flight (`follow_filter_values`).
- **Tests to add:**
  - `browse.rs`, beside the `filter-select` tests: (a) a search on a tab with no values read
    shows nothing and keeps every mark; (b) after values for some images (set through
    `TabValues` the way `filter_values.rs`'s tests do), the matching ones are shown in display
    order, the `NoMatch` ones are unmarked, the unknown ones keep their marks; (c) a search
    with a star filter: an image hidden by the stars is not shown whatever its values;
    (d) `star_label_list` without a star filter, with one, while it waits for the ratings and
    with `unread`; (e) the current image moves to the nearest shown one when its outcome is
    `NoMatch`; (f) a search that turns empty shows every file again.
  - `filter_values.rs`: (a) `forget_iptc` keeps the values and the image is chunked again;
    `record_iptc` `Ok` replaces them and it is not chunked again; (b) a failed read keeps the
    old values (`FailedOnce`), and its failed retry drops them and leaves `Unreadable`;
    (c) `mark_unreadable` on a `reread` entry drops the values; (d) `forget_iptc` on an image
    never read leaves it unread, without `reread`; (e) `search_counts` with read, unread,
    `reread`, `FailedOnce` and `Unreadable` images.
  - `loupe_text.rs`: both texts, with 0, 1 and 2 for the unreadable one.
  - `main.rs`: (a) `filter_read_order` follows `star_label_list` from the current image, also
    when the current image is hidden by the search; (b) `search_waiting` true while an image of
    the list is unread, false when all are read or unreadable; (c) `preview_must_leave` with
    `search_waiting` true is false, and with it false (the reads done) and nothing shown is
    true.
- **Not in this task:** the box, Ctrl+F, the key map, Show All emptying the search, the outlined
  box (T342); the "Filter…" window and its Exif counts (S4); any change to `meta_filter.rs`'s
  matching or to `pw-metadata`.
- **Rules:** `docs/tasks/README.md` rules 1–13. No `git commit`, `git add`, `git stash`,
  `git reset`, `git checkout` and no new branch (rule 8). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  No hand check of its own: nothing sets a search yet; the owner's hand check of S1–S3 follows
  T342.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Blocked | Added (ADR 0022 §3.2, S3-Q1–S3-Q13) |
| 2026-11-26 | Blocked | Cold read: the preview showed "No folder open …" for an empty list, not "Reading ratings …" (point 6 now names the `Status::Empty` text, the preview's status line and `set_files(Vec::new(), 0)`); `preview_must_leave` gets `search_waiting` as a parameter; `reread` only for an image whose IPTC was read, checked first in `iptc_chunk`, and `mark_unreadable` drops it too; the two existing tests whose expectations S3-Q6 and S3-Q8 change are named; the refilter also after a stopped worker; the status part's place after the name |
