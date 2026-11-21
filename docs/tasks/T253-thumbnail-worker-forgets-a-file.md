# T253. ADR 0026 F3e: the thumbnail worker forgets one file's thumbnails (pw-thumbcache) — Open

- **Why:** ADR 0026 §3.2, the owner's addition to F-Q7 (2026-11-21, the backlog sort after F3,
  from T252's hand check): a file another program is still writing (a slow copy to a USB stick)
  gets a thumbnail made from the part written so far; when the file has changed since, the shell
  asks for its thumbnail again (T254). The worker's in-memory cache keeps a made thumbnail by
  path, cell size and orientation (`CacheKey`), so the same request would be answered from the
  cache with the old, partly empty thumbnail. Today the cache is emptied only as a whole
  (`set_generation`, `cancel`), which would make every thumbnail of the tab anew. This task lets
  the shell drop one file's entries. First task of the chain T253 → T254.
- **Crate:** `pw-thumbcache`. **Files:**
  - `crates/pw-thumbcache/src/worker.rs` (point 1, and its tests)
  - `docs/test-inventory.md` (counts and the description of `pw-thumbcache`'s tests)
  - this task's own file (status and log)
- **What:**
  1. `ThumbWorker` gains `pub fn forget_path(&self, path: &Path)`, with a doc comment: every
     cache entry whose `CacheKey::path` equals `path` (compared as `PathBuf`s, exactly as the
     cache compares them; any cell size and orientation) is removed, and `cache_bytes` is lowered
     by their sizes. Queued and in-flight requests are not touched: a result of one that comes in
     later is cached as usual (T254 asks again for a cell only once its own request has been
     answered; a request of the same file at another cell size or orientation may still be in
     flight, and its result is then cached; accepted, named in T254).
     It does not block beyond the state lock and does not wake the workers. Nothing else changes.
- **Tests to add** (in `worker.rs`, with the test source and helpers the existing tests use):
  - a thumbnail made, then `forget_path` of its path, then the same request again: the source is
    called a second time (as in `changing_generation_clears_the_thumbnail_cache`);
  - `forget_path` drops the entries of that path at every cell size and orientation, and leaves
    another path's entry, which is still answered from the cache (the source not called again
    for it);
  - after `forget_path`, the freed bytes are counted: with a cache limit that holds two
    thumbnails, two made, one forgotten, and a third made, the remaining one is still answered
    from the cache (nothing was evicted for the third);
  - `forget_path` of a path not in the cache changes nothing.
- **Not in this task:** the shell's use (T254); the disk is not read.
- **Rules:** `docs/tasks/README.md` rules 1–12 (no texts); `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added in the backlog sort after F3 (the owner's choice: ADR 0026 §3.2, addition to F-Q7); waits for its cold read and the chain review |
| 2026-11-21 | Open | Cold read and chain review (T253 → T254): can be done as written (the fake thumbnails are 3 bytes, so a limit of 6 holds two); the sentence on in-flight requests softened (T254 checks only the cell's own request) |
