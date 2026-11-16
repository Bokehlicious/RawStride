# T150. ADR 0017 U6, first part: a view set from outside in the view core — Open

- **Can start now.** The owner approved U6-Q1–U6-Q12 (at the end of this task) on 2026-11-16;
  they are in ADR 0017 §6.1. They
  decide the whole step U6, which is the chain **T150 → T151 → T152 → T153 → T154**: this task
  lets the view core take a zoom level and position from outside and tells the shell its
  position; T151 the compare model, its settings and texts (plain Rust); T152 the second view
  core and the image area drawn by one function (nothing new on the screen); T153 the two images
  in the preview with their keys and menu items; T154 the locked zoom and position. The owner's
  hand check comes after T154.
- **Why:** ADR 0017 §2.2 and step U6: two images compared in the preview, with zoom and position
  locked or free. The held image is shown by a second view core (U6-Q2). When the two are locked,
  the held image must show the same zoom level and the same point of the image as the current one.
  Today the shell can only send the core relative changes (`toggle_zoom`, `zoom_by`, `pan`), and
  `Frame` does not say where the view is centred, so the second core cannot be made to follow the
  first.
- **Crates:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/state.rs` (the setter and the
  frame's new field, with unit tests), `crates/pw-viewcore/src/core.rs` (the public method),
  `crates/pw-viewcore/tests/engine.rs` (one test through `ViewCore`),
  `crates/pw-loupe/src/loupe_text.rs` (only: the field added to the two `Frame { … }` literals
  of its tests, nothing else); `docs/test-inventory.md`.
- **What:**
  1. **`Frame::centre: [f64; 2]`**: the position of the view as the core holds it (`State::centre`,
     the relative point of the upright image at the view's centre, V6), given in every frame.
     Nothing else in `Frame` changes.
  2. **`ViewCore::set_view(&self, zoom: Zoom, centre: [f64; 2])`** and the `State` function it
     calls: the view model gets this zoom level and this position, as if the user had zoomed and
     dragged there.
     - **The zoom:** `Zoom::Fit` is taken as it is. `Zoom::Scale(s)` with `s` not finite or not
       above 0 is ignored (nothing changes). Otherwise, when the current image's size is known,
       `s` is clamped as `set_zoom` clamps it (between the image's smallest scale and
       `MAX_SCALE`); **when the size is not known yet** (a file just given, not opened), it is
       clamped to `MAX_SCALE` only and kept, so the held image is not stuck at the fallback
       smallest scale of `min_scale(None)`. The view model is not clamped again when the size
       becomes known; the drawn region is kept inside the image by `geometry::visible`, as for
       every view.
     - **The centre:** a centre with a coordinate that is not finite is ignored (nothing
       changes); otherwise each coordinate is clamped to 0–1 and stored. It is **not** passed
       through `geometry::pan_centre` (which would reset it to 0.5 where the image fits the
       view); `geometry::visible` keeps the drawn region inside the image, as it does today. At
       `Zoom::Fit` the centre is stored but does not move the fitted image, as today.
     - It counts as **one step of a zoom gesture** (`gesture_at` set as `set_zoom` sets it with
       `gesture` true): the views that depend on the position are asked for only when the calls
       pause (`Config::gesture_pause`), so a drag on the other image does not make this core
       render every position on the way.
     - **Equal values change nothing:** when the zoom and centre after the rules above equal what
       the core already holds, nothing is set and no new gesture step starts, so the shell may
       call it every frame and the exact view is still rendered after the pause.
     - It is **not a user request:** it does not go through `act` (which records); like
       `set_clipping_wanted` it takes the state, calls `changed()` when something changed, and
       wakes the workers and the screen. Nothing is recorded (`Record`).
  3. Zoom and position stay when stepping (V6), as today: `set_view` sets the view model, not one
     image's.
- **Tests to add:** in `state.rs`: `Frame::centre` follows a pan and a zoom around an anchor; after
  `set_view(Scale(1.0), [0.3, 0.6])` the zoom is 100 % and the centre `[0.3, 0.6]`; with the size
  known, a scale above `MAX_SCALE` is clamped to it and one below the smallest scale to that; with
  the size not known, `Scale(0.2)` is kept as 0.2; a NaN or negative scale and a NaN centre change
  nothing; a centre of `[-1.0, 2.0]` is stored as `[0.0, 1.0]`; a second call with the same values
  starts no new gesture step and leaves the generation as it was; the views of the new position are asked for only after the
  gesture pause (as the existing gesture tests check it). In `tests/engine.rs`: through `ViewCore`
  with the test backend, `set_view` then `frame` gives the zoom and centre set, and `take_records`
  holds no record of it.
- **Not in this task:** the second core, anything else in `pw-loupe` (T151–T154), any change to the
  existing zoom and pan functions or to what they record.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency; no text the user sees.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Questions for the whole step U6 (the reviewer's proposals; the owner answers before T150 opens)

The model in short: one side shows the **held** image, the other the **current** image. The
current image is the one the preview has today: steps, ratings, labels, `M`, the right column and
the filmstrip act on it as now. The held image stays until the user holds another one or swaps.

| # | Question | Proposal |
|---|---|---|
| U6-Q1 | How is the work split? | Four tasks: T150 the view core takes a zoom and position from outside and gives its position in the frame; T151 the compare model, settings and texts in `pw-loupe` (plain Rust, tested); T152 the second image drawn in the preview, with the keys and menu items (zoom free); T153 the locked zoom and position. The owner's hand check after T153. **Changed by the cold read (2026-11-16):** five tasks, T152 split in two (the second core and the image area as one function; then the compare drawn), the lock in T154 |
| U6-Q2 | How is the second image shown? | By a **second view core** of its own, started with the program and given a list of one file (the held image) while comparing, an empty list otherwise. The first core and its stepping stay as they are, so stepping is as fast as today. A metadata write releases the file in both cores (the image check) |
| U6-Q3 | Its memory | The second core gets a quarter of the first one's budgets (by default 1 GiB for sessions and 256 MiB for textures, from `--core-mb` and `--texture-mb`), since it holds one image; with an empty list it holds nothing |
| U6-Q4 | Which images are compared when it opens? | **C** in the preview: the current image becomes the held one, and the current image steps to the next shown image (the previous one at the end of the list). With "only the marked" on (`S`) and two marked images, those two are compared. With only one shown image, C does nothing and the menu item is greyed |
| U6-Q5 | Which side is which? | The held image left (side by side) or on top (above/below); the current image right or below, with a frame in the accent colour so it is clear which side the keys act on |
| U6-Q6 | Holding and swapping | **K**: the current image becomes the held one (the winner of a pair stays; then step on). **Shift+K**: the held and the current image change places (the held one becomes current, so it can be rated or opened with `M`). Clicking the held side does nothing more than a click on the image does today |
| U6-Q7 | Layout | Side by side by default; **V** switches to above/below and back; remembered in `settings.txt` |
| U6-Q8 | Locked zoom and position | **L** locks and unlocks; locked by default, remembered in `settings.txt`. Locked: the held image always shows the current image's zoom level (fit, or the same scale, so 100 % is 100 % on both) and the same relative point of the image; zoom and drag on either side move both. Unlocked: the keys zoom the current image only, and the mouse zooms and drags the side under the pointer |
| U6-Q9 | What else follows the current image only? | Ratings, labels, `M`, the title line, Info, Zoom and Histogram in the right column. **R** (RAW ↔ JPEG, G4) switches both sides, so the two are always compared in the same mode. The clipping marks (J, Shift+J) are drawn on both sides |
| U6-Q10 | What each side shows | On each side, at the top, one line: file name, its source (RAW, embedded JPEG, JPEG or TIFF, G1) and the zoom; the rating badge on both sides as today. The filmstrip frames the held image's cell with a second, dashed frame |
| U6-Q11 | Closing | **C** again or **Escape** closes the compare and leaves the current image in the preview (the held image is let go); a second Escape closes the preview as today. Closing the preview, changing tab or opening a folder closes the compare. Compare is not remembered after a restart; the layout and the lock are |
| U6-Q12 | Names (English, GR14) | View menu items "Compare" (C), "Hold Current Image" (K), "Swap Images" (Shift+K), "Above and Below" (V, ticked when on), "Lock Zoom and Position" (L, ticked when on); the side line as "{file} · {source} · {zoom}"; the terms Compare, Held image, in the glossary. They act in the preview only |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Blocked | Added with T151–T153 (the chain of U6); waits for U6-Q1–U6-Q12 |
| 2026-11-16 | Blocked | The owner approved U6-Q1–U6-Q12 as proposed; written into ADR 0017 §6.1. Waits for the cold read |
| 2026-11-16 | Blocked | Cold read (fresh agent, of T150–T153): the scale clamp before the image's size is known (the held side would stay at 50 %) and the centre clamp spelled out (0–1, not `pan_centre`); equal values compared after the rules; `set_view` outside `act`; `loupe_text.rs`'s two test `Frame` literals named. T151: a bad settings value is reported, not ignored; the saved-text test extended; the side line made from one template per source; positions in the stepped list mapped to file indexes. T152 was too big (the image area is drawn inline in `draw`) and is split: T152 the second core and the image area as one function, T153 the compare drawn, T154 the lock (was T153). U6-Q1 changed with it |
| 2026-11-16 | Open | Cold read's changes made; open |
