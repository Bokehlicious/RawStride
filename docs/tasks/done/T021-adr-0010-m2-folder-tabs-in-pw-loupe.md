# T21. ADR 0010 M2: folder tabs in pw-loupe — Done

- **Can be done in parallel with T20** (different files).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/lib.rs` (or a new module file such as
  `src/tabs.rs` declared in `lib.rs`), `crates/pw-loupe/src/main.rs`.
- **What (ADR 0010 §3.3 and §11):**
  1. **Tab model in plain Rust** (no egui types), e.g. `pub struct Tabs` with tabs that each hold
     `folder: Option<PathBuf>`, `files: Vec<PathBuf>` and `current: usize`:
     - starts with one empty tab;
     - `open_in_active(folder, files, pos)`; `open_new(folder, files, pos)`, which activates the
       existing tab instead if that folder is already open (GR12);
     - `activate(i)`, `next()` and `previous()` (wrapping), `close_active()` (the last tab becomes
       empty instead of closing);
     - `set_current(pos)` to remember the image of the active tab;
     - `titles()`: the folder name, or "Tom" for an empty tab.
  2. **Tab keys** as a plain function with tests, separate from `key_action` (which must keep
     returning `None` for Ctrl combinations): Ctrl+Tab next, Ctrl+Shift+Tab previous, Ctrl+W close.
  3. **Drawing in `main.rs`:** a tab bar above the image; clicking a tab activates it. On activation:
     `core.open_folder(files)`, `core.go(current)` and the metadata `begin_load` for that folder.
     Remember the image from `frame.pos` with `set_current`. Zoom resets on activation (Q1).
  4. **Drag-and-drop:** opens in the active tab as today; with **Ctrl** held it opens a new tab (Q3).
     **Changed 2026-11-08 (owner, after testing):** a dropped folder always opens in a new tab; an
     empty active tab is filled instead (ADR 0010 Q3).
- **Rules:** the loupe's existing keys and behaviour are unchanged (ADR 0009 §3.3); one view core for
  all tabs; no `unwrap`/`expect`/`panic!` in non-test code; no new dependencies.
- **Tests to add:** the tab model (open in active, open new, open an already open folder, next and
  previous with wrap, close the last tab, close a middle tab and which tab becomes active, remembered
  image per tab, titles) and the tab keys.
- **Done when:** `.\tools\check.ps1` is all OK; the owner checks by hand that tabs can be opened with
  Ctrl+drag-and-drop, switched with the keys and the mouse, and closed, and that each tab comes back
  on its own image.
