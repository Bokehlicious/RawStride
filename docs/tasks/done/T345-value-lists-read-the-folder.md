# T345. ADR 0022 S4c: the value lists read the whole folder — Done

- **Why:** ADR 0022 S4 (the "Filter…" window's value lists), decided by the owner on 2026-11-26
  (ADR 0022 §3.3, S4-Q6, S4-Q7). After T344 a value list ("is one of" on Camera, Lens,
  Keywords, City, Country) shows only the values already read. This task reads the group such
  a list needs for the whole folder while the window is open, and shows the reading's progress
  under the list. The last task of the chain T343 → T344 → **T345**; after it the owner's hand
  check of S4 (S4-Q14).
- **What the reviewer checked (2026-11-26):**
  - `crates/pw-loupe/src/main.rs`: `follow_filter_values` (about line 2641): answers taken in,
    then for the active tab `exif = meta_filter.needs(Group::Exif)`, `iptc =
    meta_filter.needs(Group::Iptc)` and no filter read, batch or template batch running; the
    order from `filter_read_order` (about line 12069: the star and label filter's list from the
    current image, `read_order`); the IPTC chunk from `TabValues::iptc_chunk`, the Exif files
    from `exif_wanted` through `request_captures`; a repaint while reads go on.
    `refilter_after_filter_read` (about line 2720) refilters only while `meta_filter` is
    active. T344's `filter_window` state (the draft and the tab's index).
  - `crates/pw-loupe/src/filter_values.rs`: `TabValues::counts(group, files)` (read, total,
    unreadable); `iptc_chunk` (skips what is read, in flight or unreadable); `exif_wanted`.
  - `crates/pw-loupe/src/filter_window.rs` (T343): `FilterDraft::list_groups`.
  - `crates/pw-loupe/src/loupe_text.rs`: `reading_metadata_text`, `unreadable_images_text`.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`
  (rule 12), and this task's own file (status and log).
- **Model:** Opus — the reads' state across the window, the applied filter and the worker
  (README, "Model"; S4-Q14).
- **What:**
  1. **The groups to read** (S4-Q7): `enum ReadScope { None, Filter, Folder }` and a plain
     function `filter_read_scopes(filter: &Filter, window: Option<&FilterDraft>) -> (ReadScope,
     ReadScope)` (Exif, IPTC): a group of `FilterDraft::list_groups` is `Folder`; otherwise a
     group `filter.needs` asks for is `Filter`; otherwise `None`. The caller passes the
     window's draft only when T344's tab check (its test f: same index and folder) says the
     window's tab is the active one, otherwise `None` (T344 closes the window otherwise, so
     this holds whenever it is open). `follow_filter_values` uses it in place of
     its two `needs` calls. The IPTC conditions after it (no filter read in flight, no batch, no
     template batch) stay and gate IPTC only, as today; Exif is not gated by them.
  2. **The order per group** (S4-Q7): a `Filter` group keeps today's order
     (`filter_read_order`). A `Folder` group uses a plain function
     `folder_read_order(tab, ratings_ready, rating_of) -> Vec<PathBuf>`: `read_order(
     tab.files.len(), &star_label_list, tab.current, |_| false)` (`sort.rs`), which gives the
     star and label filter's list from the current image, then every other file in the tab's
     order, each once, mapped to `tab.files` entries (the tab's own paths, never resolved ones:
     chunks, `mark_in_flight` and the Exif requests use them; only the worker request is
     resolved, as today). So a group the window needs is read for the whole folder with the
     shown list first, and a group only the applied filter needs is not read past its list.
     Each order is made at most once per frame.
  3. **The progress under each value list** (S4-Q7): while the list's group has images of the
     tab not yet read and not unreadable (`counts(group, &tab.files)`), the window shows
     `reading_metadata_text(read + unreadable, total)` under the list; when the reading is
     done and some could not be read, `unreadable_images_text(unreadable)` (only IPTC can
     have such images: a failed Exif inspection counts as read, `counts` gives 0). The texts
     come from `loupe_text.rs`'s functions, through a plain function of the counts, never as
     literals in drawing code. While a batch or template batch runs, IPTC reads wait (point 1)
     and the text stands still until it ends; accepted. The list itself is made again each
     frame from `value_counts` (T344), so it grows as the chunks come in; a repaint is asked
     for while the reads go on.
  4. **Closing the window** (Cancel or OK): the window's groups are no longer asked for; an
     IPTC chunk in flight is answered and kept as any other (the values stay for the tab until
     its folder is listed again, S-Q7). Exif requests already queued in the capture reader
     (unbounded, first in first out, `sort.rs`) cannot be taken back and are read and kept;
     accepted, no change to `sort.rs` (noted for `known-limits.md` by the reviewer). The
     applied filter's reads go on as before. An IPTC answer is recorded whether or not the
     applied filter is active (`apply_filter_answer`), and every Exif answer is recorded
     (`Tabs::record_capture`), so nothing more is needed for the answers to land.
  5. **Nothing else changes:** `refilter_after_filter_read` stays (the list behind the window
     changes only through the applied filter, S4-Q1); the status line counts the applied
     filter only (T344, S4-Q8), not the window's reading.
- **Tests to add** (`main.rs`):
  - `filter_read_scopes`: no rows and no window → `None` for both; a search → IPTC `Filter`;
    a window with Camera "is one of" → Exif `Folder`; a window with City "is one of" and an
    applied Exif row → Exif `Filter`, IPTC `Folder`; an applied City row and a window with City
    "is one of" → IPTC `Folder`; a window with Camera "contains" → nothing from the window.
  - `folder_read_order`: under a star filter the star and label list comes first from the
    current image, then the other files in the tab's order, each file once, as the tab's own
    paths.
  - The progress text as a plain function of the IPTC or Exif counts: reading, done with
    IPTC failures, done (Exif never gives the failure text).
- **Not in this task:** any change to the window's look or rows (T344), the model (T343), the
  status line (T344); saved filters (S5).
- **Rules:** `docs/tasks/README.md` rules 1–13. No `git commit`, `git add`, `git stash`,
  `git reset`, `git checkout` and no new branch (rule 8). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  Then the owner's hand check of S4 (S4-Q14; written in full by the reviewer at this task's
  review): Ctrl+Shift+F and View ▸ Filter ▸ Filter… in the contact sheet and the preview; each
  kind of row on a text, number, date and choice field; a row not complete (OK disabled, the
  text under it); "1/250", "f/2.8", a decimal comma, a date "≤" and "between" in either order;
  "is not" on City also showing images without a City; a row on an Exif field alone (ISO
  ≥ 1600, sorted by file name) filling in as the Exif is read (T344 point 4a); a folder dropped
  or an import opening a tab while the window is open; F5 keeping the rows; a value list growing while
  "Reading metadata {i} of {n} …" counts, a picked value missing from the folder with 0; OK,
  Cancel, Escape, Clear; the button's "Filter: {n} conditions" and outline; the rows opened
  again as typed; Show All; the status line's count with an Exif row and a search; marks kept
  for unread images; another tab with its own rows; F5 and opening the folder again.
  `latency.ps1` is not needed. At Done one CHANGELOG line under Added covers T344 and T345.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Blocked | Added (ADR 0022 §3.3); waits for T344 (uses its window state and lists) |
| 2026-11-26 | Blocked | Cold read: the order per group (`ReadScope`: a group only the applied filter needs is not read past its list), `read_order` named for the folder order, the tab's own paths, the window's tab checked by the caller; the answers land already (`record_capture`, `apply_filter_answer`); queued Exif reads cannot be taken back (accepted, a known limit at Done); no Exif failure text; the text stands still during a batch |
| 2026-11-26 | Blocked | Chain review (see T343's log): the window's tab checked by index and folder as in T344; the hand check extended (an Exif-only row, a drop or import while the window is open, F5 keeping the rows). Leftovers for this task's Done: `known-limits.md` gets the queued Exif reads (point 4), the draft lost when the window's tab stops being active (T344), Ctrl+Shift+F doing nothing while the search box has the keyboard; `backlog.md` or `known-limits.md` (the owner's choice at the sort) gets the capture reader's thread dying (`try_recv` hides the disconnect, `sort.rs` about line 396: its files stay unknown and "Reading metadata" never ends; older than S4, the sort has it too) |
| 2026-11-26 | Open | T344 done |
| 2026-11-26 | Ready for review | Implemented folder reads and progress for the open value lists. Added three tests for scope, order and progress. All checks passed with the C locale for the test process. Owner's S4 hand check pending. |
| 2026-11-26 | Done | Reviewed (done by Codex): every point and test as written; `.\tools\check.ps1` run by the reviewer, all OK (2876 passed), no clippy warnings; the test inventory updated by the agent (+3), checked. The reviewer gave `ReadScope` a doc comment and put ADR 0022 back into `follow_filter_values`' (owner's yes). Leftovers in `known-limits.md` (ADR 0022): the queued Exif reads (point 4), Ctrl+Shift+F while the search box has the keyboard, the capture reader's thread dying (owner: known limit, not backlog); the draft lost on a tab change was added at T344. `CHANGELOG.md`: T344's line extended with the value lists. Awaiting the owner's hand check of S4 (the list in Done when) |
| 2026-11-26 | Done | Hand-checked by the owner: the list in Done when passed (S4 done). During it the owner asked for columns of values with counts over the catalog (`backlog.md`, "Value columns over the catalog", 602f57d) |
