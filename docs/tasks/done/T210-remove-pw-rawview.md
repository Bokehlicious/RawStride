# T210. Remove `pw-rawview`, the Tauri app's RAW job engine — Done

- **Why:** owner's decision (2026-11-19, ADR 0009 §4.1, ADR 0006): `crates/pw-rawview` has had no
  user since the Tauri app was retired (T207). The egui program uses `pw-viewcore`, which was built
  on the P7 prototype, not on `pw-rawview` (ADR 0009 §4.1). Its `libraw` feature is turned on by
  no crate since T207, so `src/libraw.rs` and the 3 ignored tests in `tests/libraw_engine.rs` are
  no longer built or tested; only the 24 simulated tests in `tests/engine.rs` run. The code stays
  in the git history and ADR 0006 keeps the design.
- **Crates:** the workspace (`pw-rawview` is removed). **Files:**
  - removed whole: `crates/pw-rawview/` (`Cargo.toml`, `src/lib.rs`, `src/libraw.rs`,
    `tests/engine.rs`, `tests/libraw_engine.rs`), `tools/m2-step2.cmd`
  - changed: `Cargo.toml`, `Cargo.lock`, `docs/test-inventory.md`
- **What:**
  1. Check first that nothing depends on `pw-rawview`: `cargo tree --offline --workspace -i
     pw-rawview` shows only itself, and a search for `pw_rawview` and `pw-rawview` in `crates/`
     and `tools/` finds nothing outside `crates/pw-rawview/` and `tools/m2-step2.cmd`. If anything
     else uses it, stop and report.
  2. Remove `crates/pw-rawview/` and `tools/m2-step2.cmd` with `git rm -r`. This deletes the 24
     tests in `tests/engine.rs` and the 3 ignored ones in `tests/libraw_engine.rs`; that is part of
     this task and not a weakened test (`docs/tasks/README.md` rule 5), because the code they test
     is removed with them.
  3. `Cargo.toml`: remove the line `pw-rawview = { path = "crates/pw-rawview" }` from
     `[workspace.dependencies]` (about line 20). The owner approved this change (2026-11-19).
     `members` uses `crates/*` and does not change. Nothing else in the file changes.
  4. Let Cargo update `Cargo.lock` (`cargo build --workspace`); do not edit it by hand and do not
     run `cargo update`. Expected diff: the `pw-rawview` and `jpeg-encoder` entries removed (the
     latter only `pw-rawview` named, as an optional dependency), and nothing else, or only
     dependency lines of removed packages. **No kept package changes its version, checksum or
     source**; if one does, stop and report it.
  5. `docs/test-inventory.md` (change it by script, `CLAUDE.md` §4): remove the two `pw-rawview`
     rows (`tests/engine.rs` and `tests/libraw_engine.rs`, about lines 43–44), and update the
     totals in the text of line 4 (today "after T87–T207 … 1982 passed, 73 ignored") from the new
     `test-output.txt`, with "T87–T210". Expected: 1958 passed, 73 ignored (`tests/engine.rs` gives
     24 passed; `tests/libraw_engine.rs` is `#![cfg(all(feature = "libraw", windows))]` and has
     given 0 tests since T207); if not, say why in the report.
- **Tests to add:** none; this task removes code.
- **Not in this task:**
  - `pw-libraw`'s `Session::render_full` and `render_region` (AHD) stay: `pw-libraw`'s own
    `tests/regions.rs` uses them, and the `pw-libraw/src/lib.rs` comment already says "(ADR 0006)".
  - The ADRs (0006, 0008, 0009, and 0015 §137 on `jpeg-encoder`), `docs/GUI_REQUIREMENTS.md` (line 50 lists `pw-rawview` as a core
    crate) and `docs/tasks/`: the reviewer updates them. Done tasks and analyses are history.
- **Rules:** remove with `git rm`, never by editing files to empty. No other file changes; if one
  must, stop and explain (rule 2).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` (with `--staged`) shows only the
  files above and this file's status and log.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added after the owner's decision to remove `pw-rawview` (ADR 0009 §4.1). Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README and this file only): doable with the named files; nothing else in `crates/` or `tools/` names `pw-rawview` or `m2-step2`; the `Cargo.lock` diff (`pw-rawview`, `jpeg-encoder`) and the totals (1958 passed, 73 ignored) confirmed. Changed after it: the expected totals and "T87–T210" on line 4 written out, `libraw_engine.rs` gives 0 tests (its `cfg`), the done-when allows this file's own log, ADR 0015 (`jpeg-encoder`) added to the reviewer's list |
| 2026-11-19 | Open | Cold read done. Runs before T211: both change line 4 of `docs/test-inventory.md` |
| 2026-11-19 | Ready for review | `cargo tree -i pw-rawview` showed only itself, no other user in `crates/` or `tools/`. `crates/pw-rawview/` and `tools/m2-step2.cmd` removed with `git rm -r`; the workspace dependency line removed from `Cargo.toml`; `Cargo.lock` from Cargo: only `pw-rawview` and `jpeg-encoder` removed. Test inventory: the two rows removed, line 4 "after T87–T210", 1958 passed, 73 ignored, as expected. `check.ps1` all OK |
| 2026-11-19 | Done | Review: the diff holds only the brief's files; `Cargo.lock` loses exactly the `pw-rawview` and `jpeg-encoder` entries, nothing else; no `pw-rawview`, `pw_rawview` or `m2-step2` left in `crates/`, `tools/` or `Cargo.toml`. `.\tools\check.ps1` run by the reviewer: all OK, 1958 passed, 73 ignored, 0 failed, no clippy warnings. The reviewer updated `docs/GUI_REQUIREMENTS.md` (the core crates) and ADR 0015 (`jpeg-encoder`); ADR 0008's mentions are history. No hand check: nothing the program does changed. T211 Open |
