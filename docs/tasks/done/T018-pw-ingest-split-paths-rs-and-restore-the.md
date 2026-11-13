# T18. pw-ingest: split `paths.rs` and restore the explanations — Done

- **Crate:** `pw-ingest`. **Files:** `crates/pw-ingest/src/paths.rs`, new files next to it,
  `crates/pw-ingest/src/lib.rs`.
- **What:** `paths.rs` now holds three unrelated things. Split it, moving tests with their
  functions:
  - `paths.rs`: `inside`, `resolve`;
  - `eject.rs`: `drive_of`, `eject`, `eject_script`, `wait_until_gone`, `retry_until_gone`;
  - `summary.rs`: `folder_summary`.
  `lib.rs` keeps exporting the same public names.
- **Restore the fuller doc comments** that were shortened in T8, for example that `inside()`
  expects paths that went through `resolve()`, and why `resolve()` canonicalises (`\\?\` prefixes,
  `subst` drives, folders that do not exist yet). Keep the comment on `eject_script` about why
  PowerShell must stay alive.
- **Rules:** a move only: no change in behaviour, signatures, messages or test assertions; keep the
  `#[cfg(any(windows, test))]` markings.
- **Done when:** `cargo test -p pw-ingest -p photoworkflow` passes with the same test counts as
  before and clippy is clean.
