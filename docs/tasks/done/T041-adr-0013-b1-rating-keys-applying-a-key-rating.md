# T41. ADR 0013 B1: rating keys, applying a key, rating texts (plain Rust) — Done

- **2026-11-09:** the agent did points 1–3; the review asked for point 4 and more tests, and the
  reviewer (Claude) finished them. The owner's `.\tools\check.ps1` run passed.

- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/src/ratings.rs`, declared and re-exported in
  `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`, `metadata.rs` or any `Cargo.toml`.
- **Why:** ADR 0013 §4.1. The keys of ADR 0004 (and X, ADR 0013 Q5) as tested plain Rust, before
  anything is drawn or written.
- **What:**
  1. `pub enum RatingKey { Stars(i8), Label(&'static str), ClearLabel, Reject }` (`Stars` is 0–5;
     `Label` holds one of `pw_metadata::LABELS`).
  2. `pub fn rating_key_action(key: egui::Key, ctrl: bool, shift: bool, alt: bool) -> Option<RatingKey>`,
     with the same `(key, ctrl, shift, alt)` that `main.rs` already collects:
     - **Shift held → `None`.** Ctrl **and** Alt (AltGr) → `None`.
     - No modifier: `Num0`–`Num5` → `Stars(0..=5)`; `Num6`–`Num9` → `Label` Red, Yellow, Green, Blue;
       `X` → `Reject`.
     - Alt only: `Num0`–`Num5` → `Stars`; Alt+6–9 and Alt+X → `None`.
     - Ctrl only: `Num0` → `ClearLabel`; `Num1`–`Num5` → `Label` Red … Purple; Ctrl+6–9 and Ctrl+X →
       `None`.
     - Every other key → `None`.
     (egui reports the number pad digits as `Num0`–`Num9` too; this is checked by hand in T43.)
  3. `pub fn apply(current: &pw_metadata::Rating, key: RatingKey) -> pw_metadata::Rating`:
     `Stars(0)` clears the stars (also a reject) and keeps the label; `Stars(n)` sets them and keeps the
     label; `Label(l)` sets `l`, or clears the label if it already is `l`; `ClearLabel` clears it;
     `Reject` sets the stars to −1, or clears them if they already are −1, and keeps the label.
  4. Texts (Swedish, ADR 0013 Q2):
     - `pub fn label_name(label: &str) -> &str`: Red → "Röd", Yellow → "Gul", Green → "Grön",
       Blue → "Blå", Purple → "Lila"; any other text is returned as it is.
     - `pub fn status_text(rating: Option<&Rating>) -> String`: "Betyg: " followed by "Avvisad" (−1),
       "1 stjärna", "{n} stjärnor" or "Ej betygsatt", then ", {label_name}" when there is a label.
       Examples: "Betyg: 3 stjärnor, Röd", "Betyg: Avvisad", "Betyg: Ej betygsatt, Blå".
     - `pub fn star_glyphs(stars: Option<i8>) -> String`: "" for `None`, "✕" for −1, otherwise `n` × "★"
       followed by `5 − n` × "•".
     - `pub const UNSUPPORTED_TEXT: &str = "Betyg och etiketter kan inte sparas för den här filtypen.";`
- **Rules:** no I/O, no threads; no `unwrap`/`expect`/`panic!` or unchecked indexing in non-test code.
  `key_action` and `grid_key_action` are **not** changed (digits and X keep returning `None` there).
- **Tests to add:** every row of point 2, including Shift+digit, Ctrl+Alt+digit, Alt+6, Ctrl+6, Ctrl+X
  and an unrelated key; `key_action` and `grid_key_action` still return `None` for `Num0`–`Num9` and
  `X`; every rule of point 3 (including Stars(0) on a rejected image and a label toggle on an image
  with stars); the texts for −1, 1, 3, `None`, and with and without a label; `label_name` of an
  unknown label.
- **Done when:** `.\tools\check.ps1` is all OK.
