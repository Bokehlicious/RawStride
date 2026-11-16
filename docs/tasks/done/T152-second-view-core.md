# T152. ADR 0017 U6, third part: the second view core, and the image area drawn by one function — Done

- **Can start now.** T151 (`compare.rs`: `Layout`, `sides`, `open_indexes`, `Compare`, `follow`;
  the settings `compare_above_below` and `compare_locked`; `compare_side_text`) and T150 are Done.
  The owner's answers to U6-Q1–U6-Q12 are in
  `docs/tasks/done/T150-locked-view-in-the-view-core.md` and ADR 0017 §6.1. Chain: T150 → T151 →
  T152 → T153 → T154.
- **Why:** ADR 0017 §2.2 and step U6. The held image is shown by a second view core (U6-Q2), and
  the preview must draw two images. Today the image area is drawn inline in the shell's frame
  (`main.rs`, from `ui.interact(…, "image")` to the status line, with one `textures`, one
  `clip_overlay` and one view core), and the image check of the writers knows one core. This
  task makes both ready **without changing anything the user sees**, so T153 only adds the
  compare. It was split from T153 by the cold read, as the whole was too big for one task.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `docs/test-inventory.md`.
- **What:**
  1. **The second view core** (U6-Q2, U6-Q3), started in `run` beside the first and **before**
     the metadata and import workers (their image check needs it): its own `NativeBackend`, made
     as the first one is (the same monitor profile) but with the TIFF limit of its own session
     budget; `Config` as the first's with `core_budget / 4` and `texture_budget / 4`. Its wake as
     the first's. It holds an empty list (nothing opened) in this task; it is kept in `Loupe`
     beside the first core, with a second `pw_render::Textures` and a second
     `OverlayCache<egui::TextureHandle>` of its own, synced each frame as the first's are (with an
     empty list nothing is uploaded). A core is never given another `Textures`: emptying it is
     done with `set_files(vec![], 0)` and its textures go back through `take_dropped`, as today.
  2. **The image check** (`image_check`, `release_before_write`): before a write, the file is
     released in **both** cores (the second first, each with `RELEASE_TIMEOUT`), then the
     histogram reader's wait and the stamp as today. The two releases are given to
     `release_before_write` as one `release_core` closure; nothing else in the check changes.
  3. **The image area as one function:** the part of the frame that draws the image (the
     interaction, the textures' sync, the frame asked of the core, the image, the clipping marks,
     the rating badge, the error and indicator texts) moved into a function or method that takes
     the view core, its `Textures`, its `OverlayCache`, the rectangle to draw in and an id for its
     `interact`, and returns that core's `Frame` (and what the caller needs of the response).
     The caller keeps what belongs to the current image only: `last_frame_status`, `last_zoom`,
     `last_clipping`, `last_statistics`, the inspector's statistics, the display mode and the
     status line. In this task it is called once, for the first core, with today's rectangle and
     id; **the drawing, the keys and the mouse are exactly as before**.
- **Tests to add:** `release_before_write` with a `release_core` that releases in two cores (two
  recording closures as the T144 test uses): both are called, the second core's first, before the
  histogram reader's wait. The rest is a move of code; `check.ps1`'s tests and the latency run
  show it changed nothing.
- **Not in this task:** opening the compare, the second core given a file, keys, menu items and
  texts (T153); the lock (T154).
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency; no new text the user sees.
  Run `D:\PhotoWorkflowData\pm-study\tools\latency.ps1` on a release build before and after and
  report V1; it must not be worse (the second core is idle). It adds a second core to the program
  and touches the safe write's image check, so it gets an independent review of the diff before
  the owner is asked to approve (README, "An extra code review").
- **Done when:** `.\tools\check.ps1` is all OK, V1 is not worse, and `git diff --stat` shows only
  the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Blocked | Added after the cold read of the chain of U6, which split the first T152 in two (this task and T153); waits for T151 |
| 2026-11-16 | Open | T151 Done |
| 2026-11-16 | Ready for review | Done by the agent. `main.rs`: the second view core (`second_core`, `second_textures`, `second_clip_overlay` in `Loupe`), started in `run` before the workers with its own `NativeBackend` (the same monitor profile, the TIFF limit of its own session budget) and a quarter of both budgets, its wake as the first's; its textures are synced in the preview's frame beside the first's and its clipping marks' cache is released each frame (nothing of it is drawn yet). `image_check` takes both cores and gives `release_before_write` one closure made by the new `release_in_both` (the second core's release first, each with `RELEASE_TIMEOUT`). The image area is drawn by `draw_image_area` (core, `Textures`, `OverlayCache`, rectangle, id, and `ImageAreaShown` with what it reads of the shell: anchor at pointer, the overlay choice, the metadata cache for the rating badge, the indicator delay, the message and the file count for the empty area's text); it returns the `Frame`. **Choices to confirm:** the caller needs nothing of the response today, so only the `Frame` is returned; the background (`full`) is filled by the caller before the call instead of between the frame and the image (nothing is painted in between, so the picture is the same); the empty area's text (the message or the status line) is drawn by the function, so it takes `message` and `total`; `Loupe::anchor` now calls the new free function `zoom_anchor`, which the image area uses too. One test added (`a_write_releases_the_file_in_both_view_cores_before_it_waits_for_the_read`); `docs/test-inventory.md` updated. `check.ps1` all OK (1604 passed, 63 ignored, 0 failed; no clippy warnings). `latency.ps1` (A1 files TN006061–67 in `D:\PhotoWorkflowData\RAW`, started on TN006061, release builds, RIGHT × 6, 1.5 s apart, two runs each, `results/latency-t152.csv`): V1 next image at fit, median 5.3 and 5.6 ms before, 5.6 and 5.5 ms after (after: 5.2–7.2 ms), one change each; not worse. The program has 40 threads instead of 38 (the second core's two workers). Not checked by eye beyond the latency runs; the independent review of the diff is the reviewer's |
| 2026-11-16 | Done | Reviewed: the diff does what the task asks (the second core started before the workers with a quarter of both budgets, its own backend with the same monitor profile and its own TIFF limit, its own `Textures` and `OverlayCache`; the file released in both cores, the second first, before the histogram wait; `draw_image_area` the old code moved line for line, the background still painted before the image); only the named files; V1 not worse (5.3/5.6 ms before, 5.6/5.5 ms after). Accepted: only the `Frame` returned; the background filled before the call; `zoom_anchor` as a free function. Independent review (fresh agent): no bugs; the 140-character doc comment of `image_check` wrapped by the reviewer; the new test assumes both releases finish within 300 ms (kept, practically always true); the second core's textures are synced only in the preview's frame, harmless while it is empty: moved to T153 point 6. `check.ps1` after the reviewer's fix all OK (1604 passed, 0 failed, 63 ignored); no clippy warning. Owner approved 2026-11-16 |
