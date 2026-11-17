# T171. ADR 0017 U9b, third part: the two move settings — Open

- **Chain:** T169 → T170 → T171 → T172 → T173 → T174. The owner's answers U9-Q5–U9-Q10 are in
  ADR 0017 §6.1.
- **Why:** ADR 0017 §2.6, U9-Q5 and U9-Q10: whether X and P move images (off by default), and the
  list of companion suffixes, both remembered in `settings.txt`. The list is a `Vec<String>`, so
  the settings types can no longer be `Copy`; this task makes that change on its own, so the
  later tasks stay small (split after the cold read of the first T171, 2026-11-17).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/settings.rs`,
  `crates/pw-loupe/src/preview.rs` and `crates/pw-loupe/src/main.rs` (only the places that copy
  the settings today), `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **`move_to_subfolders: bool`** in the shell settings, `false` by default, written and read
     as `move_to_subfolders = true|false` beside `clip_highlights`, with the same handling of a
     missing or bad value.
  2. **`companion_suffixes: Vec<String>`** in the shell settings, by default `dop`, `pp3`, `on1`,
     written as `companion_suffixes = dop, pp3, on1` and read back the same way: each entry
     trimmed and in lower case; an empty entry, or one holding `.`, `/`, `\`, `:` or a space, is
     dropped; a line with no valid entry gives the default.
     A bad line is not reported (as the other settings' bad values). The two lines are written
     after the last line the shell settings write today, `move_to_subfolders` first.
  3. **Not `Copy`:** `ShellSettings` and `Settings` derive `Clone` but no longer `Copy`. Every
     place that copied them takes a `.clone()` or a reference instead, with the same behaviour.
     The cold read found (line numbers of 2026-11-17): `main.rs` 765, 780, 784–789 (a use after
     the move), 821, 2537, 3076, 3455, 4343 and the test at 7167–7176; `preview.rs` 185;
     `settings.rs`'s tests that reuse a `base` in several `..base` updates (about 1214–1238,
     1741–1754, 1845–1869, 1968–1981), which take `base.clone()` (no assertion changes). The doc
     comment that says the settings are `Copy` (`settings.rs` about 194) is corrected. Nothing
     else in those files changes.
- **Tests to add** (`settings.rs`): both settings written and read; the defaults; a missing key;
  bad suffix entries dropped and a line with none valid giving the default. The tests that list
  every key or field (`saved_file_has_the_comment_line_and_every_key`, `changed_shell()`) are
  extended with the two new ones, and nothing else in them changes.
- **Not in this task:** using the settings (T174); a page in Settings (U8 or U10b, U9-Q10).
- **Rules:** AGENTS.md §3; no new texts.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added for ADR 0017 U9b, split out after the cold read of the first T171–T172; waits for T170 |
| 2026-11-17 | Blocked | Cold read: every place that breaks without `Copy` named (`main.rs`, `preview.rs`, `settings.rs`'s tests), the doc comment, the lines' order, no report of a bad line |
| 2026-11-17 | Open | T170 is done |
