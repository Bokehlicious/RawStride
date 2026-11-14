# T137. `cargo audit` in the check script — Done

- **Why:** the repository has no CI and no Dependabot, so nothing warns when a crate in
  `Cargo.lock` gets a security advisory (owner, 2026-11-14: GitHub's Dependabot alerts are enough
  on GitHub; version-update PRs wait until there is CI). `cargo audit` checks `Cargo.lock` against
  the RustSec advisory database, locally, on every run of `.\tools\check.ps1`.
- **Crates:** none. **Files:** `tools/check.ps1`, `docs/building.md`, `AGENTS.md` (§5, one
  parenthesis).
- **What:**
  1. **A `cargo audit` step in `tools/check.ps1`,** after `cargo fmt --all --check` and before
     `cargo test`, run through `Invoke-CheckStep` like the other steps, so its output goes to
     `test-output.txt` and its result to the summary. A known vulnerability fails the step;
     warnings (unmaintained, unsound, yanked) do not, which is `cargo audit`'s own default. Add no
     `--deny` options and no ignore list.
  2. **Without cargo-audit** (`cargo audit --version` fails), the step is not run: the report
     gets a `=== cargo audit ===` section saying cargo-audit is not installed, with the install
     command, and the summary line `cargo audit: SKIPPED`. A skipped step does not fail the check,
     so a machine without cargo-audit (or a contributor who has not installed it) still gets an
     all-OK run.
  3. **`docs/building.md`:** a row for cargo-audit in the tools table in §1 (why: checks the
     dependencies against known security advisories; how: `cargo install cargo-audit --locked`,
     optional), and the list of steps in §4 with `cargo audit`, saying that it needs the network
     (it fetches the advisory database) and is skipped when cargo-audit is not installed.
  4. **`AGENTS.md` §5:** the parenthesis after "Verify with `.\tools\check.ps1`" lists
     the steps; add `cargo audit` to it. Nothing else in `AGENTS.md` changes.
- **Tests to add:** none (no Rust code changes). Verify by running `.\tools\check.ps1` once with
  cargo-audit installed (the step runs and is OK) and check that the report has the section and
  the summary line.
- **Not in this task:** a `.github/dependabot.yml` or CI; turning on Dependabot alerts on GitHub
  (the owner does that in the repository settings); doing anything about today's five warnings
  (`proc-macro-error`, `zbus_names`, `zvariant`, `zvariant_derive` unmaintained; `glib` 0.18.5
  unsound, RUSTSEC-2024-0429), which come from dependencies the project does not choose directly;
  auditing the `Cargo.lock` files under `prototypes/` and `tools/egui-demo/`.
- **Rules:** `docs/tasks/README.md` rules 1–12 (rule 13 does not apply: no texts the user sees).
- **Done when:** `.\tools\check.ps1` is all OK, with `cargo audit: OK`, and `git diff --stat`
  shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Added. The `check.ps1` step (points 1–2) was written before the task, in the owner's session; the task takes it over. |
| 2026-11-14 | Ready for review | Points 1–4 done. `check.ps1`: fmt, cargo audit, test, clippy all OK (1370 passed, 0 failed, 53 ignored); cargo audit 0.23.0, no vulnerabilities, 5 allowed warnings. The run included T135/T136's uncommitted changes. |
| 2026-11-14 | Done | Reviewed, no changes; owner approved. check.ps1 all OK with cargo audit OK |
