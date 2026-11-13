# T61. ADR 0014 amendment 2: filter on several labels at once — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Can start now.** ADR 0014 §14 accepted by the owner (Q10–Q13 as proposed).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/filter.rs`, `crates/pw-loupe/src/lib.rs`
  (re-exports), `crates/pw-loupe/src/main.rs` (the label list only), and the tests of `browse.rs`,
  `tabs.rs` and `lib.rs` that build a `Filter` (the label field only).
- **Why:** ADR 0014 §2.1 and §5.2 as changed by amendment 2 (§14).
- **What:**
  1. `pub enum LabelChoice { Label(&'static str), NoLabel }` and `pub const LABEL_CHOICES:
     [LabelChoice; 6]` (the five labels in `pw_metadata::LABELS` order, then `NoLabel`);
     `label_choice_text`: the Swedish names, "Ingen etikett".
  2. `LabelFilter` becomes a set of `LabelChoice` (for example a small bit set; `Copy`, `Default` =
     empty = all). Methods: `all()`, `is_all()`, `contains(choice)`, `toggle(choice)` (ticking the
     sixth choice clears the set, Q12), `only(choice)` for tests and callers.
  3. `matches`: with an empty set, every label; otherwise the image's label (or `NoLabel` when it has
     none) must be in the set. AND with stars as today. `Filter::is_active`: stars not `All` or the
     set not empty.
  4. `label_filter_text(&LabelFilter) -> String`: "Alla etiketter"; one name; two names joined with
     ", "; "{n} etiketter" from three.
  5. `main.rs`: the label drop-down shows "Alla etiketter" (clears the set) and one check box per
     `LABEL_CHOICES` item; the popup stays open while ticking (egui's close-on-click-outside
     behaviour); each tick goes through `set_filter` as today (refilter without keeping, no widget
     keeps the keyboard focus after the popup closes). Yellow outline when the set is not empty.
- **Rules:** as T51; rules 6, 7, 10–12.
- **Tests to change:** `LabelFilter::Label(l)` / `NoLabel` / `All` in existing tests become
  `LabelFilter::only(…)` / `LabelFilter::all()`; assertions unchanged, except the label text tests,
  which get the new texts.
- **Tests to add:** every single choice as before; Red + Yellow; Red + Ingen etikett; all five
  colours (unlabelled images hidden); ticking all six clears the set; toggling twice returns to the
  start; `is_active` for the empty and a non-empty set; the texts for 0, 1, 2 and 3 choices; with the
  `browse.rs` setup (2: 5 stars Red, 4: Red only): Red + Yellow → [2, 4]; Ingen etikett →
  [0, 1, 3, 5, …, 11]; ≥ 3 and Red + Ingen etikett → [1, 2].
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: two and three colours;
  a colour plus "Ingen etikett"; ticking all six; the outline; two tabs with different sets.
