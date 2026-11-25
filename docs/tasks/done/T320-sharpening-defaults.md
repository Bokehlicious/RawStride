# T320. ADR 0027 W3: sharpening's defaults 80 %, 0.8 px and 2 — Done

- **Why:** W3's hand check (owner, 2026-11-25; ADR 0027 §3.3, W3-Q7 and §4.3, "W3's hand
  check"): the defaults amount 60 %, radius 0.6 px and threshold 2 sharpen too little. The new
  defaults are **amount 80 %, radius 0.8 px, threshold 2**. The first task after W3's hand check;
  T321 waits for it.
- **What the reviewer checked (2026-11-25):** the defaults live in one place,
  `pw_viewcore::Sharpening::DEFAULT` (`crates/pw-viewcore/src/sharpen.rs`, about line 27);
  everything else uses the constant. Literal 60 / 0.6 values that stand for the default are
  found only in tests and comments:
  - `sharpen.rs`: the doc comment of `DEFAULT` (about line 26) and the test
    `bad_input_and_parameters_give_none` (about lines 476–477:
    `assert_eq!(Sharpening::default(), s(60, 6, 2))` and `assert_eq!(Sharpening::DEFAULT.radius(),
    0.6)`). `RADIUS_2` (amount 60, radius 2.0) is a test value of its own, not the default, and
    stays.
  - `crates/pw-loupe/src/settings.rs`: the comment beside `sharpening: Sharpening::DEFAULT`
    (about line 341), and the tests that hold the default file's text or look for it:
    `sharpen_amount = 60\nsharpen_radius = 0.6\nsharpen_threshold = 2\n` at about lines 1301,
    2499, 2899 and 3042, and `"\nsharpen_amount = 60\n"` / `"\nsharpen_radius = 0.6\n"` at about
    lines 2552–2553.
  - `crates/pw-loupe/src/preview_text.rs`: the tests of the mark's hover text,
    `"Sharpening (U): amount 60%, radius 0.6 px, threshold 2."` and `"Sharpening: amount 60%,
    radius 0.6 px, threshold 2."` (about lines 643, 644 and 711).
  - `docs/test-inventory.md`: the descriptions of T317's settings test (`sharpen_amount = 60`,
    `sharpen_radius = 0.6`) and T318's mark test ("with the default values (60, 0.6, 2)").
  - The grey-step test (`a_grey_step_gets_darker_and_brighter_next_to_it_and_stays_grey`) still
    holds at radius 0.8: the kernel reaches 3 pixels from the step (x 17–22), inside the
    unchanged ranges it checks (x 0–9 and 30–39).
- **Crates:** `pw-viewcore`, `pw-loupe`. **Files:** `crates/pw-viewcore/src/sharpen.rs`,
  `crates/pw-loupe/src/settings.rs`, `crates/pw-loupe/src/preview_text.rs`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. `Sharpening::default()` follows through `impl Default` (about line 63); nothing else in
     `sharpen.rs` changes. `Sharpening::DEFAULT` becomes `amount: 80, radius_tenths: 8, threshold: 2`, and its doc
     comment says "Amount 80 %, radius 0.8 px, threshold 2 (ADR 0027 §3.3)".
  2. The comment in `settings.rs` beside `sharpening: Sharpening::DEFAULT` says the same values.
  3. The tests above that hold the **default** as literal values are updated to 80, 8 / 0.8 and
     2 (asked for here, not a weakening, rule 5): in `sharpen.rs` `s(80, 8, 2)` and `0.8`; in
     `settings.rs` `sharpen_amount = 80`, `sharpen_radius = 0.8`; in `preview_text.rs` "amount 80%,
     radius 0.8 px". No other test is changed. A test that fails for another reason is reported,
     not changed.
  4. `docs/test-inventory.md`: the two descriptions name the new values, and the description of
     the ignored timing test `timing_of_sharpening` (`pw-viewcore`'s row, "at radius 0.6 and
     2.0") says radius 0.8 and 2.0, since it times `Sharpening::DEFAULT` and `RADIUS_2`.
- **Tests to add:** none: the tests of item 3 check the new defaults (the default value itself,
  the default `settings.txt`, Restore Defaults through `Sharpening::DEFAULT`, and the hover text).
- **Not in this task:** sharpening at 100 % and above (T321); the button (T323). A `settings.txt`
  that already holds `sharpen_amount = 60` (written by an earlier Save) keeps those values: they
  are the user's saved values, and Restore Defaults on the Settings page Preview gives the new
  ones. No `CHANGELOG.md` line: sharpening is unreleased and its line names no values.
- **Rules:** `AGENTS.md`, `docs/tasks/README.md`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Open | Added after W3's hand check (owner, 2026-11-25) |
| 2026-11-25 | Open | Cold read (fresh agent): every line number and claim checked against the code; no test compares sharpened bytes with stored ones. Added: the timing test's description in `test-inventory.md` (item 4) and that `Sharpening::default()` follows (item 1). The task files and the owner's ADR changes are committed before an agent starts, so `git diff --stat` holds only this task's files |
| 2026-11-25 | Ready for review | `Sharpening::DEFAULT` is 80 / 8 / 2 with its doc comment; the comment in `settings.rs`; the default literals in the tests of item 3 (`sharpen.rs` 2, `settings.rs` 6 lines, `preview_text.rs` 3); `test-inventory.md` 3 descriptions (T317, T318, the timing test). No other test changed, none failed. `.\tools\check.ps1` all OK (2670 passed, 0 failed, 81 ignored; no clippy warnings). `git diff --stat` shows only this task's files |
| 2026-11-25 | Done | Reviewed: only the files named; the 60 / 6 literals left in `sharpen.rs` are test values outside the ranges (`s(60, 6, 21)` and its neighbours) and `RADIUS_2`, as the task says. ADR 0027 §3.3 and §4.3 already hold the new values (6419193). `.\tools\check.ps1` all OK (the agent's run, newer than the last change; no clippy warnings). No `CHANGELOG.md` line, no leftovers. T321 set Open |
