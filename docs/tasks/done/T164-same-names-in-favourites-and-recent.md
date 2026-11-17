# T164. Folders with the same name told apart in Favourites and Recent (pw-loupe) — Done

- **Why:** the owner's hand check of T158 (2026-11-16): Recent seemed to list the same folder
  several times. They were different folders with the same name (`import\2025\2025-07-25`,
  `Trip\1\2025\2025-07-25`, `Trip\1\2\2025\2025-07-25`), and the list shows only a folder's
  last name. **Owner, 2026-11-16** (ADR 0017 U4-Q8): such a folder is shown with as many of its
  parent folders as make the names differ, for example "2025-07-25 (import\2025)".
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/library.rs`,
  `crates/pw-loupe/src/library_text.rs`, `docs/test-inventory.md`.
- **What:**
  1. **The text** (rule 13, `library_text.rs`): `pub fn folder_list_name_text(name: &str, parents: &str) -> String`
     gives `"{name} ({parents})"`. Update the module's doc comment (a folder name is still data, but
     this one template joins it with its parents).
  2. **The names of a list** (`library.rs`): `pub fn list_names(paths: &[PathBuf]) -> Vec<String>`,
     one name per path in the same order:
     - a path whose `display_name` no other path of the list has (compared without regard to case,
       Windows) gets `display_name` as today;
     - for a group of paths with the same `display_name`, find the smallest `k ≥ 1` for which the
       last `k` components of each path's parent, joined with `\`, are different for every path of
       the group (compared as `folder_key` compares: without regard to case on Windows). From
       `Path::components`, a `Prefix` is one component written as its `as_os_str()` (`D:`, or the
       prefix of a network path as it is), and `RootDir` is skipped. A path whose parent has fewer
       than `k` components uses all of them. Each path of the group then gets
       `folder_list_name_text(display_name, parents)`. If no `k` makes them differ, every
       component of the parent is used (the same joining);
     - a path without a parent (a drive's root, whose `display_name` is `D:\`) keeps its
       `display_name`;
     - no `unwrap`, no indexing that can panic (AGENTS.md §3).
  3. **The list keeps its names right:** `FolderList` sets every entry's `name` from `list_names`
     after each change of the list (`from_paths`, `add`, `record` after its `truncate`, `remove`,
     `clear`), so a name can change when another folder with the same name comes or goes.
     `check_folders` changes no name. `ListEntry::new` may keep `display_name` as the first value;
     the doc comment of `ListEntry::name` is updated. `main.rs` is unchanged: it already draws
     `entry.name` (with the full path as the tooltip of a folder that exists).
- **Tests to add** (`library.rs`, on paths only, nothing read from the disk except what the
  existing tests do; the Windows paths in `#[cfg(windows)]` tests, written as raw strings `r"…"`):
  - `C:\Photos\import\2025\2025-07-25`, `D:\Data\Trip\1\2025\2025-07-25` and
    `D:\Data\Trip\1\2\2025\2025-07-25` give "2025-07-25 (import\2025)",
    "2025-07-25 (1\2025)" and "2025-07-25 (2\2025)";
  - two paths `D:\a\x` and `E:\b\x` give "x (a)" and "x (b)"; `D:\x` and `E:\x` give "x (D:)" and
    "x (E:)"; `D:\a\x` and `E:\a\x` give "x (D:\a)" and "x (E:\a)";
  - names that differ only in case count as the same name: `D:\a\Foto` and `D:\b\foto` give
    "Foto (a)" and "foto (b)";
  - a name no other path has is unchanged, also when other paths in the list share a name; a
    drive's root keeps `D:\`;
  - `FolderList`: after `record` of a second folder with the same name both entries get the
    longer names, and after `remove` of one the other gets its plain name back;
  - `library_text.rs`: `folder_list_name_text("2025-07-25", r"import\2025")` exactly.
- **Not in this task:** the folder tree and the tab titles (they show one folder each); `main.rs`.
- **Rules:** AGENTS.md §3; `docs/tasks/README.md` rule 13.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (besides files of other tasks still uncommitted in the tree).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Open | Added from the owner's hand check of T158; the owner's decision is ADR 0017 U4-Q8 |
| 2026-11-16 | Open | Cold read: the names are set again after `record`'s `truncate`; the tooltip is the full path only for a folder that exists; how a `Prefix` and `RootDir` are written; a drive's root keeps its name; the fallback uses every parent component instead of "the whole parent path"; the comparison follows `folder_key`; the owner's full paths, the case test's expected texts and a same-folder-on-two-drives case given; Windows tests and raw strings; the doc comment of `ListEntry::name` |
| 2026-11-17 | Ready for review | `folder_list_name_text` in `library_text.rs`; `list_names` in `library.rs`, `FolderList` sets the names after `from_paths`, `add`, `record`, `remove` and `clear`; the tests listed added (5 in `library.rs`, 1 in `library_text.rs`); a path whose parent has no components (a relative one-part path) keeps its name like a drive's root; `check.ps1` all OK (1687 passed, 65 ignored, 0 failed) |
| 2026-11-17 | Done | Reviewed: no bugs (a `\\?\` path would show that prefix; the library stores plain paths). Reviewer's `.\tools\check.ps1` on the tree with T160, T161, T163 and T164: all OK, 1694 passed, 65 ignored, 0 failed; clippy without warnings; the inventory's header set to this commit's count (1687). The owner's hand check is done after the commit (owner, 2026-11-17). No leftovers |
