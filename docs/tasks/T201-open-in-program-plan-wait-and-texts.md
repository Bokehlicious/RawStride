# T201. ADR 0019 E2, first part: the plan, the wait, the start and the texts of "Open in {name}" (pw-loupe) — Open

- **Chain:** T200 (E1, done) → T201, T202 (E2) → T203 (E3). The owner's answers are E-Q1–E-Q14 in
  ADR 0019 §3. The owner's hand check comes after E3 (ADR 0019 §4) and is written in T203.
- **Why:** ADR 0019 §2.3–§2.6, step E2. The plain-Rust part, tested without a window (GR11), as
  T187 was for the deletes: the length of the command line and how many images fit, the question
  above 100 images, a request that waits for the images' writes, the start of the program and its
  errors, and the texts. T202 wires them to the menu, Ctrl+E and the confirmation.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/external.rs` (new),
  `crates/pw-loupe/src/external_text.rs` (new), `crates/pw-loupe/src/metadata.rs` (only point 6),
  `crates/pw-loupe/src/lib.rs` (the modules and exports), `docs/glossary.md`,
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **Limits** (`external.rs`): `pub const MANY_IMAGES: usize = 100;` (above this a question is
     asked, E-Q8) and `pub const COMMAND_LINE_LIMIT: usize = 32_767;` (Windows' limit of a command
     line in UTF-16 units, the ending NUL included).
  2. **The length of the command line** (`external.rs`): `pub fn command_line_length(program:
     &Path, images: &[PathBuf]) -> usize`, in UTF-16 units, as `std::process::Command` builds it on
     Windows (`make_command_line` and `append_arg` in `std`, checked by the cold read): the
     program in double quotes with nothing escaped (its length + 2); for each image one space,
     then the path, in double quotes (+ 2) when it is empty or holds a space or a tab; a `"` in
     a path costs n + 1 more units, n the backslashes right before it (never in a Windows path,
     counted anyway); a quoted path's backslashes at its end count twice; finally 1 for the
     ending NUL. A `.bat` or `.cmd` program is built otherwise by `std` (through `cmd.exe`); it
     is measured the same way anyway (Known limit).
     Each path's length is `to_string_lossy().encode_utf16().count()` (the same on every
     platform; an unpaired surrogate becomes U+FFFD, still one unit), so a character outside the
     BMP counts 2; no I/O.
     `pub fn images_that_fit(program: &Path, images: &[PathBuf]) -> usize`: the largest `n` such
     that `command_line_length(program, &images[..n]) <= COMMAND_LINE_LIMIT` (0 when not even the
     program alone fits); one pass that adds each image's length, not a call per `n`, and no
     indexing past the list (use `get` or iterators).
  3. **The request and the plan** (`external.rs`):
     - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct OpenRequest { pub program:
       ExternalProgram, pub images: Vec<PathBuf> }` (T200's `ExternalProgram`; the images are the
       shell's resolved, absolute paths, in the tab's order);
     - `impl OpenRequest { pub fn ready(&self, is_waiting: impl Fn(&Path) -> bool) -> bool }`:
       true when no image has a write waiting (`is_waiting`, point 6, for each image);
     - `#[derive(Clone, Debug, PartialEq, Eq)] pub enum OpenPlan { Start(OpenRequest),
       Ask(OpenRequest), TooLong { count: usize, fit: usize } }` and `pub fn plan_open(program:
       ExternalProgram, images: Vec<PathBuf>) -> OpenPlan`: `TooLong` (with `count` the number of
       images and `fit` from `images_that_fit`) when the command line is longer than
       `COMMAND_LINE_LIMIT`, whatever the number; otherwise `Ask` when there are more than
       `MANY_IMAGES` images; otherwise `Start`. An empty list gives `Start` with no images (the
       shell never calls it so);
     - `#[derive(Clone, Debug, Default, PartialEq, Eq)] pub enum OpenStep { #[default] Idle,
       Confirming(OpenRequest), Waiting(OpenRequest) }` and `pub fn busy(&self) -> bool` (not
       `Idle`). One request at a time; a new command while one waits replaces it (ADR 0019 §2.4;
       T202 does the replacing, the type only holds it).
  4. **The start** (`external.rs`): `#[derive(Clone, Debug, PartialEq, Eq)] pub enum StartError
     { Missing, Failed(String) }` and `pub fn start_program(request: &OpenRequest) -> Result<(),
     StartError>`: `Missing` when `request.program.path` is not a file (`is_file`, so a folder is
     missing too); otherwise one `std::process::Command::new(path).args(&request.images).spawn()`
     (every path its own argument, E-Q7); the child is dropped at once, never waited for or
     watched (on Windows that neither stops nor waits for it), written as
     `.spawn().map(|_child| ())` and never bound to a name, so clippy's `zombie_processes` does not
     fire (no `allow` or `expect` for it); a spawn error is
     `Failed(error.to_string())`. No other option is set (no working folder, no window flags).
     It does not check the length again; T202 calls it only with a planned request.
  5. **Texts** (`external_text.rs`, rule 13; whole sentences from one template each, singular and
     plural in the same function; as in `deletes_text.rs`, no full stop follows `{reason}` at the
     end of a text):
     - `open_in_text(name: &str)`: "Open in {name}" (the Image menu's item per program, E-Q3);
     - `open_in_another_text()`: "Open in Another Program…" (the item with an empty list);
     - `waiting_for_writes_text()`: "Waiting for writes …" (E-Q6);
     - `open_many_question_text(count: usize, name: &str)`: "Open {count} images in {name}?"
       (1: "Open 1 image in {name}?", never called so);
     - `open_button_text()`: "Open"; `open_cancel_text()`: "Cancel";
     - `opened_text(count: usize, name: &str)`: "1 image opened in {name}." / "{count} images
       opened in {name}." (the status line after the start; the ADR does not name a text, this
       is it);
     - `too_long_text(count: usize, name: &str, fit: usize)`, four whole templates in the same
       function: for `count` 2 or more and `fit` 2 or more "{count} images cannot be opened in
       {name} at once: the command line would be longer than Windows allows. At most {fit}
       images fit."; for `fit` 1 "{count} images cannot be opened in {name} at once: the command
       line would be longer than Windows allows. At most 1 image fits."; for `fit` 0 "{count}
       images cannot be opened in {name} at once: the command line would be longer than Windows
       allows. Not even one image fits."; and for `count` 1 (or 0), whatever `fit`, "The image
       cannot be opened in {name}: the command line would be longer than Windows allows.";
     - `start_failed_text(name: &str, reason: &str)`: "{name} could not be started: {reason}";
     - `program_missing_text(name: &str, path: &str)`: "{name} could not be started: {path} does
       not exist." (E-Q9; the path is not a system reason, so a full stop ends it);
     - `image_gone_text(name: &str)`: "{name} was not opened: it is no longer in its folder."
       (T202 point 8, an image moved or deleted while its request waited);
       `nothing_to_open_text()`: "Nothing was opened: the images are no longer in their
       folders.";
     - `program_number_text(number: usize)`: "Open in Program {number}" (T202's `command_text`
       for a command whose name is not at hand; not shown in the menu).
  6. **A write waiting for an image** (`metadata.rs`, only this): `pub(crate) fn write_pending(&self,
     path: &Path) -> bool` on `MetadataCache`: `is_pending(path)` (a rating write queued or
     sent without its answer) **or** the editor's write in flight is for `path`
     (`editor.writing`), **or** `path` is in the running "Apply to All Marked" batch's `queue` or
     `in_flight` (`EditorBatch::pending_paths()`, metadata.rs about line 272, already chains
     both). Paths are compared as `is_pending` compares them (the cache's own absolute
     paths, `==`). ADR 0019 §2.4: "a rating or metadata write of any of the images is waiting in
     the queue or running". Until T202 uses it, it carries `#[cfg_attr(not(test),
     allow(dead_code))]` with a comment naming T202, as T188 did.
  7. **Exports** (`lib.rs`): `mod external; mod external_text;` and `pub use` of everything public
     in them (`main.rs` is a separate binary crate and uses them in T202).
  8. **Glossary** (`docs/glossary.md`, §3 "Window, menus and commands", the 3-column form, "–" in
     the Swedish column as the newer rows): "Open in {name}", "Open in Another Program…",
     "Waiting for writes …", "Open" (the question's button; its other button is "Cancel", §6),
     and the sentence of `too_long_text` as "command line" (the program's arguments, Windows'
     own limit of 32 767 characters); each note naming ADR 0019 and T201. Name them in the
     report.
- **Tests to add:** `external.rs`: `command_line_length` for a program and no image (the quotes
  and the NUL), a path without spaces, a path with a space (quoted), a path with a non-ASCII
  letter (one unit) and one with a character outside the BMP (two units); `images_that_fit` with
  all fitting, with a list where exactly `n` fit (built from long paths so the edge is crossed),
  and 0 when the program path alone is too long; `plan_open`: 100 images give `Start`, 101 give
  `Ask`, a list too long gives `TooLong` with the right `count` and `fit` (also above 100);
  `OpenRequest::ready` false while one image waits and true when none does; `OpenStep::busy`;
  `start_program`: a path that does not exist and a folder give `Missing`, a file that is not a
  program (a `.txt` written to a temporary folder) gives `Failed` (no real program is started by
  a test). `external_text.rs`: each text with a count with 1 and with more, `too_long_text`'s
  four templates, each text without a count once. `metadata.rs`: `write_pending` true for a
  queued rating write, for the editor's write in flight and for an image queued and an image in
  flight in a batch, false for another image and after each has answered (with the existing
  `FakeSubmit` helpers, as `is_pending_and_current_rating_follow_the_rating_store`; a small
  `edits_capacity` keeps later batch writes in `queue`; the editor's and the batch's answers are
  given as the existing editor and batch tests give theirs, by the worker event they expect).
- **Known limits** (to `backlog.md` when this task is Done): the length follows `std`'s quoting
  as it is today; a later `std` that quotes otherwise is caught only by the start's error (the
  start then fails with Windows' reason). A `.bat` or `.cmd` set up as a program is measured as
  an `.exe` (`std` starts it through `cmd.exe` and may refuse some arguments); its start may
  then fail with `Failed`.
- **Not in this task:** the menu, Ctrl+E, the confirmation window, the wait and the start in the
  shell (T202); "Show in File Explorer" (T203). No change to `settings.rs` (T200's type is used as
  it is).
- **Rules:** `AGENTS.md` §3; `docs/tasks/README.md` rule 13 (the texts are made here; no drawing
  code in this task). No new dependency (`std::process::Command` only).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows no change from this
  task outside the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Open | Added (ADR 0019 E2, order 1c), with T202 and T203 as one chain. Cold read (fresh agent, 2026-11-19): the command line as `std` builds it (the program quoted with nothing escaped, a `"` in a path costs n + 1), the length counted with `encode_utf16` on every platform, `.bat`/`.cmd` as a known limit, the spawn written so clippy's `zombie_processes` does not fire, `too_long_text`'s four templates written out, `program_missing_text` ending in a full stop, `EditorBatch::pending_paths` named, how the tests answer the editor's and the batch's writes. Chain review (fresh agent, T200–T203, 2026-11-19): T202 waits also while a move or a delete is busy and leaves out images that are no longer files, so `image_gone_text` and `nothing_to_open_text` were added here; T202's question has a guard of its own; the wait without a time limit, quitting while a request waits and a non-UTF-8 path after Save are known limits of T202; T203 extends the menu test as T202 left it, notes that the preview has no menu bar, gets an independent code review, and its hand check covers "Apply to All Marked", an X with the subfolder setting, Ctrl+E with an empty list in the preview, and what is not checked by hand |
