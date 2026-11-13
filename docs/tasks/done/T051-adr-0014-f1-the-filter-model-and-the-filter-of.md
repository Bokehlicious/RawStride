# T51. ADR 0014 F1: the filter model and the filter of a tab (plain Rust) — Done

- **Second review 2026-11-10: accepted.** The review points below are fixed.

- **Review 2026-11-10: changes requested (one point).** Everything else is accepted: `matches`,
  `visible`, `nearest_visible`, the lists and texts, the `Tab` field, the reset in `Tab::open`, and the
  tests. Still to do, in `crates/pw-loupe/src/filter.rs` only:
  1. **`AtLeast` outside 1–5 must match the way its text reads.** `star_filter_text` shows
     `AtLeast(0)` as "≥ 1 stjärna" and `AtLeast(6)` as "5 stjärnor", but `matches` uses the raw
     value (`AtLeast(0)` lets through stars ≥ 0, `AtLeast(6)` nothing). In `matches`, clamp the
     minimum to 1–5 first, so the filter does what its text says. Add a test: `AtLeast(0)` behaves as
     `AtLeast(1)` (a reject still never matches) and `AtLeast(6)` as `AtLeast(5)`, for every value in
     `STAR_VALUES`.
  Then run `.\tools\check.ps1`.

- **Can start now.** Independent of T52–T54.
- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/src/filter.rs`, declared and re-exported in
  `crates/pw-loupe/src/lib.rs`; `crates/pw-loupe/src/tabs.rs`. **Not** `main.rs` or any `Cargo.toml`.
- **Why:** ADR 0014 §2.1–§2.3, §4.1, §5.2, Q1, Q8. The filter as tested plain Rust before anything is
  drawn.
- **What:**
  1. Types:
     - `pub enum StarFilter { All, AtLeast(u8), Unrated, Rejected, NotRejected }` (`AtLeast` is 1–5;
       `AtLeast(5)` is "5 stjärnor");
     - `pub enum LabelFilter { All, Label(&'static str), NoLabel }` (`Label` holds one of
       `pw_metadata::LABELS`);
     - `pub struct Filter { pub stars: StarFilter, pub label: LabelFilter }`, `Default` = all / all,
       and `pub fn is_active(&self) -> bool` (anything other than all / all).
  2. `pub fn matches(filter: &Filter, rating: Option<&pw_metadata::Rating>) -> bool`. No rating and a
     rating without stars count as unrated. Stars: `All` → true; `AtLeast(n)` → stars ≥ n (so a reject,
     −1, never matches); `Unrated` → no stars; `Rejected` → −1; `NotRejected` → anything except −1
     (unrated included). Label: `All` → true; `Label(l)` → the label is `l`; `NoLabel` → no label.
     Stars and label combine with AND.
  3. `pub fn visible(count: usize, keep: Option<usize>, is_match: impl Fn(usize) -> bool) -> Vec<usize>`:
     the indexes `0..count` that match, in order, plus `keep` if it is `< count` (the keep-current rule,
     ADR 0014 §2.2).
  4. `pub fn nearest_visible(visible: &[usize], current: usize) -> Option<usize>`: `current` if it is
     in `visible`; otherwise the first index after it; otherwise the last one; `None` when `visible` is
     empty (§2.3).
  5. Texts (ADR 0014 §5.2), in the order of the drop-down lists:
     - `pub const STAR_FILTERS: [StarFilter; 9]` = All, AtLeast(1)…AtLeast(4), AtLeast(5), Unrated,
       Rejected, NotRejected, and `pub fn star_filter_text(f: StarFilter) -> &'static str`: "Alla
       betyg", "≥ 1 stjärna", "≥ 2 stjärnor", "≥ 3 stjärnor", "≥ 4 stjärnor", "5 stjärnor", "Ej
       betygsatt", "Avvisade", "Alla utom avvisade" (Q8);
     - `pub const LABEL_FILTERS: [LabelFilter; 7]` = All, the five labels in `LABELS` order, NoLabel,
       and `pub fn label_filter_text(f: LabelFilter) -> &'static str`: "Alla etiketter", the Swedish
       names of `label_name`, "Ingen etikett". `AtLeast` outside 1–5 gives the text of the nearest
       valid value; no panic.
  6. `tabs.rs`: `Tab` gets `pub filter: Filter`. `Tab::open` resets it to the default (a new or
     refreshed folder starts without a filter, Q1). `Tabs::set_filter(&mut self, filter: Filter)` sets
     the filter of the active tab. Other tabs keep their own filter.
- **Rules:** no I/O, no threads; no `unwrap`/`expect`/`panic!` or unchecked indexing in non-test code;
  rules 6, 7, 10–12. Existing tests are not changed.
- **Tests to add:** every star choice against unrated, 1, 3, 5, −1 (with and without a label); every
  label choice against no label, Red and Purple; AND of both; "≥ 1" never matches −1;
  `NotRejected` matches unrated; `visible` with and without `keep`, and `keep` out of range;
  `nearest_visible` for current visible, a following index, only earlier ones, empty; the text lists
  (length, order, Swedish texts); a new tab and `Tab::open` start without a filter; `set_filter` changes
  only the active tab; two tabs keep different filters across `next`/`previous`.
- **Done when:** `.\tools\check.ps1` is all OK.
