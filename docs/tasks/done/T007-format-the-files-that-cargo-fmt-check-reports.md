# T7. Format the files that `cargo fmt --check` reports — Done

- **Crates:** `photoworkflow` (`src-tauri`), `preview-bench` (`tools/preview-bench`).
- **Files:** `src-tauri/src/main.rs`, `src-tauri/src/rawview.rs`, `tools/preview-bench/build.rs`.
- **What:** run `cargo fmt --all`. Only these three files should change, and only in layout (line
  breaks, the order of the `mod` lines in `main.rs`). If any other file changes, stop and report it.
- **Done when:** `cargo fmt --all --check` prints nothing, `git diff --stat` lists only these three
  files, and `cargo test -p photoworkflow` passes with the same test count as before.
