# T256. ADR 0026 F4a: a generation change that keeps the thumbnail cache (pw-thumbcache) — Done

- **Why:** ADR 0026 §3.5 (the owner, 2026-11-21, when F4 was written): every change of the
  active tab's files starts a new thumbnail generation, and `ThumbWorker::set_generation` empties
  the in-memory cache with it, so every visible thumbnail is made anew. During an import (F4) the
  active tab changes several times a second. The cache is kept by path, cell size and orientation
  (`CacheKey`), not by place in the tab, so its entries stay true when the files only move or
  more files come. This task lets the shell change the generation without emptying the cache.
  First task of the chain T256 → T257 (F4a), before the import part T258 → T259 → T260.
- **Crate:** `pw-thumbcache`. **Files:**
  - `crates/pw-thumbcache/src/worker.rs` (point 1, and its tests)
  - `docs/test-inventory.md` (counts and the description of `pw-thumbcache`'s tests)
  - this task's own file (status and log)
- **What:**
  1. `ThumbWorker` gains `pub fn set_generation_keeping_cache(&self, generation: u64)`, with a
     doc comment: as `set_generation` (the generation set, `visible` cleared, the queue cleared,
     the workers woken), but the cache and `cache_bytes` are kept. A request of an older
     generation still in flight when it comes back is **not sent** (its index is stale), but its
     result **is cached** when no `set_generation` or `cancel` came since the request was made
     (only `set_generation_keeping_cache`): the cache is kept by path, so the image is still
     true, and without this a slow RAW preview would never be finished while the shell changes
     generation several times a second during an import (the chain review). For example a
     counter of cache epochs in the state, raised by `set_generation` and `cancel` only, and the
     epoch taken with each request when it is queued; a result is cached when its epoch is the
     current one, and sent only when its generation is the current one (and it is not far from
     the visible range, as today). After `set_generation` or `cancel` an old result is neither
     cached nor sent, as today. `set_generation` and `cancel` otherwise unchanged.
- **Tests to add** (in `worker.rs`, with the test source and helpers the existing tests use,
  for example as in `changing_generation_clears_the_thumbnail_cache`):
  - a thumbnail made in generation 0 (a new worker starts at 0, and `request` refuses another
    generation), then `set_generation_keeping_cache(1)`, then the same path, cell and
    orientation asked for in generation 1 (at another index): answered from the cache, the
    source not called again, the event carrying generation 1 and the new index;
  - after `set_generation_keeping_cache`, a request still queued from the old generation is not
    made (the source not called for it);
  - a request of the old generation **being made** when `set_generation_keeping_cache` is called
    (with `gated_worker`, as in `in_flight_results_from_an_old_generation_are_discarded`): after
    the gate opens its event is not sent, but a request of the new generation for the same path,
    cell and orientation is answered from the cache (the source called once in all);
  - the same with `set_generation` instead: the old result is neither sent nor cached (the new
    request calls the source again); and the existing
    `in_flight_results_from_an_old_generation_are_discarded` still passes;
  - the same sequence with `set_generation` still makes the thumbnail anew: the existing
    `changing_generation_clears_the_thumbnail_cache` shows it; name it in the report.
- **Not in this task:** the shell's use (T257); `set_generation`, `cancel` and `forget_path` are
  unchanged.
- **Rules:** `docs/tasks/README.md` rules 1–12 (no texts); `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added when F4 was written (ADR 0026 §3.5, the owner's choice of step F4a); waits for its cold read and the chain review |
| 2026-11-21 | Blocked | Cold read: doable with `worker.rs` alone; test generations start at 0, the in-flight case tested with `gated_worker`, `changing_generation_clears_the_thumbnail_cache` named. Chain review: an old result coming back after `set_generation_keeping_cache` is cached (not sent), else a slow RAW preview never finishes while a tab fills during an import; the cache epoch proposed |
| 2026-11-21 | Open | Cold read and chain review done (rows above); ready for its agent |
| 2026-11-21 | Ready for review | `set_generation_keeping_cache` added; `State::cache_epoch` raised by `set_generation` and `cancel` only, taken with each queued request; a result is cached when its epoch is current and sent when its generation is current and it is not far from the visible range. 5 tests (the four listed plus the old result after `cancel`); `changing_generation_clears_the_thumbnail_cache` and `in_flight_results_from_an_old_generation_are_discarded` unchanged and passing. Note: as point 1 words it, a current-generation result far from the visible range is now cached (not sent); before it was dropped. check.ps1 all OK (2209 passed, 76 ignored), no clippy warnings |
| 2026-11-21 | Done | Reviewed: as point 1 and the chain review say; the five tests and the two named old ones seen passing in `test-output.txt` (newer than the change), no clippy warnings. The far-from-visible result now cached accepted (as point 1 words it). Leftover: the in-flight check compares generation and index, not path, so a new generation's request for a file still being made in the old one is made a second time in parallel (double work for a slow RAW preview during an import): written into T257's "Not in this task" (the owner, 2026-11-21). Nothing the user sees on its own; checked by hand through T260 |
