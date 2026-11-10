# ADR 0014: Milestone 5 – filter and multi-selection in the egui program

- Status: **accepted by the owner (2026-11-10, step F0).** The scope (§3), S1–S6 as behaviour
  requirements (§7) and every proposal in §11 are accepted as written. Nothing is built, installed or
  added as a dependency by this ADR. Each step in §9 is its own work part, with its own approval and
  commit.
- **Amendment 2 (owner, 2026-11-10, §14):** the label filter takes several labels at once (T61).
- **Amendment 1 (owner, 2026-11-10, §13):** §2.1 gets a star condition (≥, =, ≤), and §2.2 removes
  a rated image that stops matching at once. Tasks T59, T60; T57 changed.
- Date: 2026-11-10
- Related:
  - ADR 0004, addendum "filter and multi-selection" (2026-11-04): today's Tauri behaviour. **Its
    rules are carried over** unless §11 says otherwise; this ADR decides how the egui program
    implements them;
  - ADR 0013 (milestone 4, ratings and labels; Q1 made this milestone 5; §4.2 the write queue was
    designed so that one key can target several images);
  - ADR 0011 (milestone 3, the contact sheet; Q6 reserved Escape in the grid for clearing a
    multi-selection);
  - ADR 0009 §3.3 (Ctrl+A reserved);
  - [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md) GR6 (keyboard first), GR8 (feature parity), GR11
    (testable);
  - `ui/web/app.js` (`matches`, `applyFilter`, `ratingTargets`, `clickCell`, `pickRange`, `go`) and
    `ui/tests/filter-select.test.js` (22 checks).

## 1. Context

- RawStride (`pw-loupe`) has the loupe, the contact sheet, folder tabs, the import dialog, and
  ratings and labels (ADR 0013). GR8 still lists **filter and multi-selection**, IPTC metadata and
  TIFF viewing before the Tauri app can be retired. This milestone is the first of those three.
- **Already in place:**
  - `RatingStore` (`pw-loupe/src/ratings.rs`): the shown value per path, an ordered write queue that
    coalesces unsent writes per path, rollback on failure, read merge. One key → one `set` today.
  - `Tab { folder, files, current, mode }`; the grid model (`columns`, `visible_range`, `navigate`)
    works on positions `0..files.len()`; `ThumbGrid` keys thumbnails by file index.
  - `pw-viewcore` gets the file list with `open_folder(files)` when the loupe opens. Its caches are
    keyed by **position** in that list (`Key = (usize, Source)`), and `open_folder` drops them all.
- **Missing:** a filter, a marking of several images, rating keys that act on the marking, and a
  loupe that steps within the filter.

## 2. Decisions (ADR 0004 addendum; points 1 and 2 changed by amendment 1, §13)

1. **Filter by stars:** two lists side by side: a **condition** (≥, =, ≤) and a **star choice**
   (all, 1–5 stars, unrated, rejected, all except rejected). The condition applies only when a
   number of stars is chosen; with any other star choice it is disabled and changes nothing, and
   it keeps its value. "≥ n" matches n stars or more; "= n" exactly n stars; "≤ n" n stars or
   fewer **and unrated images**. A rejected image (−1) never matches a number of stars, under any
   condition. **By label:** all, one of the five colours, or no label. Stars and label combine
   with AND.
2. **An image that stops matching after a rating leaves the visible list at once**, in the grid
   and in the loupe, for every image the key acted on (one image or a marking). If the current
   image leaves, point 3 applies. Marked images that leave are unmarked (point 8); marked images
   that still match stay marked. A refused key changes nothing. A rolled-back write (a failed
   write that restores the old rating) does not recompute the list; the next move or filter
   change does.
3. **When the filter hides the current image**, the nearest following visible image becomes current
   (the last visible one if none follows). If nothing is visible, the loupe returns to the grid.
4. **An active filter is always visible:** its control is outlined, and the status line shows
   "n / shown (av total)", or "Inga bilder matchar filtret (N i mappen)".
5. **Multi-selection in the contact sheet:** Ctrl+click toggles one image, Shift+click marks a range
   from the anchor, Shift+arrows extend the range, Ctrl+A marks every visible image, Escape or a plain
   arrow clears the marking. The status line shows "k markerade" when more than one is marked.
6. **Targets of a rating key:** in the grid, every marked image (in grid order) if the current image
   is marked; otherwise the current image. **In the loupe, always the shown image.**
7. **Batch rules:** one write per image, queued in order. Files that cannot carry ratings are skipped
   silently; if none of the targets can, one message. A label key removes the label only if **all**
   ratable targets already have it; otherwise it sets it on all.
8. **Marked images that the filter hides are unmarked.**

## 3. Scope

### 3.1 In scope

| Area | Content |
|---|---|
| Filter model | `Filter { stars, label }`, `matches(rating)`, and the visible list of a tab, recomputed after a rating (§2.2) and around a move (§4.2, §2.3). Plain Rust |
| Filter controls | Three drop-down lists (`egui::ComboBox`) in the tab bar row, next to "Importera…": the star condition, the stars and the label. An active filter is outlined |
| Grid | Draws only the visible images, in folder order. Layout, visible range, keys and scrolling work on positions in the visible list; thumbnails keep their file-index keys, so a filter change reloads nothing |
| Loupe | Steps within the visible list; Home/End go to the first and last visible image; the status line shows "3 / 12 (av 31)" |
| Selection model | Marking, anchor, click / Ctrl+click / Shift+click, Shift+navigation, Ctrl+A, Escape, and the rating targets (§2.5, §2.6). Plain Rust |
| Batch ratings | `RatingStore` applies one key to several paths with the batch rules (§2.7, §4.3) |
| Display | Marked cells drawn differently from the current cell (§5.1); "k markerade" in the status line |
| Tabs | Each tab keeps its own filter (Q1), marking and anchor |
| Texts | Swedish, as in the Tauri app |

### 3.2 Out of scope

- Sorting (by rating, date or name other than today's order).
- Filters by file type, date, camera, keyword or other metadata (ADR 0007).
- Saving the filter between sessions (Q2): needs a settings format, which does not exist yet
  (`persistence` is off, GUI_REQUIREMENTS §6).
- Moving rejected or picked images to `_rejected` / `_picked` (ADR 0013 §13).
- Marking several images in the loupe, and drag-to-mark (a rubber band) in the grid.
- Folders much larger than the test set (1 000+ images); measured when that milestone comes.

## 4. Architecture

### 4.1 Where the code goes

| Part | Crate | Notes |
|---|---|---|
| `Filter`, `StarFilter`, `LabelFilter`, `matches`, `visible(files, ratings, filter, keep)` | `pw-loupe` (library), new `filter.rs` | No I/O. Takes the shown rating per path through a closure, so it is tested without `RatingStore` |
| `Selection { marked, anchor }` with `click`, `ctrl_click`, `shift_click`, `extend_to`, `all`, `clear`, `retain_visible`, `targets` | `pw-loupe` (library), new `selection.rs` | Works on file indexes; positions come from the visible list |
| Batch rating: `RatingStore::set_many` (§4.3) | `pw-loupe` (library), `ratings.rs` | The write queue is unchanged |
| Tab state: `filter`, `selection` in `Tab`; `current` stays a file index | `pw-loupe` (library), `tabs.rs` | |
| Keeping loupe caches when the list changes (Q3) | `pw-viewcore` | A new `set_files(files, current)`; see §4.4 |
| Drawing: filter controls, marked cells, status texts; wiring clicks and keys | `pw-loupe` (binary) | egui, no new crates |

### 4.2 Positions and file indexes

- The tab's `files` and `current` (a file index) do not change. The visible list is a
  `Vec<usize>` of file indexes, recomputed when the filter, a rating or the folder changes, and on a
  move (§4.2 third point). It is never recomputed by drawing alone.
- The grid model (`navigate`, `visible_range`, `columns`) keeps working on positions `0..n`; the
  wiring maps position ↔ file index through the visible list. `ThumbGrid::to_request` takes the file
  indexes of the visible range instead of a `Range`.
- **Recompute after a rating (§2.2):** after an accepted rating key, the visible list is
  recomputed without keeping any image (`Tab::rated`). A move (a key, a click, a loupe step) still
  recomputes the list keeping the current image before the move and the new current image after
  it, as `go()` in `app.js`; after §2.2 this only matters when a shown rating changed without a
  key (a rolled-back write).

### 4.3 Batch ratings

`apply` toggles per image (a label key on an image that has the label clears it; X on a rejected
image clears the reject). With several targets that would give a mix, so `set_many` decides **once**
for the whole batch and then sets an explicit value on each:

| Key | All ratable targets already have it | Otherwise |
|---|---|---|
| Label(l) | clear the label on all | set l on all |
| Reject (X) | clear the stars on all | set −1 on all |
| Stars(n), ClearLabel | – | set on all |

- Files without `supports_rating` are skipped; `set_many` returns `Unsupported` only if no target was
  ratable.
- Each target is one `set` on the existing queue, so coalescing, order, rollback and read merge work
  as in ADR 0013 §4.2. A batch of 300 images is 300 queued writes; the worker takes 8 at a time and
  the rest wait (rule 2).
- With one target, `set_many` gives the same result as today's `set` (the single-image tests stay).

### 4.4 The loupe under a filter (Q3)

The loupe must step within the visible list **and prefetch within it** (V1: +1 and +2 in the direction
of travel must be the next *visible* images). Three ways:

| | How | Effect |
|---|---|---|
| A | Give the core the visible list with `open_folder` each time it changes | Simple, but every change drops all caches: after a rating that removes an image, the next image loads cold (V1 miss) |
| B | Give the core the whole folder and map steps to file indexes | Prefetch looks at hidden images; with a sparse filter almost every step is a cold load |
| **C** | **New `ViewCore::set_files(files, current)`: the core takes the visible list and moves its caches from old positions to new ones by path; entries for paths no longer in the list are released** | Prefetch follows the filter and nothing already decoded is lost. A change in `pw-viewcore`, with tests |

Proposal: **C**. `open_folder` is unchanged and still used for a new folder or tab.

### 4.5 Keys and focus

- Filter controls take focus only while open; after a choice, focus returns to the grid or the loupe
  (GR6). The rating focus rule of ADR 0013 §4.3 applies to every key here.
- New in the grid: Shift + any navigation key (arrows, Page Up/Down, Home/End) extends; Ctrl+A marks
  all visible; Escape clears. `grid_key_action` gets a `shift` argument; the rating key map is
  unchanged (Shift+digit still does nothing).
- Ctrl+A in the loupe does nothing; A alone keeps toggling the zoom anchor.

## 5. Display

### 5.1 Contact sheet

- The current cell keeps its yellow outline. A marked cell gets a blue outline (`#4c8dff`, the
  label blue) and a faint blue fill behind the thumbnail; the current cell inside a marking has both.

### 5.2 Filter controls

- Condition: "≥", "=", "≤" (default "≥"); enabled only when a number of stars is chosen.
  Stars: "Alla betyg", "1 stjärna", "2 stjärnor", "3 stjärnor", "4 stjärnor", "5 stjärnor",
  "Ej betygsatt", "Avvisade", "Alla utom avvisade". Label: "Alla etiketter", "Röd", "Gul", "Grön",
  "Blå", "Lila", "Ingen etikett".
- An active filter has a yellow outline: the star list (and the condition list when a number of
  stars is chosen) when not "Alla betyg"; the label list when not "Alla etiketter".

### 5.3 Status line

- Grid and loupe: "3 / 12 (av 31) · 12 markerade · TN100423.ARW · …"; without a filter, as today
  ("3 / 31"). "k markerade" only when more than one image is marked (the order the owner chose on
  2026-11-10).
- Nothing visible: "Inga bilder matchar filtret (31 i mappen)", also as text in the empty grid.

## 6. Dependencies

- No new crates and no `Cargo.toml` change.

## 7. Behaviour requirements

Proposed as behaviour requirements now and targets after a first measurement, as in ADR 0011 §7 and
ADR 0013 §7:

| # | Requirement | Target |
|---|---|---|
| **S1** | Changing the filter updates the grid in the same frame, without reloading thumbnails and without I/O on the UI thread (G2) | By hand on the test set; tests for the visible list |
| **S2** | A rated image that stops matching leaves the visible list at once, in the grid and the loupe; in the loupe the next visible image is shown without a cold load when it was prefetched | Tests (`Tab::rated`); by hand in grid and loupe |
| **S3** | Stepping in the loupe under a filter stays in the filter and meets V1 for the next visible image | By hand with `latency.ps1` under "≥ 3 stjärnor" on the test set; tests for `set_files` |
| **S4** | A key on k marked images updates all k cells in the same frame; their writes reach the disk in order, with the batch rules of §4.3 | Tests with the fake writer; by hand: mark 20, press 7 twice, reopen → no label |
| **S5** | The marking never contains a hidden image, and the targets never include one | Tests |
| **S6** | Every check of `ui/tests/filter-select.test.js` that is not Tauri-specific has a `cargo test` counterpart that passes, **except the check "a rated image stays until the user moves on", which amendment 1 replaces** (the Tauri app keeps the old behaviour) | Test list in the T task |

## 8. Testing and acceptance

### 8.1 Automated (`cargo test`)

- `filter.rs`: every star and label choice against unrated, 1–5, −1, with and without a label;
  "≥ n" excludes −1; AND of both; keep-current; the current image moves to the nearest following one.
- `selection.rs`: click, Ctrl+click (adds, removes, never empties to zero from a plain current),
  Shift+click from the anchor in both directions, Shift+navigation, Ctrl+A, Escape, plain navigation
  clears, `retain_visible`, `targets` in grid and loupe.
- `ratings.rs`: `set_many` with the table of §4.3; skipped files; all unratable → `Unsupported`;
  coalescing across two batches; rollback of one failed image in a batch leaves the others.
- `pw-viewcore`: `set_files` keeps resident views and sessions of paths that remain, at their new
  positions, and releases the rest; the plan (`wanted`) after `set_files` uses the new neighbours.
- The ported `filter-select.test.js` cases (S6).

### 8.2 By hand (owner)

- Each filter on the test set; an empty filter; a filter in two tabs (Q1).
- Under "Ej betygsatt": rate the current image and see it leave at once with the next image
  current (grid and loupe); rate a marking and see all of it leave. Under "Grön": give the current
  image another label and see it leave at once.
- The star condition: "= 3", "≤ 2" (unrated included, rejects not) and "≥ 4" on the test set; the
  condition list disabled under "Ej betygsatt" and keeping its value; two tabs with different
  conditions.
- Mark a range with Shift+click and Shift+arrows, add with Ctrl+click, press a label twice; Ctrl+A;
  Escape. Include a file that cannot carry ratings.
- Loupe under "≥ 3 stjärnor": step with the arrow held; `latency.ps1` for V1 (S3).
- Close the window right after a batch of 30 (ADR 0013 R4 still holds).

## 9. Work steps

| Step | Content | Tasks |
|---|---|---|
| **F0** | Accept this ADR; answer §11; accept S1–S6 | **Done 2026-11-10** |
| **F1** | `pw-loupe`: `filter.rs` and the filter in `Tab` (plain Rust, tests) | T51 |
| **F2** | `pw-loupe`: `selection.rs` and the marking in `Tab`; `grid_key_action` with Shift (plain Rust, tests) | T52 |
| **F3** | `pw-loupe`: `RatingStore::set_many` and the batch failure text (plain Rust, tests with the fake writer) | T53 |
| **F4** | `pw-viewcore`: `set_files` that keeps caches by path (Q3; tests) | T54 |
| **F5** | `pw-loupe`: the browse logic of a tab (visible list, moves, clicks, texts) in plain Rust (T55); then the binary: filter controls, the grid over the visible list, marked cells, status texts, clicks and keys (T56) | T55, T56 |
| **F6** | `pw-loupe` (binary): the loupe under a filter with `set_files` | T57 |
| **F7** | Acceptance by hand (§8.2), including V1 under a filter | Owner |
| **F5c** | Amendment 1: a rating removes an image that stops matching at once (§2.2) | T59 |
| **F5d** | Amendment 1: the star condition ≥, =, ≤ (§2.1) | T60 |
| **F5e** | Amendment 2: several labels at once (§2.1) | T61 |
| **F8** | Documentation: architecture.md, test inventory, GR8 status, ADR 0004 addendum (egui note) | T58 |

T59 comes after T56 and before T57; T60 after T57 (all three change `main.rs`). T51–T54 do not depend on each other; T55 needs T51 and T52; T56 needs T53 and T55; T57 needs T54
and T56. Until T57, the loupe opens on the visible list with `open_folder` (§4.4, way A) as an
interim step.

## 10. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| `set_files` in `pw-viewcore` touches the cache bookkeeping (positions in `resident`, `sessions`, `sizes`, `inflight`) | A wrong image for a position, or a leak | Its own step (F4) with tests; the key stays `(position, Source)` and is remapped in one place |
| A large batch fills the worker queue | Writes wait | Already handled: the UI queue retries each frame (ADR 0013 rule 2); the cells show the value at once |
| Ratings of a tab are still loading when a filter is active | Images flash in and out | Q7: an active filter shows nothing until the read is done |
| Recomputing the visible list every frame | Wasted time in large folders | Recomputed only on a change (§4.2) |
| The keep-current rule and the marking interact (a kept image is marked) | A hidden image stays marked | `retain_visible` runs after each recompute; tests |

## 11. Questions and the owner's answers (2026-11-10)

The owner accepted every proposal below as written.

| # | Question | Decision (the proposal, accepted) |
|---|---|---|
| **Q1** | One filter for the whole window (as the Tauri app, which has one folder) or one per tab? | **Per tab.** A new tab (an import) starts without a filter, so a fresh card never looks half empty because of a filter set for another folder |
| **Q2** | Remember the filter between sessions (the Tauri app does, in browser storage)? | **Not in this milestone.** There is no settings format yet. Recorded as an accepted parity gap; it comes with settings |
| **Q3** | How the loupe steps under a filter (§4.4) | **C:** `ViewCore::set_files`, so prefetch follows the filter and decoded images are kept |
| **Q4** | Keys for the filter (GR6)? | **None in this milestone**, as in the Tauri app; GR6 lists stepping, zoom, ratings, labels and RAW/JPEG, not the filter. Keys can be added later (Shift+digit is free) |
| **Q5** | Does the marking survive a visit to the loupe? | **Yes, if the user does not step:** Enter and Escape back keep it; a step in the loupe clears it, as in `app.js` |
| **Q6** | The message when several writes of a batch fail | **One line:** "Kunde inte spara betyget för 3 bilder. Senast: TN005979.ARW: filen är skrivskyddad". One failure gives today's text |
| **Q7** | An active filter while the tab's ratings are still being read | **Show nothing yet**, with "Läser betyg …" in the grid and the status line; then apply. If the read fails, an active filter shows nothing and the status line gives the error |
| **Q8** | Extra star choice "Alla utom avvisade" (hide only rejects)? | **Yes**, as the last item of the star list. It is the common culling view, costs one match arm, and the Tauri app cannot do it today. Say no to keep strict parity |
| **Q9** | Look of marked cells (§5.1) | Blue outline and faint blue fill; current keeps yellow |

## 12. Consequences

- RawStride gets the last culling feature of the Tauri app; GR8 then lists IPTC metadata and TIFF
  viewing.
- `pw-loupe` gains two tested models (filter, selection) and batch ratings; `Tab` grows by two
  fields.
- `pw-viewcore` gains `set_files`; its other behaviour is unchanged.
- The reserved keys Ctrl+A (ADR 0009 §3.3) and Escape in the grid (ADR 0011 Q6) get their meaning.

## 13. Amendment 1 (owner, 2026-11-10)

| Point | Change | Reason | Tasks |
|---|---|---|---|
| §2.1 | A condition list (≥, =, ≤) beside the star list; "≤ n" includes unrated images; rejects never match a number of stars; "Ej betygsatt", "Avvisade" and "Alla utom avvisade" stay | Filtering on an exact number and an upper bound | T60 |
| §2.2 | A rated image that stops matching leaves at once instead of when the user moves on | The owner's culling workflow: what no longer matches should go at once | T59; T57 updated |
| §5.3 | The marking count before the file name | Owner decision | T56 (done in its review) |

The same change applies to the ADR 0004 addendum (points 1 and 2); T58 updates it. Parity with
`ui/tests/filter-select.test.js` (S6) is given up for the one check that §2.2 replaces.

## 14. Amendment 2: several labels at once (owner, 2026-11-10)

Status: **accepted by the owner (2026-11-10)**, after the hand check of T60 ("I miss filtering on
several colours at once"). Every proposal below is accepted as written. Task T61.

**Proposed change of §2.1 (label part):** the label filter is a **set** of label choices: the five
colours and "Ingen etikett". An image matches when its label is in the set (OR within the set);
stars and label still combine with AND. An empty set means "Alla etiketter".

| # | Question | Decision (the proposal, accepted) |
|---|---|---|
| **Q10** | How is the set chosen? | The label list stays one drop-down; its items become check boxes (Röd, Gul, Grön, Blå, Lila, Ingen etikett) and the popup stays open while ticking. "Alla etiketter" at the top clears the set. Closed, it shows "Alla etiketter", the one name ("Röd"), or the names joined ("Röd, Grön"), shortened to "3 etiketter" from three |
| **Q11** | Can "Ingen etikett" be combined with colours? | **Yes**: "Röd + Ingen etikett" shows red and unlabelled images (for example: what is red or not yet sorted) |
| **Q12** | All six ticked? | Treated as "Alla etiketter" (the set is cleared), so the filter is not shown as active when it lets everything through |
| **Q13** | Keys? | None, as Q4 |

Unchanged: per tab, not saved between sessions (Q1, Q2); the yellow outline when the set is not
empty (§5.2); a rating that takes an image out of the set removes it at once (§2.2).
