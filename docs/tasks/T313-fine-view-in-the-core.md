# T313. ADR 0027 W2: fine details in the view core — Done

- **Why:** ADR 0027 §3.1 and W2-Q2, W2-Q3, W2-Q6, W2-Q8 (answered by the owner 2026-11-24, ADR
  0027 §4.2). While **fine details** is on, a RAW view below 50 % is also made from the
  full-resolution render (PPG) reduced on the worker with the Lanczos filter, instead of only
  from the half-size render (W1's reduced view). The reduced view is shown first as today; the
  fine view replaces it in one swap when it is ready (V2). A LibRaw render cannot be cancelled, so
  the fine view is made only for the current image, after the dwell, on the background worker,
  after everything else the plan wants. This task makes the view core's part and reports the
  state to the shell; the key and the mark are T314's. The second task of the chain T312 →
  **T313** → T314. Waits for T312 (Done, and its time accepted).
- **What the reviewer checked (2026-11-24):**
  - `pw-viewcore` `state.rs`: `ViewKind` is `Half`, `Zoom`, `Ring`, `Scaled`, `Reduced`;
    `ViewKind::name`. `Want` (`Shown`, `Fit`, `Target`, `Ring`); `wanted(full)` lists the wants
    in order (the shown view; after the dwell the other view, +1 and +2 in the direction of
    travel, the other display mode, the ring, −1); during a gesture only the shown view at
    ≤ 0.5. `req(key, want)` makes the `JobView` (`Reduced` below 0.5, whole image, density the
    scale). `serves` (a `Reduced` view serves no requirement below half its density),
    `serving` (the least dense, then the smallest), `satisfied` (a `Reduced` requirement counts
    only as itself, `same_reduction`). `plan()` turns the wants into jobs in order; `pick
    (interactive)` gives the first job not running (the interactive worker only the current
    image's; a render outside the current image only if `textures_fit`), lending the session.
    `finish_render` sets the density per kind and on an error inserts the key into `failed`
    (which `frame` shows as `Status::Error`). `estimate_bytes` has a branch per kind;
    `gesture_stand_in` uses resident `Reduced` views. `go` sets `since` (the dwell's start).
    `releasing` holds paths being released for a metadata write; `pick` opens none of them.
    `focus_wanted`, `clipping_wanted` and `Frame::focus`/`FocusState` are the model for a flag
    the shell sets and a state the frame gives.
  - `core.rs`: `Task::Render` matches on the kind (`Scaled` → `render_region` then `reduce`);
    every view then goes through `to_display`. `ViewCore::set_focus_wanted` is the model for a
    setter (lock, compare, set, `cv.notify_all`, `wake_ui`).
  - `lib.rs` re-exports `FocusState` and the other frame types.
  - `pw-render` `lib.rs`: `mipmapped(kind)` and the magnification `matches!(kind,
    ViewKind::Half | ViewKind::Reduced)` (linear, W1-Q9); its test lists the kinds.
  - `Frame` is built as a literal outside `pw-viewcore` only in `pw-loupe` `loupe_text.rs`'s
    tests (`frame`, `tiff_frame`); nothing outside `pw-viewcore` matches on `ViewKind` except
    `pw-render`.
- **Crates:** `pw-viewcore`, `pw-render`, `pw-loupe` (test helpers only). **Files:**
  `crates/pw-viewcore/src/state.rs` (points 1–8 and its tests; its test helper's match on
  `ViewKind` gets a `Fine` arm), `crates/pw-viewcore/src/core.rs`
  (points 2 and 9), `crates/pw-viewcore/src/lib.rs` (`FineState` and `FinePause` exported),
  `crates/pw-viewcore/tests/engine.rs` (the engine test), `crates/pw-render/src/lib.rs` (point
  10), `crates/pw-loupe/src/loupe_text.rs` (the new `Frame` field in its two test helpers, as
  `FineState::Off`; nothing else), `docs/test-inventory.md` (rule 12), and this task's own file
  (status and log).
- **What:**
  1. **The kind:** `ViewKind::Fine`, named `"fine"`: "The whole image below 50 % while fine
     details is on: the full-resolution render (PPG) reduced on the worker to the view's density
     with the Lanczos filter (ADR 0027 §3.1, W2)". Every place that treats `Reduced` as a
     whole-image view reduced on the worker, without mipmaps, treats `Fine` the same
     (`serves`' rule of half the density, `estimate_bytes`, the density in `finish_render`,
     `gesture_stand_in`), unless a point below says otherwise.
  2. **The flag:** `State` gets `fine_details: bool` (off at start; not saved, the shell holds it
     for the session, W2-Q4), set by `ViewCore::set_fine_details(on: bool)` as
     `set_focus_wanted` is (a value equal to the one held changes nothing; not a user request,
     nothing recorded).
  3. **When it applies** (W2-Q3): for the current image only, when `fine_details` is on, the
     current key's source is `Source::Raw` (so never in JPEG mode, and never for a plain JPEG or
     TIFF file, which are keyed `Source::Jpeg`), its size is known and its scale is below 0.5
     (`< HALF_DENSITY - EPS`), and the image has not failed for fine details (point 7).
  4. **The want:** a new `Want::Fine`. `wanted(false)` adds `(current key, Want::Fine)` **last**,
     after −1, when point 3 holds, not during a gesture and only after the dwell (the same
     checks the list already makes before it adds the other wants). `wanted(true)` (what must not
     be evicted) adds it whenever point 3 holds. `req(key, Want::Fine)` gives `kind: Fine`,
     region the whole image, density the current scale (`quantise`), `cover` and `density` as for
     `Want::Shown`; `None` when point 3 does not hold. The shown view stays the reduced view
     (`Want::Shown` is unchanged), so it is made and shown first, as today.
  5. **The plan and the worker** (W2-Q2): `satisfied` counts a `Fine` requirement only as
     itself, resident or in flight, as for `Reduced` (`same_reduction`), and a `Fine` view,
     resident or in flight, satisfies **no other** requirement (so a gesture's half-size view or
     the shown reduced view is still made while a fine render runs). In `pick`, the
     **interactive** worker never takes a `Fine` render, nor a `Job::Open` that the plan emits
     only because of a `Want::Fine` (the current image's session was closed, for example by a
     release: the background worker opens it again for the fine view); the background worker
     takes them as any job, so, being last, only when no earlier job of the plan can be taken.
     A `Fine` render is not given while the current file is in `releasing` (a metadata write
     waits for no fine render that has not started). It is refused when `textures_fit` says no,
     also for the current image (W2-Q6; point 7); such a refusal counts in `stats.refused` but
     does not go into `self.refused` (the record's miss reason is about the shown view).
     `remove_inflight` and `accept`'s comparisons that look at the density for `Reduced` do so
     for `Fine` too.
  6. **What is drawn** (W2-Q8, V2): `serves` gives `false` for a `Fine` view while
     `fine_details` is off, so it is never drawn then and turning fine details off draws the
     resident reduced view at once, or makes it if it was evicted. While it is on, `serving`
     prefers **any** `Fine` view that serves (the least dense of them, then the smallest) before
     a view of another kind. So the fine view replaces the reduced view in one swap once it is
     accepted, and after a zoom to a new scale below 0.5 an earlier fine view that still serves
     (not below half its density) stays drawn until the new fine view replaces it (one swap,
     never fine → reduced → fine). While no fine view serves, the reduced view (or W1's stand-in
     during a gesture) is drawn as today. `gesture_stand_in` uses `Fine` views only while
     `fine_details` is on, and at an equal density prefers the `Fine` view. `protected_views`
     protects, while fine details is on, both the drawn fine view and the current image's exact
     reduced view (the shown requirement's), so neither is evicted for the other and the plan
     never renders the reduced view again under the budget. `accept` keeps a `Fine` result of the
     current generation while `fine_details` is on, also when its image is no longer current (it
     is not checked against `wanted`), and drops it while fine details is off; a kept fine view
     of an earlier image is not protected and is evicted as the budget needs, so stepping back to
     it shows it at once while it is still there.
  7. **When it goes wrong** (W2-Q6): a `Fine` render that fails (`finish_render`'s error
     branch), is refused by the budget (point 5) or is rejected by the presentation (`reject`,
     for example too large for the GPU) does **not** go into `failed` and shows no error: the
     inflight entry is removed as for any failed render, and the key goes into a new set of
     images that failed for fine details, kept as `failed` is (moved by `set_files` to the new
     positions, emptied by a new folder). `wanted` no longer asks for their fine view. A
     `Fine` render that **failed** in the backend also drops the image's session once it is
     returned (`drop_session`): a failed LibRaw call ends the session (`known-limits.md`,
     LibRaw), and a later job on it (Z, a new level, the focus map) would fail and show an
     error; the plan opens a new session when one is needed. The reduced view stays drawn. A
     budget refusal pauses fine details for that image until a new folder, as a failure does
     (W2-Q6; a refusal that was only for a moment is not retried, accepted).
  8. **What the frame says:** `Frame` gets `fine: FineState`; `FineState` and `FinePause` are
     both exported from `lib.rs` (T314 matches on them) and derive at least `Clone, Copy, Debug,
     PartialEq, Eq` (`Frame` derives `Clone, Debug, PartialEq`):
     `Off` (fine details is off); `Waiting` (it applies, point 3, and the drawn view is not a
     fine view of the current image yet); `Shown` (the frame's view is ready, `Status::Ready`,
     and it is a `Fine` view of the current key and generation, not an earlier image's view kept
     on screen); `Paused(FinePause)`, where `FinePause` is `Mode` (the current image is shown
     from its embedded JPEG, or is a JPEG or TIFF file), `Zoom` (its scale is 0.5 or more) or
     `Failed` (point 7). While the flag is off it is always `Off`. While it is on: with no image
     (`Status::Empty`) `Off`, since nothing is shown; for an image that cannot be shown
     (`Status::Error`) `Paused(Failed)`; for a RAW image whose size is not known yet `Waiting`.
     `Mode` is checked before `Zoom`, `Zoom` before `Failed`.
  9. **The render** (`core.rs`, `Task::Render`): `ViewKind::Fine` → `render_region` of the
     view's region (the whole image), then `reduce` to the view's size, as `Scaled`; then
     `to_display` as every view.
  10. **`pw-render`:** a `Fine` view is not mipmapped and is magnified linearly, as `Reduced`
      (W1-Q9); its test lists `Fine` with the other kinds.
- **Tests to add** (in `state.rs`'s tests with its test backend, unless named otherwise):
  - With fine details off, no plan ever holds a `Fine` render (fit, a free level below 0.5,
    RAW mode).
  - With it on, RAW mode, fit below 0.5: before the dwell the plan holds no `Fine` render; after
    the dwell, once the other wants are satisfied, the plan's last job is a `Fine` render of the
    current image (whole image, the fit scale); during a zoom gesture there is none.
  - `pick(true)` never returns a `Fine` render; `pick(false)` returns it.
  - JPEG mode, a plain JPEG file, and a scale of 0.5 and of 1.0: no `Fine` render;
    `Frame::fine` is `Paused(Mode)`, `Paused(Mode)`, `Paused(Zoom)`, `Paused(Zoom)`.
  - Before the fine view is accepted the frame draws the reduced view and `fine` is `Waiting`;
    after it is accepted the frame draws the fine view's id and `fine` is `Shown`.
  - Turning fine details off with both views resident: the frame draws the reduced view's id at
    once and `fine` is `Off`; the fine view is not drawn.
  - A failed `Fine` render: no `Status::Error`, the reduced view still drawn, `fine` is
    `Paused(Failed)`, and the plan asks for no new `Fine` render of that image.
  - A `Fine` render refused by the texture budget: `Paused(Failed)`, the reduced view drawn.
  - While the current file is being released (`begin_release`), no `Fine` render is picked.
  - Stepping on while a `Fine` render is in flight: its result is accepted and kept; stepping
    back draws it at once with `fine` `Shown`.
  - After a zoom gesture that ends at a new scale below 0.5, a `Fine` render at the new density is
    planned; the earlier fine view is drawn meanwhile when it serves (not below half its
    density).
  - `tests/engine.rs`: with the core running and fine details on, a RAW image at fit below 0.5
    gets a view of kind `Fine` whose size is the reduced view's, made from `render_region` of the
    whole image.
  - With both views resident and fine details on, the drawn fine view and the reduced view are
    both protected: filling the budget evicts neither, and no reduced render is planned again.
  - A rejected `Fine` view (`reject`): no `Status::Error`, `Paused(Failed)`.
  - After `set_files` moves the images, an image that failed for fine details at its old
    position is still `Paused(Failed)` at its new one.
  - A failed `Fine` render drops the session: a following `Zoom` request of the same image
    opens a new session and is shown, with no `Status::Error`.
  - With the current image's session closed and the reduced view resident, the plan's open for
    the fine view is not given to the interactive worker.
  - `pw-render`: `Fine` not mipmapped, magnified linearly.
- **Known and accepted (W2-Q2; chain review 2026-11-24):**
  - A fine render holds the current image's only session and cannot be cancelled; it keeps the
    session through the reduction and `to_display` too. So a zoom to 100 % made just after it
    starts (at a free level below 50 %, where the 100 % view is not prefetched), E or J, and the
    release for a metadata write (`RELEASE_TIMEOUT`, 5 s, `main.rs`) wait for it, up to the time
    T312 measured.
  - It holds the background worker as well: after a step during it, only the current image is
    served (the interactive worker) until it ends, so +1 and +2 of the new position are
    prefetched late. "Only when +1 and +2 are ready" (W2-Q2) is reached by the plan's order:
    a neighbour's job refused by the budget is passed over, and the fine render may then start.
  - With the compare open both cores may make a full-resolution render at the same time; the
    render's temporary buffers are not in the budget (as for `Scaled` today).
  The owner's `latency.ps1` after T314 shows whether these matter (W2-Q7).
- **Not in this task:** the key, the menu item and the mark (T314); a fine view for +1 (W2-Q7,
  after the owner's `latency.ps1`); fine details in JPEG mode (§2.2).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text is added). No
  new dependency. An extra code review follows (more than one crate).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W2-Q2, W2-Q3, W2-Q6, W2-Q8); waits for T312 |
| 2026-11-24 | Blocked | Cold read: the interactive worker also takes no open made only for the fine view; a `Fine` view satisfies no other requirement; a budget refusal not in `self.refused`; `remove_inflight`, `accept` and `reject` named (a rejected fine view does not go into `failed`); `serving` prefers any serving fine view (no fine → reduced → fine after a zoom); `protected_views` keeps the reduced view too; `gesture_stand_in` and `Shown` (only the current image's ready view) made exact; the failed set kept as `failed` is across `set_files`; the test helper's `ViewKind` match named; the wait of a zoom to 100 % behind a fine render written as accepted (W2-Q2, measured by W2-Q7); four tests added |
| 2026-11-24 | Blocked | Chain review (whole chain T312-T314; noted here as T313 holds most of it): `FinePause` exported and both enums' derives named, `core.rs` also for point 2; a failed fine render drops the session (else a later Z or focus map fails on the dead session and shows an error, against W2-Q6), with a test; the waits behind a fine render (the session through the reduction, the background worker, the release's 5 s), the approximated "+1 and +2 ready", the compare's two renders and the budget refusal for the session written as known and accepted |
| 2026-11-24 | Open | T312 Done; the owner accepted its time 2026-11-24 (ADR 0027 §4.2): T313 goes ahead as written. Set Open |
| 2026-11-24 | Ready for review | Points 1–10 done; 16 state tests and 1 engine test added, `pw-render`'s test lists `Fine`. Choices to check: an image that cannot be shown gives `Paused(Failed)` before the mode is checked (point 8 read as written); the render's lending is shared by the `Fine` and the other render arm in a small `lend_render` helper; turning fine details off also clears a fine view kept as the last frame, so it is never drawn while preparing the reduced view again |
| 2026-11-24 | Done | Reviewed: points 1–10 as written, the 16 state tests, the engine test and `pw-render`'s check there; `test-inventory.md` counts right (state 181 → 197, engine 16 → 17, 2624 passed); check.ps1 all OK, no clippy warning. The three choices accepted. Independent review (fresh review agent, rule on large tasks): no bugs; findings, all accepted by the owner 2026-11-24 as known: (1) `protected_views` may protect a non-fine view (the half-size view) for the fine want while no reduced view is resident, which makes a budget refusal of the fine render likelier under a tight budget; (2) after a zoom whose new fine render fails or is refused, an earlier fine view that still serves stays drawn while `fine` is `Paused(Failed)` (noted in T314); (3) `Status::Error` gives `Paused(Failed)` also in JPEG mode (choice 1); (4) a budget refusal pauses the image until a new folder (accepted in W2-Q6); (5) the interactive `pick` computes `plan_of(false)` a second time; (6) the engine test does not prove the whole-image render came from the fine job. T314 set Open |
