# T4. Ingest summary: only count files that were copied or already present — Done

- **Crate:** `photoworkflow`. **File:** `src-tauri/src/ingest.rs`.
- **Problem:** in `run()`, the `folders` summary counts every plan item that did not fail. When the
  user cancels, files that were never copied are still counted.
- **What:**
  - Move the folder counting into a separate function that does not need Tauri, for example
    `fn folder_summary(items: &[&pw_ingest::PlanItem], dest: &Path) -> Vec<(String, usize)>`,
    keeping the current sort order (largest first, then by name).
  - Call it only with items that are actually in the destination. `pw_ingest::Report` only
    gives counts, not which files were copied, so decide per item by checking whether
    `opt.dest.join(&item.rel)` exists. This is reliable because `pw-ingest` renames a copy to its
    final name only after it has been verified (ADR 0005, point 6). Do not change `pw-ingest`.
  - Keep the existence check outside `folder_summary`, so the function itself stays pure and
    testable.
- **Tests to add:** the sort order; items in two folders; only the items passed in are counted.
- **Done when:** `cargo test -p photoworkflow` passes.
