# T60. ADR 0014 amendment 1, §2.1: the star condition ≥, =, ≤ — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Start after T57 is done** (all three change `main.rs`).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/filter.rs`, `crates/pw-loupe/src/lib.rs`
  (re-exports), `crates/pw-loupe/src/main.rs` (the filter lists only), `crates/pw-loupe/src/browse.rs`
  (tests only, see below).
- **Why:** ADR 0014 §2.1 and §5.2 as changed by amendment 1 (§13).
- **What:**
  1. `pub enum StarCondition { AtLeast, Exactly, AtMost }`, `AtLeast` the default;
     `pub const STAR_CONDITIONS: [StarCondition; 3]` in that order; `star_condition_text`:
     "≥", "=", "≤".
  2. `StarFilter::AtLeast(u8)` becomes `StarFilter::Stars(u8)` (a number of stars, clamped to 1–5
     as today). `Filter` gets `pub condition: StarCondition`.
  3. `matches` for `Stars(n)`: `AtLeast` → n stars or more; `Exactly` → exactly n; `AtMost` → no
     stars (unrated) or 0 to n stars. A reject (−1) never matches `Stars(n)`. The other star choices
     ignore `condition`.
  4. `Filter::is_active`: true when `stars` is not `All` or `label` is not `All`; the condition alone
     never makes a filter active.
  5. `STAR_FILTERS`: All, Stars(1)…Stars(5), Unrated, Rejected, NotRejected. `star_filter_text`:
     "Alla betyg", "1 stjärna", "2 stjärnor", "3 stjärnor", "4 stjärnor", "5 stjärnor",
     "Ej betygsatt", "Avvisade", "Alla utom avvisade" (outside 1–5: the nearest valid value, as today).
  6. `main.rs`: a narrow condition list just before the star list (order: condition, stars, label).
     Enabled only when the star choice is a number of stars; it keeps its value while disabled.
     Yellow outline on the condition and the star list when a number of stars is chosen. A choice
     works as the other two lists (`set_filter`, refilter without keeping, no widget keeps focus).
- **Rules:** as T51; rules 6, 7, 10–12.
- **Tests to change** (approved by amendment 1): every `StarFilter::AtLeast(n)` in the tests of
  `filter.rs` and `browse.rs` becomes `StarFilter::Stars(n)` with the default condition; assertions
  unchanged, except `star_filter_list_and_texts` and `star_filter_text_outside_one_to_five_uses_the_nearest_valid_value`,
  which get the new texts.
- **Tests to add:**
  - Each condition with n = 1–5 against unrated, 1–5 stars and rejected (≤ n includes unrated, no
    condition matches a reject).
  - `Exactly` and `AtMost` outside 1–5 behave like the nearest valid value (= 0 as = 1, ≤ 9 as ≤ 5).
  - The condition is ignored for All, Unrated, Rejected and NotRejected.
  - `is_active`: All/All with each condition is inactive; `Stars(n)` with each condition is active.
  - `STAR_CONDITIONS` order and texts.
  - With the `browse.rs` setup (1: 3 stars, 2: 5 stars Red, 3: rejected, 4: Red only): = 3 → [1];
    = 5 → [2]; ≤ 3 → [0, 1, 4, 5, …, 11]; ≤ 3 and Red → [4]; ≥ 3 → [1, 2] (unchanged).
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (§8.2): "= 3", "≤ 2" and
  "≥ 4" on the test set; the condition disabled under "Ej betygsatt" and keeping its value; two tabs
  with different conditions.
