# T302. A batch that applies a template file to many images — Blocked

- **Why:** ADR 0017 U13-Q8 and U13-Q27 (owner, 2026-11-24; part U13d). The right-click menu's
  **Apply Template** writes a template file to every target image "in the same batch as Apply to
  {n} Marked" (U13-Q8). Today the batch in `metadata.rs` (`start_batch`, `EditorBatch`) sends
  only the editor's edits, belongs to the metadata window (its image is read again at the end)
  and gives its summary as an `EditorEvent`, which only the open metadata window takes. This
  task lets the same batch apply a `TemplateFile` with no metadata window behind it and keep its
  summary for the main window (T303 shows it).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/metadata.rs`,
  `docs/test-inventory.md`.
- **Background (read before starting):** ADR 0017 §6.5 U13-Q8, U13-Q19, U13-Q27, U13-Q28; §2.3.1
  and U12c-Q5, U12c-Q6, U12d-Q10 (the batch of "Apply to {n} Marked"). `metadata.rs`:
  `MetadataSubmit` and its `impl` for `MetadataWorker`, `EditorBatch` (`pending_paths`,
  `answered`), `start_batch`, `send_batch`, `finish_batch`, `batch_written`, `batch_running`,
  `batch_progress`, `write_pending`, the `WorkerEvent::EditsWritten` arm of `apply`,
  `take_editor_events`, `BatchSummary` (`status_lines`), the exit wait (`unsaved_at_exit` or the
  function that waits for `batch_running`), and the tests' `FakeSubmit` and batch tests
  (`batch_images`). T299's `MetadataWorker::apply_template` (answered with
  `Event::EditsWritten`).
- **What:**
  1. **The request:** `MetadataSubmit::apply_template(image, template: TemplateFile) ->
     Result<RequestId, SubmitError>`, implemented for `MetadataWorker` with T299's request and
     for the tests' `FakeSubmit`.
  2. **One batch, two kinds:** a write of the batch is either the editor's edits or a template
     (one `TemplateFile` shared by every image of a template batch, for example in an `Arc`, not
     cloned per image beyond what the request needs). The batch's window image becomes optional:
     "Apply to {n} Marked" keeps it (unchanged behaviour; `start_batch` keeps its signature and
     stores `Some`); a template batch has none, so no image is read again at its end
     (`finish_batch` sets no `editor.reload` and gives no `EditorEvent` for it; `batch_written`
     compares with the optional window image). The inspector's and the window's reads follow
     each write as today through `EditsWritten`. Both kinds use the one batch slot
     (`editor.batch`), so everything that checks it today (`save_edits` refusing while a batch
     runs, the `EditsWritten` arm, the exit wait's pending files) covers a template batch too.
     The queue may hold only the paths and the shared template (an `Arc<TemplateFile>`), cloned
     once per request when it is sent, since `apply_template` takes it by value.
  3. **Starting it:** `start_template_batch(images: Vec<PathBuf>, template: TemplateFile, worker)
     -> bool` (by this name, as T303 calls it): each image its own `apply_template`, sent as the queue
     takes them, in the order given; refused (false, nothing sent) while the editor's write or
     any batch runs (`editor.writing` or `editor.batch`), as `start_batch` is, and refused for
     an empty list of images. `start_batch` is likewise refused while a template batch runs.
  4. **Its end:** when every write has answered, a template batch gives its `BatchSummary` not as
     an `EditorEvent` but kept for the main window, taken with a new method (by this name,
     as T303 calls it: `take_template_batch_done() -> Option<BatchSummary>`), which the metadata window's
     `take_editor_events` never takes. Every image's notices are listed in `kept` (there is no
     window image), with the same texts as the other images of "Apply to {n} Marked"
     (`ConflictKept`, `QualifiersKept`, `Truncated`, `IimConvertedToUtf8`, `DigestNotWritten`);
     a failed image is in `failed` with its reason, as today.
  5. **While it runs:** `batch_running` and `batch_progress` are true and give the progress for
     either kind (the metadata window, if opened meanwhile, is locked and shows the progress as
     during its own batch); `template_batch_progress() -> Option<(usize, usize)>` gives only a
     template batch's progress (as `batch_progress` counts it), so the main window can show it
     in the status bar (T303 calls it by this name).
     `write_pending` and the exit wait count a template batch's pending writes as they count
     the other batch's, and the exit wait (`finish_pending_writes`) also takes
     `take_template_batch_done` and adds its `failed` to the failures it lists. That the
     metadata window is locked during a template batch, and that its Save does nothing then,
     is intended (U13-Q27).
  6. **Unused until T303:** `metadata.rs` is a module of the binary, so the new methods that only
     T303 calls get `#[cfg_attr(not(test), allow(dead_code))]`, as the file's other test-only
     methods have, so clippy stays without warnings; `main.rs` is not changed (T303 removes the
     attributes when it calls them; `metadata.rs` is in T303's files for that only).
- **Tests to add** (`metadata.rs`, with `FakeSubmit`, which records each `apply_template` (id,
  path, template) and counts it against the same queue limit as `write_edits`, as the real
  worker has one queue): a template batch of three images sends
  three `apply_template` in order and, when all answer, gives one summary through the new method
  and none through `take_editor_events`, with no read of any image asked for (no window or
  inspector open on them); an empty list is refused; a failed image is
  in `failed` and the others counted as written; the notices of every image are in `kept` (one
  of each kind named in point 4); a full queue holds the rest until it takes them; a template
  batch is refused while "Apply to {n} Marked" runs and the other way round; `write_pending` is
  true for a template batch's image until its answer; the exit wait waits for a template
  batch and lists its failed image. The existing batch tests pass unchanged. The test inventory's `pw-loupe` row and totals
  are updated.
- **Not in this task:** the right-click menu, the question, the status bar and the message box
  (T303); reading the template file (T303 uses the template thread); any new text.
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Order:** after T299 is committed. May run beside T300 or T301 only if neither changes
  `metadata.rs` (they do not). T303 waits for it.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  No hand check of its own (T303's covers it).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0017 U13-Q8, U13-Q27, owner, 2026-11-24; part U13d). Waits for T299, its cold read and the chain review of T299–T303 |
| 2026-11-24 | Blocked | Cold read (fresh agent): the task fits `metadata.rs`. Added: the one batch slot for both kinds (`finish_batch` and `batch_written` with an optional window image; `start_batch` keeps its signature); an empty list refused; the shared template cloned once per request; the exit wait also lists a template batch's failed images; the window's lock during a template batch intended; `allow(dead_code)` outside tests until T303 (new point 6); `FakeSubmit`'s record and one shared queue limit; "no window open" in the first test |
| 2026-11-24 | Blocked | Chain review (T299–T303, one fresh agent; recorded in T299's log): the names T303 calls fixed (`start_template_batch`, `take_template_batch_done`, `template_batch_progress`); T303 removes the `dead_code` attributes (`metadata.rs` in its files for that only) |
