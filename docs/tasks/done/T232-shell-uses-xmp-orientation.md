# T232. ADR 0024 O1c: the shell gives the read orientations to the view and the thumbnails (pw-loupe) — Done

- **Why:** ADR 0024 §2.4 (O-Q6): the XMP orientation read in the folder's batch call (T230) is
  given to the view core and the thumbnails (T231). Until it is read, the embedded orientation is
  used, and the image turns when the value arrives. After this task an image whose sidecar (or
  whose own XMP) says it is turned is shown turned, in the preview and in the contact sheet. This
  ends ADR 0024 O1; O2 (writing) and O3 (keys, menu, the owner's hand check) follow.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/metadata.rs` (points 1–2),
  `crates/pw-loupe/src/thumbs.rs` (point 3), `crates/pw-loupe/src/main.rs` (points 4–5),
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **Kept per path** (`metadata.rs`, `MetadataCache`): a new field
     `orientations: HashMap<PathBuf, u16>` (documented: ADR 0024 §2.4, the XMP orientations read
     with the ratings), and a list of changes not yet taken,
     `orientation_changes: Vec<(PathBuf, Option<u16>)>`. On `WorkerEvent::RatingsRead` with
     `Ok` for the current load (the arm that today calls `self.ratings.read`), for every path of
     that load that was `Loading` (the same `paths` list): the read value is
     `orientations.get(path)` from the event (absent = `None`). If it differs from what is kept
     for the path (absent = `None`), the kept value is updated (inserted or removed) and
     `(path, value)` is pushed onto `orientation_changes`. A read that fails changes nothing.
     `prune` drops the orientations of images no longer open, as it drops their ratings, **and
     pushes `(path, None)` for each one it drops**: the cores keep values per path (T231), so
     without it a folder opened again after its sidecar lost the value would read `None`, find
     `None` kept, push nothing, and the cores would still show the old turn.
  2. **Read by the rest of the shell** (`metadata.rs`): `pub fn orientation(&self, path: &Path)
     -> Option<u16>` and `pub fn take_orientation_changes(&mut self) -> Vec<(PathBuf,
     Option<u16>)>` (empties the list), each with a doc comment.
  3. **A thumbnail made again** (`thumbs.rs`, `ThumbGrid`): a new
     `pub fn forget(&mut self, index: usize) -> bool` that sets the entry to `Missing` from
     **any** state, `Requested` included (unlike the private `Entry::forget`, which leaves
     `Requested` alone): a request in flight was sent with the old orientation, and its late
     event then lands on a `Missing` entry as `Ready(None)` (`apply`), which `wants` asks for
     again. Returns whether the entry was `Ready` (a texture to release). An index outside the
     grid changes nothing and returns `false`.
  4. **The values are sent** (`main.rs`, in `draw` right after `self.metadata.poll(…)`): for each
     `(path, value)` of `take_orientation_changes()`:
     - `self.core.set_orientation(path.clone(), value)` **and**
       `self.second_core.set_orientation(path.clone(), value)` (the compare's core). T231's
       cores keep the value per path across `open_folder` and `set_files`, so mode switches, tab
       switches and the compare need nothing sent again.
     - If the path is in the active tab's files (compared as resolved paths: `tab.files` may hold
       relative paths, the metadata cache keys absolute ones, `resolve_paths`), its index in
       `tab.files` is the thumbnail index: `self.thumb_grid.forget(index)`, and the old texture
       stays in `thumb_textures` until the new thumbnail replaces it in `poll_thumbnails` (no
       empty cell in between).
     - A path in another tab is only sent to the cores; its thumbnail is asked for with the value
       when that tab is shown (point 5).
  5. **The thumbnails are asked for with it** (`main.rs`, both `ThumbnailRequest { … }`, which
     T231 gave `orientation: None`: the contact sheet's and the filmstrip's):
     `orientation: self.metadata.orientation(<the resolved path>)`.
     The values come from T230's read (only `1..=8`), so no out-of-range value reaches the
     worker; that `Some(<the file's own value>)` and `None` are cached as two thumbnails is an
     accepted cost (T231's review).
  6. **A late thumbnail with an old value is dropped** (`main.rs`, `poll_thumbnails`): an event
     whose `orientation` (T231) differs from `self.metadata.orientation(<its resolved path>)` is
     not given to `thumb_grid.apply` and makes no texture; the request made after `forget`
     (point 4) brings the right one.
  7. **Info:** ADR 0024 §2.4 names Info too, but Info shows no orientation today
     (`info_panel.rs` has none), so nothing changes there; the report says so.
- **Tests to add:**
  - `metadata.rs` (built inline as the existing tests do, with `FakeSubmit`, `begin_load`,
    `cache.load_request` and `cache.apply(…)`, around lines 1682–1760; a small test helper for
    "a load and its read" is fine): a load whose read gives orientation 6 for one image pushes one change
    and `orientation` gives `Some(6)`; the same read again pushes nothing; a later read without
    the value pushes `(path, None)`; a failed read pushes nothing; an image without a rating but
    with an orientation gets it; `prune` drops it and pushes `(path, None)`, and a path pruned
    without a value pushes nothing; the same folder loaded again after a prune, with the value
    still there, pushes it again; `take_orientation_changes` empties the list.
  - `thumbs.rs`: `forget` on a ready cell returns `true` and the cell is asked for again for the
    same cell size; on a requested cell it returns `false`, and the late event of that request
    leaves the cell asked for again; on an index outside the grid it returns `false`.
  - What `main.rs` does in points 4–6 has no unit test (drawing code); the report says how it
    was checked (for example by running RawStride on a folder with a sidecar holding
    `tiff:Orientation`, if the agent can).
- **Not in this task:** keys, menu items and writing (ADR 0024 O2, O3; what O3 needs from here is
  in its row in `plans/adr-0018-0025-steps.md`); holding thumbnail requests until the read has
  come, so a turned image's thumbnail is not decoded twice when a folder opens (a known cost,
  accepted for now); the zoom rule (O3); the
  owner's hand check (after O3, ADR 0024 §4; the reviewer may look at the result with a sidecar
  test folder); the change log line (the reviewer adds it at Done).
- **Rules:** `docs/tasks/README.md` rules 1–13 (no new user-visible text is expected). No new
  dependencies. No `.unwrap()` or `.expect()` outside tests.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Blocked | Added (ADR 0024 O1, chain T230 → T231 → T232); waits for T231 |
| 2026-11-20 | Blocked | Cold read: doable in the named files once T231's cores keep orientations per path across `open_folder`/`set_files` (T231 point 3 says so). Added: the compare's `second_core` also told (point 4); resolved paths for the active-tab check and the requests; `ThumbGrid::forget` returns `bool`, resets `Requested` too, and the old texture stays until the new one arrives; the metadata tests built inline as today. Known: a moved file has no orientation until the next read |
| 2026-11-20 | Blocked | Chain review (notes in T230's log): `prune` pushes `(path, None)` (point 1), late thumbnails made with an old value dropped (point 6, with T231's event field), the double decode at folder open named as an accepted cost; O3's needs recorded in the plan's O3 row |
| 2026-11-20 | Open | T231 done (committed); this task can start. Point 5 notes, from T231's review, that the values are only `1..=8` and the double cache of the file's own value and `None` is accepted |
| 2026-11-20 | Ready for review | Points 1–7 done. `metadata.rs`: `orientations` and `orientation_changes` in `MetadataCache`, kept from the `Ok` read for the loading paths (`read_orientations`), `prune` drops and pushes `(path, None)`, `orientation` and `take_orientation_changes`. `thumbs.rs`: `ThumbGrid::forget`. `main.rs`: `follow_orientations` (after `metadata.poll` in `draw`) tells both cores and forgets the active tab's thumbnail by resolved path; both requests carry `read_orientation(…)`; `poll_thumbnails` drops an event whose orientation is not the read one. Info: no orientation shown today, nothing changed (point 7). 7 tests (4 metadata, 3 thumbs); test-inventory 1995 passed, 72 ignored. `main.rs` points 4–6 not run by hand (no GUI check by the agent). check.ps1 all OK, no clippy warnings |
| 2026-11-20 | Done | Reviewed: the diff holds only the named files and matches points 1–7. Checked: `read_orientations` only on an `Ok` read and only for the loading paths, a change pushed only when the value differs; `prune` sorted and pushing `(path, None)`; `metadata.poll` is called only in `draw`, and `follow_orientations` right after it, so a changed value and the thumbnail's `forget` come in the same frame before `poll_thumbnails`, and a dropped late thumbnail never leaves a cell waiting; the cores get resolved paths (`open_loupe_core`, `loupe_follows_visible`, `hold_in_second_core`), matching the metadata cache's keys; the filmstrip's thumbnail index is the tab's file index, as `follow_orientations` uses. No extra code review needed (one crate, five files). The seven tests ran and passed; `.\tools\check.ps1` all OK (1995 passed, 72 ignored), no clippy warnings. Nothing left over. `CHANGELOG.md`: one line under Added. Points 4–6 not run by hand by the agent: the owner's hand check follows (owner, 2026-11-20: after this commit, not waiting for O3); this corrects T230's and T231's Done rows only in timing. Committed on the owner's yes; ADR 0024 O1 done in code |
| 2026-11-20 | Hand check OK | The owner's hand check of ADR 0024 O1 (T230–T232) passed: on a folder whose sidecars hold `tiff:Orientation`, the images are shown turned in the preview, the contact sheet and the compare. Plan O1 row |
