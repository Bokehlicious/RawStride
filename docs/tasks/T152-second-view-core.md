# T152. ADR 0017 U6, third part: the second view core, and the image area drawn by one function — Blocked

- **Blocked: waits for T151** (and T150; the owner's answers to U6-Q1–U6-Q12 are in
  `docs/tasks/T150-locked-view-in-the-view-core.md` and ADR 0017 §6.1). Chain: T150 → T151 →
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
