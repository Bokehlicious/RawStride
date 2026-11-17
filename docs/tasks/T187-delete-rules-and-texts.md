# T187. ADR 0018 B3, first part: the delete's plan, batch and texts in the shell (pw-loupe) — Blocked: waits for T186

- **Chain:** T185 (B1) → T186 (B2) → T187, T188 (B3) → T189 (B4) → T190 (B5). The owner's
  answers are B-Q1–B-Q15 in ADR 0018 §3; how the steps fit is ADR 0018 §2.10.
- **Why:** ADR 0018 §2.1, §2.5, §2.7. The shell's plain-Rust part, tested without a window
  (GR11), as T172 was for the moves: the confirmation's content from T186's `DeletePrepared`
  answer, a batch of deletes sent behind queued rating writes and gathered when every answer has
  come, the undo list of moves forgetting deleted files, and the texts. T188 connects the
  metadata cache, T189 the keys, the menu and the window.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/deletes.rs` (new),
  `crates/pw-loupe/src/deletes_text.rs` (new), `crates/pw-loupe/src/moves.rs` (only point 4),
  `crates/pw-loupe/src/lib.rs` (the modules and exports), `docs/glossary.md`,
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  0. **Comparing paths without regard to case** (`deletes.rs`): `pub fn same_path(a: &Path,
     b: &Path) -> bool`: the two paths have the same number of components and each pair is equal
     after `to_string_lossy().to_lowercase()` (Unicode, so å/Å match); the same on every platform;
     no I/O (`tabs::same_folder` does I/O and is not used). Every "compared without regard to
     case" below uses it; `moves.rs` uses it in point 4.
  1. **The plan** (`deletes.rs`): `pub struct DeletePlan { pub images: Vec<PathBuf>, pub files:
     usize, pub bytes: u64, pub partners: Vec<PathBuf>, pub refused: Vec<(PathBuf, String)> }` (the reason as text: the
     `DeleteRefusal`'s `Display`, or a reason of the shell's own, as T190 adds) (`Clone`, `Debug`,
     `PartialEq`, `Eq`) and `pub fn plan_delete(prepared: Vec<PreparedDelete>, is_image: impl Fn(&Path) -> bool) ->
     DeletePlan` (T186's types; `is_image` is true for a file whose extension is one of the
     image types, `pw_raw::SUPPORTED_EXTENSIONS` without regard to case, which the caller
     passes), going through the answers in order:
     - an image whose path is one of the files of an earlier image's `Ok` group that was kept
       (for example the JPG of a marked RAW+JPEG pair) is left out entirely, also when its own
       answer is an `Err`: it goes with that group. `relocate::companions` lists every partner of
       any image of a pair, so a pair's two groups hold the same files whichever comes first;
     - an `Ok` group that is not left out adds its image to `images`; `files` is the number of
       distinct files (compared the same way) of those groups, and `bytes` the sum of their
       `bytes` (a left-out image's group is the earlier group itself, so nothing is counted
       twice);
     - an `Err` adds `(image, reason.to_string())` to `refused`;
     - `partners` (owner, 2026-11-17, ADR 0018 B-Q18): the image files (`is_image`) of the kept
       groups that are not among `images` and not their image, each once (`same_path`), in the
       order met: the RAW+JPEG partner, and also a TIFF or DNG of the same name, which go along
       by B-Q3. The confirmation names them, so nothing goes unseen.

     The plan is what the confirmation shows (§2.5) and what the batch sends.
  2. **One batch** (`deletes.rs`): `#[derive(Clone, Debug)] pub struct DeleteBatch`, as T172's
     `Batch` but for deletes; its images are compared as given (they come from the plan), as
     `Batch` does, and `sent` or `answered` for an image not in the batch, or already answered,
     changes nothing:
     - `DeleteBatch::new(images: Vec<PathBuf>)`; `contains(&self, image: &Path) -> bool`;
     - `to_send(&self, is_waiting: impl Fn(&Path) -> bool) -> Vec<PathBuf>`: the images not yet
       sent whose path has no rating write queued or sent without its answer (`is_waiting`, the
       metadata cache's `is_pending`, as the moves use; ADR 0018 §2.3, B-Q10). It marks nothing;
       `sent(&mut self, image: &Path)` marks one once the worker took it; `skip(&mut self,
       image: &Path)`: not sent and needs no answer (as `Batch::skip`; T190 skips an image whose
       reject was cleared while the delete ran);
     - `answered(&mut self, image: &Path, result: Result<Vec<PathBuf>, DeleteError>)`;
     - `done(&self) -> bool` once every image has its answer, and `finish(self) -> DeleteResult
       { images: Vec<PathBuf>, files: Vec<PathBuf>, failed: Vec<(PathBuf, DeleteError)> }`:
       `images` the images whose answer was `Ok`, `files` every file of those answers **and the
       `deleted` files of the failed ones** (T185: a group Windows deleted only partly), `failed`
       the others. A failed answer whose image is among another answer's `files` (`same_path`) is
       not a failure and is dropped entirely (its file is in `files` through the other answer;
       intended). `DeleteResult` derives `Clone, Debug, Default, PartialEq, Eq`.
  3. **Where a delete is** (`deletes.rs`): `#[derive(Clone, Debug, Default)] pub enum DeleteStep
     { #[default] Idle, Preparing { images: Vec<PathBuf>, id: Option<pw_metadata::RequestId> },
     Confirming(DeletePlan), Running(DeleteBatch) }` (`pw_metadata::RequestId`, not the crate's
     own `RequestId` of the import worker) and `pub fn busy(&self) -> bool` (not `Idle`). One delete at a time;
     T189 drives it. `Preparing` holds the images until the request is taken (`id` `None` while
     the worker's queue is full).
  4. **The undo list of moves forgets deleted files** (`moves.rs`, only this): `pub fn
     forget_deleted(&mut self, deleted: &[PathBuf])` on `UndoMoves`: in each step, every
     `MovedImage` with any `Moved::to` among `deleted` (`same_path`) is dropped; then a partner's
     entry (`moved` empty, as `Batch::finish` keeps a JPG moved along by its RAW) is dropped too
     when its `image` is a `Moved::from` of an entry dropped in the same step; a step left empty
     is dropped. A pair is deleted together (B-Q3), so "any" and "all" of an entry's files are
     the same in practice. The dropped entries' `restore` ratings are lost (intended: the files
     are gone). So deleting images in `_rejected` does not leave an undo step that can only fail
     (ADR 0018 §2.10).
  5. **Texts** (`deletes_text.rs`, rule 13; whole sentences from one template each, singular and
     plural in the same function; each case its own whole `format!` template; sizes with
     `pw_core::format_bytes`, as `import_text.rs` does; `{reason}` is
     `DeleteError::reason.to_string()` (the `DeleteRefusal`'s phrase, not `DeleteError`'s
     sentence) and `{name}` the file name of `DeleteError::file` (the file that stopped the
     image, for example a locked `.XMP`), as the move texts take `MoveError`'s; as there, no full
     stop follows `{reason}` at the end of a text, since a system error text may end in one):
     - `delete_title_text()`: "Delete to Recycle Bin";
     - `delete_confirm_text(images: usize, files: usize, bytes: u64)`, four templates: one image
       and one file "Move 1 image (1 file, {size}) to the Recycle Bin? You can restore it from
       the Recycle Bin in Windows."; one image and more files "Move 1 image ({files} files,
       {size}) to the Recycle Bin? You can restore it from the Recycle Bin in Windows."; more
       images (and files) "Move {images} images ({files} files, {size}) to the Recycle Bin? You
       can restore them from the Recycle Bin in Windows."; `images` 0 gives the plural template
       (never called so);
     - `delete_refused_text(count: usize, name: &str, reason: &str)`: "{name} cannot be deleted:
       {reason}" for 1 (and 0), "{count} images cannot be deleted. The first: {name}: {reason}"
       for more (shown in the window under the question, and in the status line when nothing can be
       deleted);
     - `delete_partners_text(names: &[String])`: for one "This image file of the same name goes
       along: {name}." and for more "These image files of the same name go along: {names}." (the
       names joined by ", "; B-Q18);
     - `delete_button_text()`: "Delete"; `delete_cancel_text()`: "Cancel";
     - `deleted_text(count: usize)`: "1 image moved to the Recycle Bin." / "{count} images moved
       to the Recycle Bin." (ADR 0018 §2.7);
     - `delete_failed_text(name: &str, reason: &str)`: "{name} could not be moved to the Recycle
       Bin: {reason}";
     - `delete_partly_failed_text(name: &str, reason: &str, left: &[String])`: "{name} could not
       be moved to the Recycle Bin: {reason}. Some of its files are already in the Recycle Bin;
       these were left: {left}." (the paths joined by ", "; ADR 0018 §2.4);
     - `deletes_failed_text(count: usize, name: &str, reason: &str)`: "{count} images could not
       be moved to the Recycle Bin. The last: {name}: {reason}" for 2 or more; for 1 (and 0) the
       same text as `delete_failed_text`;
     - `delete_while_moving_text()`: "Images are still being moved. Delete again when the move
       is done.";
     - `delete_running_text()`: "A delete is still running. Try again when it is done.".
  6. **Exports** (`lib.rs`): `mod deletes; mod deletes_text;` and `pub use` of everything public
     in them: `same_path` (`main.rs` is a separate binary crate and uses it in T189), `DeletePlan`, `plan_delete`, `DeleteBatch`,
     `DeleteResult`, `DeleteStep`, and every text function of point 5, as `moves` and
     `moves_text` are exported (so nothing is dead code before T189 uses it).
  7. **Glossary** (`docs/glossary.md`, the 3-column form, "–" in the Swedish column as the newer
     rows), in §3 "Window, menus and commands": "Recycle Bin" (Windows' own name, not translated
     by RawStride), "Delete to Recycle Bin" (T189's menu item, key Delete), "Delete Rejected
     Images" (T190's menu item), "RAW+JPEG pair" (an image's RAW and JPEG files of the same
     name, deleted together, B-Q3; T190's reason uses it), and "Delete" and "Cancel" as the
     confirmation's buttons when the glossary lacks them; each note naming ADR 0018 and T187.
- **Tests to add:** `deletes.rs`: `plan_delete` with two images and their counts and size; a
  marked RAW and its JPG (the JPG left out, files and size counted once, no partner named); a
  RAW alone whose JPG and TIFF of the same name are in its group (both in `partners`); a refused image beside
  a good one; only refused images (empty `images`). `DeleteBatch`: `to_send` leaves out an image
  with a pending write and gives it once the write is answered, never after `sent`; `finish`
  sorts answers into deleted and failed and drops the failed answer of a partner deleted by
  another image. `DeleteStep::busy`. `moves.rs`: `forget_deleted` drops a moved image whose file
  was deleted, keeps the others, and drops a step left empty (the existing tests do not change).
  `same_path`: equal in other case (also å/Å), different, different lengths. `deletes_text.rs`:
  each text with a count with 1 and with more (and the four templates of
  `delete_confirm_text`); each text without a count once.
  `forget_deleted` also: a RAW+JPG step from `Batch::finish` (the JPG's entry with no moves) is
  dropped whole when the RAW's moved files are deleted.
- **Known limit:** a reason from the system (`DeleteRefusal::Failed`) is Windows' own text, in
  Windows' language, as for the moves.
- **Not in this task:** the metadata cache (T188); keys, menu, window and wiring (T189); "Delete
  Rejected Images" (T190).
- **Rules:** `AGENTS.md` §3; `docs/tasks/README.md` rule 13 (the texts are made here; no
  drawing code in this task).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows no change from this
  task outside the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked: waits for T186 | Added for ADR 0018 B3, written with B2–B5 for the chain review |
| 2026-11-17 | Blocked: waits for T186 | Cold read: doable with the named files. Changed: `same_path` (point 0) for every case-blind comparison; the plan's rule for a pair stated (a pair's groups hold the same files), and an `Err` image inside a kept group left out; the batch's comparisons and unknown answers; derives and `pw_metadata::RequestId`; `forget_deleted` also drops a partner's entry with no moves, and loses `restore` on purpose; `{reason}` and `{name}` defined (the reason phrase; the file that stopped the image), no full stop after `{reason}` as in the move texts; each count case its own template, 1 and 0 defined; exports named; glossary section and the buttons; tests adjusted |
| 2026-11-17 | Blocked: waits for T186 | From T189's and T190's cold read: `same_path` is `pub` and exported (`main.rs` is a binary crate of its own); "RAW+JPEG pair" added to the glossary rows |
| 2026-11-17 | Blocked: waits for T186 and T183 | Chain review (fresh agent, T185–T190 read together): waits for T183 too (both change `moves.rs`, `lib.rs`'s exports and `test-inventory.md`); `DeleteBatch::skip` added for T190; `finish` puts a partly failed group's `deleted` files in `files` |
| 2026-11-17 | Blocked: waits for T186 and T183 | Owner, from the chain review (ADR 0018 B-Q18): the plan's `partners` and `delete_partners_text` name the image files of the same name that go along (a TIFF or DNG too, not only the RAW+JPEG partner) |
| 2026-11-17 | Blocked: waits for T186 | T183 is Done: waits for T186 only. T183 changed `moves.rs` (`moved_to_folders`), `lib.rs`'s exports and `main.rs` (`relist_tabs`, `batch_done`, `undo_done`, `undo_move`): line numbers to be checked when this task is set Open |
