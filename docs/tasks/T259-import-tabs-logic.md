# T259. ADR 0026 F4: what the import's tabs need to know (pw-loupe logic) — Open

- **Why:** ADR 0026 §3.4 and what was settled on 2026-11-21: the shell opens and fills the
  destination tabs from the import's events (`ImportEvent::Planned` and `Placed`, T258), leaves out
  of a listing the destination images the import has not shown yet, opens later folders' tabs
  behind the active one, and at the end makes the newest folder's tab active only if the user has
  not moved (F-Q10). This task writes that bookkeeping as plain, tested Rust; the shell uses it in
  T260. Second task of the import part T258 → T259 → T260.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/import_tabs.rs` (new: points 1–3, and their tests)
  - `crates/pw-loupe/src/tabs.rs` (point 4, and its tests; `same_folder_lexically` made `pub`)
  - `crates/pw-loupe/src/lib.rs` (the module and its `pub use`)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **`ImportTabs`** (new module `import_tabs.rs`, a module doc comment naming ADR 0026 §3.4):
     one running import's images and folders. Plain data, no file access: every folder
     comparison in this module uses `same_folder_lexically` (`tabs.rs`, made `pub` by this task,
     its doc comment unchanged in meaning: other case and a trailing separator do not matter, a
     `subst` drive or a `\\?\` prefix does), not `same_folder`, which reads the disk. With doc
     comments:
     - `pub fn new(planned: Vec<PathBuf>) -> Self`: the destination images the import will put in
       place (`Planned`), none placed yet;
     - `pub fn place(&mut self, images: &[PathBuf]) -> Vec<PlacedInFolder>`: records `images`
       (one `Placed` event) as placed and returns them grouped by their parent folder, the groups
       in the order their folders first come in `images` and each group's images in `images`'
       order; folders are told apart by `same_folder_lexically`, and a group's `folder` is the
       parent as the first of its images writes it. `PlacedInFolder { folder: PathBuf, images:
       Vec<PathBuf>, first: bool }` (derives `Clone, Debug, PartialEq, Eq`): `first` is true when
       no image of that folder was placed earlier in this import. An image placed but never
       planned is recorded as placed and grouped like the others. An image without a
       parent folder is left out. The folders are remembered in the order of their first
       placement (`pub fn folders(&self) -> &[PathBuf]`);
     - `pub fn waiting(&self, path: &Path) -> bool`: `path` is a planned image not placed yet.
       Paths are matched by folder with `same_folder_lexically` and by file name compared
       without regard to ASCII case, so a tab's own spelling of the folder matches the import's;
     - `pub fn leave_out_waiting(&self, listing: Vec<PathBuf>) -> Vec<PathBuf>`: `listing`
       without the paths `waiting` names, in its order.
  2. **`with_placed`** (`pub fn with_placed(folder: &Path, files: &[PathBuf], images: &[PathBuf])
     -> Vec<PathBuf>`): a tab's new listing after a `Placed` event: `files` (the tab's files, in
     its own form) followed by each of `images` written in the tab's form (`folder` joined with
     the image's file name) whose file name no file of `files` has (file names compared without
     regard to ASCII case), in `images`' order. An image without
     a file name is left out.
  3. **The end rule of F-Q10.** `pub struct ImportPosition { pub folder: PathBuf, pub current:
     Option<PathBuf>, pub mode: ViewMode }` (derives `Clone, Debug, PartialEq, Eq`), the active
     tab's folder, current image (the shell turns `Tab::current` into the path, `None` for an
     empty tab, T260) and view.
     `ImportTabs` gains `pub fn first_shown(&mut self, at: ImportPosition)`, which keeps `at`
     the first time it is called and ignores later calls, `pub fn first_kept(&self) -> bool`,
     whether a position was kept, and `pub fn end_target(&self, now:
     Option<&ImportPosition>) -> Option<PathBuf>`: the newest folder (the last of `folders()`)
     when a position was kept, `now` is equal to it (folder by `same_folder_lexically`,
     `current` exactly, `mode` equal) and the newest folder is not that same folder
     (`same_folder_lexically`); otherwise
     `None`.
  4. **Tabs.** `Tabs` gains, with doc comments:
     - `pub fn index_of_folder(&self, folder: &Path) -> Option<usize>`: the first tab whose folder
       is `folder` by `same_folder` (the test `open_new` uses; `open_new` may use it);
     - `pub fn open_behind(&mut self, folder: PathBuf, files: Vec<PathBuf>) -> usize`: when the
       folder is open in a tab, that tab's index, nothing changed; otherwise a new tab at the end
       opened on `folder` with `files` and the first file current (`Tab::open`, so the view is
       the contact sheet, GR13), and its index. The active tab does not change, also when it is
       empty (unlike `open_new`, which fills an empty active tab).
- **Tests to add** (in `import_tabs.rs` and `tabs.rs`):
  - `place`: two events over two folders give `first` true only at each folder's first group;
    groups in order; `folders()` in the order of first placement; an image without a parent is
    left out;
  - `waiting`: planned and not placed is waiting; placed is not; never planned is not; a folder
    spelt with other case or a trailing separator, and a file name in other case, still match
    (lexical: the folders need not exist);
  - `leave_out_waiting` keeps the order and drops only the waiting images;
  - `with_placed`: images added in the tab's form after the tab's files, those already there not
    twice, order kept;
  - `first_kept` false before and true after `first_shown`; `end_target`: `None` before `first_shown`; `None` with one folder; the newest folder when
    `now` equals the kept position; `None` when the tab, the current image or the view differs,
    or `now` is `None`; a second `first_shown` does not replace the first;
  - `index_of_folder` and `open_behind`: a new tab behind with the active index unchanged (also
    when the active tab is empty), its first file current and the contact sheet; a folder
    already open returns its index and leaves its files, filter and marks as they were.
- **Not in this task:** the shell's use of all this (T260); the import worker (T258).
- **Rules:** `docs/tasks/README.md` rules 1–12 (no texts); `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added when F4 was written (ADR 0026 §3.4); waits for T258 |
| 2026-11-21 | Blocked | Cold read: doable with the named files; `same_folder` reads the disk, so `ImportTabs` uses `same_folder_lexically` (made `pub`); the grouping key, a placed image never planned, `with_placed`'s comparison (file name, ASCII case) and the derives named; `open_behind` differs from `open_new` on an empty active tab (said). No code dependency on T258; the order is the chain's |
| 2026-11-21 | Open | T258 done; ready for its agent |
