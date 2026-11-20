# T237. ADR 0024 O3c: Rotate Left and Rotate Right in the shell (keys, menu, writes, rollback) — Done

- **Why:** ADR 0024 §2.1 and §2.5 (O-Q1, O-Q2, O-Q8, O-Q10, O-Q11; accepted by the owner
  2026-11-17): **Ctrl+←** turns 90° anticlockwise and **Ctrl+→** 90° clockwise, in the contact
  sheet and the preview, with "Rotate Left" and "Rotate Right" in the Image menu; 180° is two
  presses. The targets are those of a rating key (the marked images in the contact sheet,
  otherwise the current one; the current image in the preview). The image turns at once in the
  view and its thumbnail, the write goes into the ratings' ordered queue, and a failed write
  turns it back and shows the reason. The undo is a turn the other way (Ctrl+Z stays the undo of
  moves). Memory cards follow the rating rule of today (the same files, `supports_rating`).
  T235 made the view keep the zoomed point; T236 made `OrientationStore` and
  `EmbeddedOrientationReader`; this task wires them in and ends ADR 0024. Last task of the chain
  T235 → T236 → **T237** (plan O3 row); the owner's hand check of ADR 0024 follows it.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/metadata.rs` (point 1),
  `crates/pw-loupe/src/main.rs` (points 2–4), `crates/pw-loupe/src/shell.rs` (point 5),
  `crates/pw-loupe/src/shell_text.rs` (point 6), `crates/pw-loupe/src/orientations.rs` (point 3:
  the key map only), `crates/pw-loupe/src/lib.rs` (re-exports and `command_of_key`, point 3),
  `docs/test-inventory.md`, and this task's own file (status and log). The glossary terms come
  with T236.
- **What:**
  1. **`MetadataCache`** (`metadata.rs`) keeps the turns in an `OrientationStore` instead of
     `orientations` and `orientation_changes`, and `read_orientations` goes:
     - `RatingsRead`: `store.read(id, &orientations, &paths)` with the same `paths` the ratings
       get.
     - `OrientationWritten { id, path, result, .. }`: as `RatingWritten` does — `window_written`,
       `inspector_written` and a `WriteAnswer` (the file was written), then
       `store.written(id, &path, result.map_err(write_error_text))`, its message pushed to
       `failures`.
     - `poll` pumps the store after the ratings (`failures.extend(store.pump(worker))`);
       `MetadataSubmit` gets `OrientationWriter` as a second supertrait beside `RatingWriter`,
       and the test double (`FakeSubmit`) implements it, answering as its `write_rating` does,
       with a field of its own that records each orientation write (path and value) and an
       entry in `operations` as `write_rating` makes one.
     - A stopped worker: `failures.extend(store.fail_sent())` beside the ratings'.
     - `prune` calls `store.prune(keep)` (its changes replace the T232 code there).
     - `orientation(path)` and `take_orientation_changes()` keep their signatures and give
       `store.orientation` and `store.take_changes`, so `follow_orientations` and the
       thumbnails' check in `main.rs` stay as they are.
     - New: `turn(&mut self, paths: &[PathBuf], clockwise: bool) -> Result<Vec<PathBuf>, RatingRefused>`
       (`store.turn_many`) and `embedded_read(&mut self, path: &Path, orientation: Option<u16>)`.
     - `is_pending(path)` is true while a rating **or** a rotation write of the path is queued or
       sent, **or** a turn of the path waits for its embedded value (`store.is_waiting`): moves,
       deletes and "Open in" wait for all of them (T178, T201), so a turn's write never lands on
       a moved file. `pending_writes()` counts the rating and rotation writes (the wait at exit,
       and the Tasks area, whose line "Writing ratings: {count}" then counts rotations too —
       accepted for now, `backlog.md`); `finish_pending_writes` lists in `unsaved.pending` the
       files of both kinds of writes and the store's `waiting_paths` (the exit wait does not
       wait for those: the reader's answers come through `main.rs`), each file once.
     - `write_pending`'s doc comment ("a rating write") names the rotation writes and the
       waiting turns too.
     - The two queues stay apart: the ratings are pumped before the rotations in each `poll`,
       so a rating and a turn of the same frame are sent ratings first; they write different
       tags, and both go into the worker's one ordered queue. Do not merge the stores.
     - `failure_text()` gives the text of the store whose failure came last (a rating or a
       rotation), so the status line names the right kind; keep which one in a field set where
       each store's failures are pushed (`written`, `pump`, `fail_sent`). When both fail in one
       `poll`, the rotation's is last (the store is pumped after the ratings). When the last
       kind's store has no text, the other store's text is used.
  2. **`main.rs`, the action:** a field `orientation_reader: EmbeddedOrientationReader`, started
     where `capture_reader` is. `fn rotate(&mut self, clockwise: bool)`: the targets are
     `key_targets()`; with none, nothing. `metadata.turn(&targets, clockwise)`:
     - `Ok(asks)`: each path in `asks` is sent to `orientation_reader.request`; when that returns
       false (the reader could not start or has stopped), the value is read at once on the UI
       thread, `pw_raw::inspect(&path).ok().map(|info| info.orientation)`, and given to
       `metadata.embedded_read` (one header read; `None` only when inspect fails). Starting
       from 1 instead would write the file's own value without turning it, or jump 180°.
     - `Err(RatingRefused::Unsupported)`: the status line shows `ROTATION_UNSUPPORTED_TEXT`;
       another error shows its text (as `rate` does).
     Each frame, before `follow_orientations`, every answer of `orientation_reader.try_recv()` is
     given to `metadata.embedded_read`. When `try_recv` gives `Err(ReaderStopped)`, every path
     of the store's `waiting_paths()` is answered the same way on the UI thread and the count
     of reads in flight goes to 0 (otherwise moves, deletes and "Open in" would wait for ever,
     and the repaint loop would never stop). A turn does not change the marking, the filter or the
     current image, and moves nothing.
     **Redraw:** keys are handled after `follow_orientations` in the frame, so `rotate` calls
     `ctx.request_repaint()` (the turn reaches the view and the thumbnail in the next frame at
     once, not at the next mouse move). While embedded reads are in flight (a count kept in
     `main.rs`: up for each request sent, down for each answer), the frame asks
     `ctx.request_repaint_after(CAPTURE_POLL_INTERVAL)`, as `follow_sort` does for the capture
     times.
  3. **The keys:** `rotation_key_action(key, ctrl, shift, alt) -> Option<bool>` in
     `orientations.rs` (re-exported from `lib.rs`): Ctrl+← → `Some(false)` (anticlockwise),
     Ctrl+→ → `Some(true)`; with Shift or Alt, or any other key, `None`. In `main.rs` it is
     checked where `rating_key_action` is, under the same condition
     (`!ctx.egui_wants_keyboard_input()`), and a repeated key event (held key) is ignored, so a
     held Ctrl+→ turns once. Ctrl+←/→ do nothing today in either view (`grid_key_action`,
     `key_action`, `preview_key_action` and the other key maps return nothing for them), so no
     existing key changes. `command_of_key` (`lib.rs`) gives `Command::RotateLeft` /
     `Command::RotateRight` for them, checked beside `rating_key_action`, so the existing test
     `every_menu_key_gives_its_items_command` holds for the new items.
  4. **The menu items** run the same action: `Command::RotateLeft` → `rotate(false)`,
     `Command::RotateRight` → `rotate(true)`; enabled exactly when the rating items are
     (`command_enabled`, beside `Stars`/`Reject`), acting in both views (`acts_in` gives
     `Where::Both`).
  5. **`shell.rs`:** `Command::RotateLeft` and `Command::RotateRight` (doc comments with ADR 0024
     §2.1, O-Q1, T237); in the Image menu after the Label submenu and before the separator and
     `ToggleMode`: a separator, then `item(Command::RotateLeft, ctrl(Key::ArrowLeft))` and
     `item(Command::RotateRight, ctrl(Key::ArrowRight))`. Not in `IMAGE_CONTEXT_MENU` (its items
     and order are ADR 0019's, E-Q17; ADR 0024 names only the Image menu).
  6. **`shell_text.rs`:** `command_text` gives
     "Rotate Left" and "Rotate Right"; `key_text` shows `Key::ArrowLeft` as "←" and
     `Key::ArrowRight` as "→" (so the menu shows "Ctrl+←", "Ctrl+→", not egui's names).
- **Tests to add:**
  - `orientations.rs`: `rotation_keys_match_adr_0024`: Ctrl+← / Ctrl+→ give `Some(false)` /
    `Some(true)`; without Ctrl, with Shift or Alt, and Ctrl with ↑/↓ give `None`.
  - `metadata.rs` (with `FakeSubmit`):
    - `a_turn_is_shown_at_once_written_and_confirmed`: a folder read with orientation 6 for a
      supported path → `turn([path], true)` → `orientation(path)` is 3, the change is taken, the
      next `poll` sends one orientation write of 3; its `Ok` answer keeps 3 and adds a
      `WriteAnswer`.
    - `a_failed_rotation_turns_back_and_its_text_is_the_status_text`: the answer `Err` → the
      orientation is 6 again, a change is recorded, `poll` returns the rotation message and
      `failure_text()` gives the rotation text; a later failed rating write makes
      `failure_text()` the rating text.
    - `a_queued_rotation_holds_back_a_move`: `is_pending(path)` is true from the turn until the
      answer, and `pending_writes()` counts it; a turn waiting for its embedded value also
      makes `is_pending` true, until `embedded_read`.
    - `a_folder_read_while_a_rotation_is_sent_keeps_the_turn` (plan O3 row, need (a)).
    - `a_stopped_worker_fails_a_sent_rotation`.
    - `the_exit_wait_lists_a_pending_rotation`: `finish_pending_writes` with a rotation that is
      never answered lists its file in `unsaved.pending`; so does a turn still waiting for its
      embedded value.
  - `shell.rs`: `the_rotate_items_follow_the_label_submenu_in_the_image_menu` (their keys
    Ctrl+← and Ctrl+→, `Where::Both`, not in the context menu).
  - `shell_text.rs`: the two menu texts and `key_text` of Ctrl+← and Ctrl+→.
  - **Existing tests:** `shell_text.rs`'s `every_menu_item_text_in_menu_order` and
    `every_menu_key_text_in_menu_order` get "Rotate Left"/"Rotate Right" and "Ctrl+←"/"Ctrl+→"
    after "No label" and before "RAW ↔ JPEG"/"R"; no other existing test changes (rule 5). If
    another one fails, stop and report it.
- **Not in this task:** the zoom rule (T235); the store's rules (T236: change them only by
  stopping and asking); the right-click menu (ADR 0019 E-Q17); the locked compare (the shell copies the
  current core's turned centre to the held side; nothing is changed there, it is looked at in
  the hand check); turning a RAW+JPEG pair's partner (a turn acts on the file turned, as a
  rating key does); showing the orientation in Info
  or the metadata window (O-Q7: nothing special shown); mirror commands, free angles, pixel
  turns (ADR 0024 §5); a `CHANGELOG.md` line (the reviewer adds it at Done, `docs/tasks/
  README.md`, "The change log").
- **Rules:** `docs/tasks/README.md` rules 1–13. No new dependencies. No `.unwrap()`,
  `.expect()` or unchecked indexing outside tests. User-visible texts only through
  `shell_text.rs` and `rating_text.rs` (rule 13).
- **The owner's hand check** (ADR 0024 §4, after Done; written in the plan's O3 row), on a copy
  folder such as `D:\PhotoWorkflowData\O-Q12-rotation`: a RAW and a JPEG turned with Ctrl+→ and
  Ctrl+← in the contact sheet and the preview, the thumbnail and the view turn at once; several
  marked images turned with one key; at 100 % the same point stays in the centre, at fit the
  image stays fit; near an edge at 100 %, a turn and a turn back (T235's review: the view may end
  a little off where it started, since the drawn point is clamped after the turn); in the compare with the zoom locked (L), what the held side does when the
  current image is turned (T235: the shell copies the turned centre to it; a task only if it
  looks wrong); a failed write (a JPEG made read-only) turns back and the status line says
  why; after a restart the turns are still there (the RAW from its sidecar, the JPEG from its
  Exif); a DNG (and a TIFF if at hand), during the session and after a restart; a RAW+JPEG
  pair turned in RAW and in JPEG mode (R), noting that the partner file is not turned; with
  many marked images on a slow drive, images without a read value may turn one by one; DxO PhotoLab shows the RAW turned; if one is at hand, a JPEG with no Exif that is not
  JFIF (`backlog.md`, "Creating Exif in a JPEG was measured on JFIF only").
- **Done when:** `.\tools\check.ps1` is all OK and `git status` shows only the files above
  changed.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Blocked | Added (chain T235 → T236 → T237, ADR 0024 O3); waits for T236 |
| 2026-11-20 | Blocked | Cold read: doable; the key maps, `MetadataSubmit`, `failure_text`'s use and the repeat flag checked. Fixed: `command_of_key` in `lib.rs` (its existing test would fail); the two `shell_text.rs` list tests named; a repaint after a turn and while embedded reads are in flight; "Writing ratings" in the Tasks area counts rotations (accepted, backlog); `failure_text` when both fail in one poll; `FakeSubmit`'s field; a turn waiting for its embedded value holds back moves, deletes and "Open in" and is listed at exit (T236 got `is_waiting`/`waiting_paths`); the glossary moved to T236; the locked compare added to the hand check |
| 2026-11-20 | Blocked | Chain review (T235's log): a reader that cannot start or has stopped is answered with `pw_raw::inspect` on the UI thread; `write_pending`'s doc comment; the queues kept apart; the locked compare and a pair's partner named under Not in this task; DNG/TIFF and a RAW+JPEG pair in the hand check |
| 2026-11-20 | Open | T236 done; set to Open |
| 2026-11-20 | Ready for review | Points 1–6 and every listed test done; files as named. Beyond the text: `MetadataCache::waiting_orientations()` (the store's `waiting_paths`, which `main.rs` needs when the reader has stopped); `rotate` takes the `egui::Context` for its `request_repaint`; `finish_pending_writes` lists the waiting turns also when nothing else is pending (it no longer returns before the listing, still without reading the clock), and dedupes the rating, rotation and waiting paths (the editor's and the batch's paths are listed as before). check.ps1 all OK, 2051 passed, 75 ignored, no clippy warnings |
| 2026-11-20 | Done | Reviewed: the diff holds only the named files and matches points 1–6 and the tests; no existing test changed beyond the two text lists named; no existing key takes Ctrl+←/→ (`grid_key_action` and `key_action` drop every Ctrl chord but Ctrl+A); `failure_text` is read only when failures come in the same frame, so the last-kind rule holds. The agent's additions accepted (`waiting_orientations`, `rotate` taking the context, `finish_pending_writes` listing waiting turns without reading the clock). The reviewer's own clean run: `.\tools\check.ps1` all OK, 2051 passed, 75 ignored, no clippy warnings. CHANGELOG [Unreleased] Added line. The owner's hand check of ADR 0024 follows (above) |
| 2026-11-20 | Done | The owner's hand check of ADR 0024 (`D:\PhotoWorkflowData\RAW - kopia`): every point passed except a JPEG with no Exif that is not JFIF (none at hand; the backlog item stays). A RAW+JPEG pair's partner is not turned: accepted as it is. Found: a turn of a Sony ILCE-1 JPEG is always refused by the safe write's verify and turned back (ExifTool fixes `Sony:HiddenDataOffset` when it writes IFD0 Orientation; backlog); a Panasonic JPEG turns and is saved. A failure text from an earlier turn stayed in the status line until cleared, which first looked like a wrong file named; not a fault |
