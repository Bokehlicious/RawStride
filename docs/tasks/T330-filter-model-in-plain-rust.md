# T330. ADR 0022 S1: the filter model in plain Rust — Open

- **Why:** ADR 0022 (accepted 2026-11-17) adds a quick search and an extended filter on text and
  metadata. Step S1 is the part that needs no file, no ExifTool and no window: which fields exist,
  which conditions a field allows, and whether one image's values match. It is the first task of
  the plan's order 3 (`docs/tasks/plans/adr-0018-0025-steps.md`, row S1); S2 (reading the values),
  S3 (the search box) and S4 (the window) build on it. **Nothing in the program changes yet:** the
  new module is not called from anywhere.
  This is also the first task given to a Codex agent (owner, 2026-11-25), so it is written to be
  done from this file alone.
- **Crate:** `pw-loupe` (the library part). **Files:** `crates/pw-loupe/src/meta_filter.rs`
  (new, with its tests in a `#[cfg(test)] mod tests` at the end, as `filter.rs` has them),
  `crates/pw-loupe/src/lib.rs` (one added line), and this task's own file (status and log).
- **Model:** Codex — the owner's test of letting Codex implement a task (2026-11-25). The task is
  one layer, one new file, plain Rust, no FFI, no concurrency and no safe write.
- **Read first:** `AGENTS.md`, `docs/tasks/README.md` (How to work on a task), `docs/TESTING.md`,
  and ADR 0022 §2.1–§2.5 (`docs/adr/0022-filter-on-text-and-metadata.md`; read it, do not edit
  it). `crates/pw-loupe/src/filter.rs` (the star and label filter, ADR 0014) shows the style: plain
  Rust, no I/O, a doc comment on every public item. The new module is **beside** it, not a change
  to it.
- **What:**
  1. **`lib.rs`:** add `pub mod meta_filter;` after `mod loupe_text;` (alphabetical). It is `pub`
     so that the not-yet-used items raise no `dead_code` warning; add no `#[allow]`.
  2. **`meta_filter.rs`:** a module doc comment (ADR 0022 §2, "plain Rust, no I/O (GR11)") and
     these public items, each with a doc comment. Derives: `Field`, `Group`: `Debug, Clone, Copy,
     PartialEq, Eq, Hash`; `Outcome`, `RowError`: `Debug, Clone, Copy, PartialEq, Eq`; `Value`,
     `Condition`, `Row`, `ImageValues`, `Filter`: `Debug, Clone, PartialEq` (floats forbid `Eq`);
     `Filter` also `Default`:
     - `enum Group { Name, Exif, Iptc }`: where a field's value comes from. `Name` is always known.
     - `enum Field` with exactly these 19 variants: `FileName`; Exif: `Camera`, `Lens`,
       `FocalLength`, `Aperture`, `ExposureTime`, `Iso`, `CaptureDate`, `FileType`; IPTC: `Title`,
       `Headline`, `Description`, `Keywords`, `PersonShown`, `Creator`, `CopyrightNotice`, `City`,
       `Country`, `Location`. `pub const ALL: [Field; 19]` and `pub fn group(self) -> Group`
       (`FileName` → `Name`, the eight Exif fields → `Exif`, the ten IPTC fields → `Iptc`).
       **No user-visible texts** (names for the window, glossary entries) in this task: S3 and S4
       make them in their text modules (README rule 13).
     - `enum Value { Text(String), List(Vec<String>), Number(f64), Date(i64) }`. A `Date` is whole
       seconds since 1970-01-01 00:00:00 with no time zone (the caller converts; this module does
       no date parsing). A `Text` counts as a list of one element. `Keywords` and `PersonShown`
       are normally `List`.
     - **Which `Value` a field takes** (any other `Value` for it counts as absent): `Number` for
       `FocalLength`, `Aperture`, `ExposureTime` and `Iso`; `Date` for `CaptureDate`; `Text` or
       `List` for every other field (`Number` and `Date` are absent there).
     - `enum Condition { Contains(String), Is(String), IsNot(String), OneOf(Vec<String>),
       AtLeast(f64), AtMost(f64), Between(f64, f64), HasNo }`. On a `Date` field the numbers are
       the same seconds, compared as `f64` (exact below 2^53).
     - `enum RowError { NotAllowed, EmptyValue, NotANumber }` (with `std::fmt::Display` and
       `std::error::Error`; the text of `Display` is a developer message, not shown to the user).
     - `struct Row` with private fields (the texts are stored as given, not trimmed), `Row::new(field: Field, condition: Condition) ->
       Result<Row, RowError>`, `field()` and `condition()`. **Which conditions a field allows**
       (anything else is `NotAllowed`; **`NotAllowed` is checked first**, then `EmptyValue`, then
       `NotANumber`):

       | Field | Allowed conditions |
       |---|---|
       | `FileName`, `Headline`, `PersonShown`, `Creator`, `Location` | `Contains`, `Is`, `IsNot` |
       | `Title`, `Description`, `CopyrightNotice` | `Contains`, `Is`, `IsNot`, `HasNo` |
       | `Camera`, `Lens`, `City`, `Country` | `Contains`, `Is`, `IsNot`, `OneOf` |
       | `Keywords` | `Contains`, `Is`, `IsNot`, `OneOf`, `HasNo` |
       | `FileType` | `OneOf` |
       | `FocalLength`, `Aperture`, `ExposureTime`, `Iso`, `CaptureDate` | `AtLeast`, `AtMost`, `Between` |

       `Contains`, `Is` and `IsNot` with a text that is empty after trimming, and `OneOf` with no
       element or only empty ones, are `EmptyValue`; empty elements of a longer `OneOf` are
       kept in the row but never match anything. A `NaN` in `AtLeast`, `AtMost` or `Between`
       is `NotANumber`. `Between(a, b)` with `a > b` is stored as `Between(b, a)`; both ends are
       included.
     - `enum Outcome { Match, NoMatch, Unknown }`.
     - `struct ImageValues`, one image's values: `ImageValues::new(file_name: &str)`;
       `set_exif(&mut self, values: Vec<(Field, Value)>)` and `set_iptc(...)` replace a whole group
       and mark it **read** (a field of another group in the list is ignored; a field not in the
       list is **absent**, which is not the same as unread); `is_read(&self, group: Group) -> bool`
       (`Name` is always true); `set_value(&mut self, field: Field, value: Option<Value>) -> bool`
       sets or removes one value for RawStride's own writes (ADR 0022 S-Q8) and returns `false`,
       doing nothing, for `FileName` or a field of a group not read yet; `value(&self, field:
       Field) -> Option<&Value>`. The file name is not a stored `Value`: `FileName` is answered
       from the name given to `new`, which is the file name with its extension, no path.
     - `struct Filter` (`Default` = nothing active): `set_search(&mut self, text: &str)`,
       `search(&self) -> &str` (the text as given), `push(&mut self, row: Row)`,
       `remove(&mut self, index: usize) -> bool`, `clear(&mut self)` (search and rows),
       `rows(&self) -> &[Row]`, `is_active(&self) -> bool` (a search with at least one word, or at
       least one row), `condition_count(&self) -> usize` (the rows only, for "Filter: {n}
       conditions", ADR 0022 §2.5), `needs(&self, group: Group) -> bool` (an inactive search needs nothing; whether the active search
       or a row needs that group's values: the search needs `Iptc`; a row needs its field's group;
       `Name` is never "needed" in this sense and returns `false`; this is how S2 reads only what
       is asked for, S-Q7), `outcome(&self, image: &ImageValues) -> Outcome` and `matches(&self,
       image: &ImageValues) -> bool` (`true` only for `Outcome::Match`, so an unknown image is
       hidden, ADR 0022 §2.4).
  3. **Matching rules** (what `outcome` does; the same rules for the search's words and the rows):
     - **Text compare:** both sides are trimmed and lower-cased with `str::to_lowercase`. Nothing
       else is normalised (no accent folding). `Contains` is a substring test, `Is` is equality of
       the whole text. A `List` is tested element by element: `Contains` and `Is` match if any
       element does; `IsNot` matches if **no** element equals the text.
     - **Absent value** (the group is read, the field is not in it, or it is the wrong `Value`
       type for the field): `Contains`, `Is`, `OneOf`, `AtLeast`, `AtMost` and `Between` give
       `NoMatch`; `IsNot` gives `Match`; `HasNo` gives `Match`. A present value that is only
       empty or white space (an empty `Text`, a `List` of such) counts as absent.
     - `OneOf`: the value (or any element of a list) equals any of the given texts, compared as
       above. `FileType` values are `Text` such as `"RAW"`, `"JPEG"`, `"TIFF"`, `"DNG"`.
     - `HasNo`: `Match` if the value is absent (as above), else `NoMatch`.
     - `AtLeast(x)`, `AtMost(x)`, `Between(a, b)` on a `Number` or `Date` (as `f64`), ends
       included; a `NaN` never matches.
     - **Unknown:** a row whose field's group is not read gives `Unknown`, whatever its
       condition. `FileName` is always known. The quick search needs `Iptc`: if it is not read the
       search gives `Unknown`, even if the file name already contains every word.
     - **Quick search** (ADR 0022 §2.2): the text is split at white space (`char::is_whitespace`)
       into words, lower-cased; no words = not active = `Match`. The search matches if **every**
       word is contained in **at least one** of: the file name, and, from the IPTC group,
       `Title`, `Headline`, `Description`, `Keywords`, `PersonShown`, `Location`, `City`,
       `Country` and `Creator` (a list's elements each). Words may be found in different fields.
     - **Combining** the search and all rows: any `NoMatch` gives `NoMatch`; otherwise any
       `Unknown` gives `Unknown`; otherwise `Match`. A filter that is not active, and an inactive
       search inside an active filter, give `Match` (never `Unknown`) for every image.
  4. No panics: no `.unwrap()`, `.expect()`, `panic!`, `unreachable!` or direct indexing in the
     code outside `#[cfg(test)]` (`AGENTS.md` §3). `Filter::remove` with a bad index returns
     `false`.
- **Tests to add** (in `meta_filter.rs`, `#[cfg(test)] mod tests`; every name below is a test, a
  helper may build an `ImageValues` with both groups read):
  1. `Field::ALL` has 19 distinct fields and `group()` gives 1 `Name`, 8 `Exif` and 10 `Iptc`.
  2. `Row::new`: every allowed pair of the table above is `Ok` (one test over the whole table) and
     every other pair of the 19 fields × the conditions is `NotAllowed` (loop over `Field::ALL` and
     one example of each condition, with a valid value such as `"x"`, `1.0` or `vec!["x".into()]`; the expected result comes from the table written out in the
     test, not from the code under test).
  3. `Row::new`: `Contains("  ")` is `EmptyValue`; `OneOf(vec![])` and `OneOf(vec![" ".into()])`
     are `EmptyValue` (on `Title` and `Keywords`); `AtLeast(f64::NAN)` on `Iso` is `NotANumber`;
     `Contains("")` on `Iso` is `NotAllowed`; `Between(5.0, 2.0)` on `Aperture` becomes
     `Between(2.0, 5.0)`.
  4. Text: `Contains` is case-blind and part of a word (`"Sunset"` contains `"sun"`, with a `Title`);
     `Is` needs the whole text; `Is("  sunset ")` equals `"SUNSET"`; `"Ä"` and `"ä"` are equal,
     `"e"` and `"é"` are not.
  5. Lists: `Contains` and `Is` on `Keywords` match one element; `IsNot` on `Keywords` is `Match`
     only if no element equals; `OneOf` on `Keywords` matches any element.
  6. Absent values: with both groups read and the field missing, `Contains`, `Is`, `OneOf` are
     `NoMatch`, `IsNot` and `HasNo` are `Match`; an empty `Text("")` and `List(vec![" "])` count
     as absent; a `Text` in `Iso` counts as absent and `AtLeast` gives `NoMatch`.
  7. `HasNo` on `Title`, `Description`, `Keywords` and `CopyrightNotice`: `Match` when absent,
     `NoMatch` when present.
  8. Numbers and dates: `AtLeast`, `AtMost` and `Between` include their ends on `Aperture`
     (`Number(2.8)`), `ExposureTime` (`Number(0.004)`), `Iso` and `FocalLength`, and on
     `CaptureDate` with `Date(…)` values (two dates 60 s apart; `Between` containing both); `NaN`
     in the **value** never matches.
  9. `FileType`: `OneOf(vec!["raw".into(), "DNG".into()])` matches `Text("RAW")` and `Text("dng")`
     and not `Text("JPEG")`.
  10. Unknown: with the `Exif` group unread, a `Camera` row is `Unknown`; with `Iptc` unread, a
      `Title` row is `Unknown` even for `HasNo`; a `FileName` row is never `Unknown`; after
      `set_exif` the same row gives `Match` or `NoMatch`; `is_read` follows `set_exif`/`set_iptc`.
  11. `set_value`: sets and removes one value after the group is read (`true`); `false` for
      `FileName` and for an unread group; the change is seen by `outcome`.
  12. Quick search: empty text and `"   "` are not active and `Match` (also for an image with
      nothing read); `"sun beach"` matches when `sun` is in the `Title` and `beach` in the
      `Keywords`, and not when `beach` is nowhere; a word in the file name only matches; a word in
      `Location`, `City`, `Country`, `Creator`, `PersonShown`, `Headline`, `Description` matches;
      a word only in `CopyrightNotice` or `Camera` does **not** match; with `Iptc` unread the
      result is `Unknown` even if the file name has every word; `matches` is `false` then.
  13. Combination: search `Match` + a row `NoMatch` is `NoMatch`; a row `Unknown` + a row
      `NoMatch` is `NoMatch` (No before Unknown); only `Unknown` and `Match` give `Unknown`; all
      `Match` gives `Match`; `needs(Iptc)` is `true` for an active search, for a `Title` row and
      false for a `Camera` row alone and for an empty search; `needs(Exif)` the reverse; `needs(Name)` is always `false`.
  14. `Filter` bookkeeping: `Default` is not active, `condition_count()` is 0; one row makes it
      active and 1; a search alone makes it active but `condition_count()` stays 0; `remove(0)`
      returns `true` once and `false` on a bad index; `clear()` empties both; two rows give 2.
  15. Row order does not change the result (for these three images: all read with `Title` "Sunset", `Iso` 100; all read with no `Title`,
      `Iso` 800; `Iptc` unread; and the rows `Title` contains "sun", `Iso` `AtLeast(50.0)`, pushed
      in both orders: the same `Outcome` for each image).
- **Not in this task:**
  - reading any value (ExifTool, `pw_raw::inspect`, the metadata worker): S2;
  - the search box, Ctrl+F, the status line "Reading metadata {i} of {n}": S3;
  - the "Filter…" window, value lists with counts (S-Q10), "Show All", names of fields as the user
    sees them: S4;
  - saved filters and `settings.txt`: S5;
  - joining the new filter to the star and label filter, `visible(…)`, `Tab`, `main.rs`: S3;
  - OR between rows, regular expressions, accent folding, date parsing (ADR 0022 §5).
- **Do not touch:** any other file. In particular: `crates/pw-viewcore/src/state.rs` and the rest
  of `pw-viewcore`; `crates/pw-loupe/src/filter.rs`; any ADR or `AGENTS.md`; `tools/`, `.github/`
  and any CI file; every `Cargo.toml` (**no new dependency**) and `Cargo.lock`; `CHANGELOG.md`;
  `docs/test-inventory.md` (**an exception to README rule 12 for this task:** the reviewer updates
  the inventory and its counts after the merge, so several parallel changes cannot clash on it;
  put the new tests' count and a one-line description in your report instead). If another file
  seems to need a change, stop and ask.
- **Rules:** README rules 1–11 and 13 apply, except that **this task overrides rule 1** (you
  work in the worktree on `task/T330`, not on `feature/gui-workspace`). **Never run `git commit`, `git add`, `git stash`,
  `git reset` or `git checkout`, and never create a branch or a checkpoint:** you work in a git
  worktree on `task/T330`, and the reviewer commits. **Stop and ask the owner whenever something
  is unclear, contradicts the code or ADR 0022, or needs a file outside the list;** do not guess
  and do not leave a point out. Do not weaken a test to pass it. Build only in the worktree's own
  `target` folder. No personal data (user folders, e-mail addresses) in any file.
- **Report (as the task's README rule 9, plus):** every file changed and why; the exact summary
  from `test-output.txt` after `.\tools\check.ps1` (if the script cannot run in your sandbox, say
  so, run `cargo fmt --all -- --check`, `cargo build --workspace` and `cargo test --workspace`,
  paste their summaries, and say that `check.ps1` is left to the reviewer); the number of tests
  added and a one-line description for the inventory; any `cargo clippy` warning; every doubt,
  and each place where you had to choose.
- **Done when:** `.\tools\check.ps1` is all OK (or as the report says), `git status` shows only
  `crates/pw-loupe/src/meta_filter.rs` (new), `crates/pw-loupe/src/lib.rs` and this task's file,
  and `git diff docs/tasks/` shows only the status and one new Log row.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Open | Written (ADR 0022 S1; the first task for Codex, in its own worktree `C:\Development\PhotoWorkflow-T330` on `task/T330`). Reviewer's reading: `filter.rs` is the star and label filter and stays unchanged; no existing code calls the new module; `pw-loupe` has no `meta_filter` yet. Chain review: not needed (a single task, no other task between S1 and S2 touches it). Cold read (fresh agent): found the value type per field, the order of the `Row::new` checks, empty `OneOf` elements, stored texts, file name form, `needs` with an inactive search, the derives, rule 1 for the worktree, a typo and a vague test 15 — all fixed in the text. The README's index row is added by the reviewer; the README's Model line lists only Sonnet and Opus, so `Codex` is a one-off for this test. |
