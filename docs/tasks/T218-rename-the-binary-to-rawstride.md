# T218. The program file renamed from `pw-loupe.exe` to `rawstride.exe` — Open

- **Why:** step R10 of `docs/tasks/plans/public-release.md`, the last step of the rename (ADR 0012
  decision 3, "step 2"). ADR 0012 set it "at the latest when the egui program replaces the Tauri
  app", which happened with T207 on 2026-11-19. It waited for the trademark search (PR-Q3), which the
  owner made on 2026-11-19 (ADR 0012, Consequences). Today `cargo build` makes
  `target\<profile>\pw-loupe.exe`; after this task it makes `rawstride.exe`.
- **Crates:** `pw-loupe` (`Cargo.toml` and one text constant). **Files:**
  - `crates/pw-loupe/Cargo.toml` (the `[[bin]]` table, lines 15–17)
  - `crates/pw-loupe/src/lib.rs` (`USAGE`, line 203)
  - `README.md` (line 6 and the layout block, line 69)
  - `docs/building.md` (section "3. Build and run", lines 36–43)
  - `docs/architecture.md` (the `pw-loupe` row of the crate table, line 60)
  - `docs/adr/0012-product-name-rawstride.md` (one line under "Consequences")
- **What:**
  1. `crates/pw-loupe/Cargo.toml`: in `[[bin]]`, `name = "pw-loupe"` → `name = "rawstride"`.
     `path = "src/main.rs"` and the `[package]` name `pw-loupe` stay (ADR 0012 decision 4: crate
     names are unchanged). `Cargo.lock` does not change, since it lists packages, not binaries.
  2. `crates/pw-loupe/src/lib.rs` line 203: the first word of `USAGE`, `pw-loupe [--core-mb …`
     → `rawstride [--core-mb …`. Only that word; the rest of the text, where it lives and how it is
     printed (`--help`, a bad argument; `main.rs`) stay. Rule 13 asks nothing more for this one
     word: the text is English and no glossary term changes. The test at `lib.rs` line 622
     compares with the constant itself and stays as it is.
  3. `README.md` line 6: "The new program (`pw-loupe`, window title "RawStride", …" → "The new
     program (`rawstride.exe`, built from the crate `pw-loupe`; window title "RawStride", …". Line 69:
     "the RawStride program (pw-loupe)" → "the RawStride program (crate pw-loupe, rawstride.exe)".
     The command `cargo run --release -p pw-loupe` (line 36) stays: `-p` names the package.
  4. `docs/building.md` section 3: the two `cargo run` commands stay as they are (same reason).
     The sentence "`pw-loupe` is the RawStride program (window title "RawStride")." becomes: the
     crate `pw-loupe` builds the RawStride program, `target\release\rawstride.exe` (window title
     "RawStride"); add that the built program can also be started directly with a folder,
     `target\release\rawstride.exe "D:\Photos\2026-11-14"`.
  5. `docs/architecture.md` line 60: "The egui program (until it is renamed)" → "The egui program,
     built as `rawstride.exe`". The rest of the row stays.
  6. `docs/adr/0012-product-name-rawstride.md`, after the last bullet of "Consequences":
     `- **Step 2 done (2026-11-19, T218):** the binary is `rawstride` (`rawstride.exe`); the crate is still `pw-loupe` (decision 4).`
- **Tests to add:** none (one word of a text constant; no behaviour changes). `check.ps1` builds the binary under its new name.
- **Not in this task:**
  - The `"pw-loupe: …"` prefixes of the `eprintln!` messages in `crates/pw-loupe/src/` (18): they
    name the crate, which keeps its name (decision 4). The release build has no console
    (`windows_subsystem = "windows"`, `main.rs` line 8), so a user does not see them; a debug run
    from a terminal does.
  - Older ADRs and analyses that name `pw-loupe.exe` (for example ADR 0013 line 221, ADR 0009
    line 320, `docs/analyses/2026-11-11-adr0009-s6-acceptance.md` line 10) and `docs/tasks/done/`: records of their time (ADR 0012 decision 5).
  - The owner's latency tools outside the repository (`D:\PhotoWorkflowData\pm-study\tools\s6a.ps1`
    and `S6a-guide.md`, which use the path `target\release\pw-loupe.exe` and the process name
    `pw-loupe`): the reviewer updates them after this task (plan step R10).
  - The old `target\*\pw-loupe.exe` left from earlier builds is not deleted; `cargo clean` is not run.
- **Rules:** no other file changes; if one must, stop and explain (rule 2). After the change:
  - `cargo build --release -p pw-loupe` makes `target\release\rawstride.exe` (paste
    `Get-Item target\release\rawstride.exe | Select-Object Name, LastWriteTime`);
  - `git grep -n -E "pw-loupe\.exe|\(pw-loupe\)|until it is renamed|\"pw-loupe \[" -- README.md docs/building.md docs/architecture.md crates/pw-loupe/Cargo.toml crates/pw-loupe/src/lib.rs`
    finds nothing (paste the command and its empty result).
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the six files above
  and this file's status and log; the two outputs of "Rules" are pasted in the report.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (plan step R10, after PR-Q3: the owner's TMview search 2026-11-19). Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README, this file and the named files): doable; line numbers and quoted texts right; nothing builds or runs the binary by name (no `CARGO_BIN_EXE_*`, `build.rs` uses `-bins`, the settings folder does not depend on it). Changed after it: `lib.rs` `USAGE` added (it began with `pw-loupe`), with its pattern in the `git grep`; the `eprintln!` prefixes counted as 18 and the console reason corrected (a debug run shows them); the S6 acceptance analysis added to the older records |
| 2026-11-19 | Open | Cold read done |
