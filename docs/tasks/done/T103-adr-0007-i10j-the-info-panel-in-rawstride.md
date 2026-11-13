# T103. ADR 0007 I10j: the info panel in RawStride — Done

- **Can start after T98, T101 and T102 are accepted** (D15 approved on 2026-11-12). Not at the same time as
  T105 or T106 (`main.rs`).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/metadata.rs`;
  `crates/pw-loupe/src/lib.rs` (`Action` and `key_action` only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 and §6.2 (conflicts visible), I10 in the step plan.
- **What** (as D15):
  1. **`lib.rs`:** `Action::ToggleMetadata` on the key `M` (no Ctrl or Alt), in the loupe and the
     contact sheet.
  2. **`metadata.rs`:** the cache keeps the `MetadataRead` (or the error) of **one** image, the one
     shown in the panel. Requests through `MetadataWorker::read_metadata` (T98), at most **one in
     flight**: when the image changes while a read runs, the newest image is asked for when it
     comes back, and results for another image are dropped. A read is asked for only while the
     panel is shown and after the image has been the current one for 200 ms (moving quickly with
     the arrow keys queues nothing). After `RatingWritten` or `EditsWritten` for the image, it is
     read again. A loaded folder change empties it.
  3. **`main.rs`:** a panel on the right (`egui::SidePanel::right`, width 320, resizable, its own
     scroll area), shown and hidden with `M`, hidden at start, not remembered between runs. The
     image is the loupe's current image, or in the contact sheet the single selected image; with
     several selected `PanelText::Selected(n)`, with none `PanelText::NoneSelected`. Contents:
     `PanelText::Loading` while loading; `PanelText::ReadError(fel)` on an error; otherwise
     the file name and three sections from T102's `info_panel`, headed by `PanelText::Values`
     (label, value, origin in grey), `PanelText::Conflicts` (shown only when there are any, in the
     warning colour) and `PanelText::Notices` (only when there are any). A file kind without
     metadata support shows `UNSUPPORTED_TEXT`.
- **Text rule** (as T102): the panel's drawing code in `main.rs` holds no user-visible string
  literals; every text comes from `info_panel.rs` (`info_panel`, `panel_text`). A text that turns
  out to be missing is not written in `main.rs`: report it in the hand-over, and it is added to
  `info_panel.rs` as a whole-sentence template. The panel does not assume a fixed text width (labels and values wrap).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. The rating path is unchanged.
- **Tests to add:** `key_action` for `M` (with and without Ctrl); `metadata.rs` with the existing
  fake submitter: one read in flight, the newest image asked for after it, a stale result dropped,
  no read before 200 ms or while hidden, a new read after a rating write of the image.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand: the panel shows the
  values of a JPEG, a DNG and an ARW with a sidecar, and a conflict (a file from the level-C samples
  or one made by the reviewer).
