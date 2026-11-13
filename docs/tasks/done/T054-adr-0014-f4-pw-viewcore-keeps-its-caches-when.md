# T54. ADR 0014 F4: pw-viewcore keeps its caches when the file list changes — Done

- **Second review 2026-11-10: accepted.** The review point below is done. Commit together with T53.

- **Review 2026-11-10: changes requested (one point).** `set_files` is accepted as written: the
  remap by path, the release of removed paths with exact byte counts, `lent`/`detached` so a stale
  render never touches a session under a key of the new order, `last_draw`, and the eight tests.
  Still to do, in `crates/pw-viewcore/src/state.rs` only:
  1. **The generation in `running` also changes `open_folder`, and that needs a test.** Before, a job
     of the old folder still running blocked the job with the same key in the new folder (`pick`
     skipped `Open(key)` while any job with that key ran), so the new folder's first image waited for
     the old folder's unpack. Now it starts at once. That is better and is accepted, but it is a
     behaviour change outside `set_files` ("`open_folder` keeps its behaviour"), and every behaviour
     change gets a test. Add one: an `Open` picked before `open_folder` is still running; after
     `open_folder`, `pick` hands out the `Open` of the same position in the new folder; when the old
     one finishes, it is discarded and the new one is kept. Mention the change in your log row.
  Then run `.\tools\check.ps1`.

- **Can start now.** Independent of T51–T53. **For a Claude agent** (the cache bookkeeping of
  `pw-viewcore`).
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/state.rs`,
  `crates/pw-viewcore/src/core.rs`. Not `pw-loupe`.
- **Why:** ADR 0014 §4.4, Q3 (way C). The loupe gets the visible list. When it changes (an image
  drops out after a rating, or the filter changes), images already opened and rendered must be kept
  and prefetch must follow the new order. Today every cache is keyed by position
  (`Key = (usize, Source)`) and `open_folder` empties them all.
- **What:**
  1. `State::set_files(&mut self, files: Vec<PathBuf>, current: usize)` and
     `ViewCore::set_files(&self, files, current)` (through `act`, like `open_folder`).
  2. Same list (equal paths in the same order) → only `go(current)`; nothing else changes.
  3. Otherwise:
     - **Finished state is moved to the new positions by path:** `sizes`, `failed`, owned sessions
       (`Some`) with `session_bytes_of` and `session_lru`, `resident` (`pos`), and `ever_resident`.
       Resident views keep their **ids**, so the textures in `pw-loupe` stay valid and need no upload.
     - **State of paths no longer in the list is released** the way eviction does: resident ids go to
       `take_dropped`, sessions are dropped and their bytes subtracted, `texture_bytes` and
       `session_bytes` stay exact.
     - **Work in flight** (`running`, `inflight`, `outbox`, lent sessions) belongs to the old order.
       You may start a new generation for it, as `open_folder` does, as long as the moved resident
       views still serve (they are checked against the generation in `serves`). The rule: **a result
       of the old order must never be stored, shown, or remove anything under a key of the new
       order.** Note that today a stale `finish_render` removes `sessions[key]`; after a remap that key
       can belong to another image. If lent sessions need to remember the generation they were lent in,
       change that inside `state.rs`.
     - `cur` becomes `current` (clamped); zoom, centre, direction and display mode are kept. If the
       current image is the same path as before, the view on screen stays until a new one is ready
       (no flash, as `the_view_on_screen_is_kept_while_the_next_is_prepared`).
  4. `open_folder` keeps its behaviour.
  5. If a structure cannot be remapped safely in this way, **stop and report** instead of choosing
     another design.
- **Rules:** as the other tasks; no `unwrap`/`expect`/`panic!` or unchecked indexing in non-test code;
  rules 6, 7, 10–12. Existing `pw-viewcore` tests are not changed.
- **Tests to add** (in `state.rs`, with the existing helpers):
  - a resident view of an image that stays moves to its new position with the same id, and `frame`
    draws it without a new render;
  - removing an image releases its resident id (`take_dropped`) and its session bytes;
  - after `set_files`, `wanted` and `plan` use the neighbours of the **new** list (+1, +2 are the next
    paths in the new order);
  - a render lent before `set_files` and finished after it is discarded, leaves the session of the
    image now at that position alone, and leaves `session_bytes` exact;
  - an open finished after `set_files` is discarded;
  - the same list → nothing dropped, generation unchanged;
  - the current image unchanged → its view keeps being drawn; zoom kept;
  - an empty list → no current image, everything released.
- **Done when:** `.\tools\check.ps1` is all OK.
