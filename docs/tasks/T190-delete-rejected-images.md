# T190. ADR 0018 B5: "Delete Rejected Images" in the active tab's folder (pw-loupe) — Blocked: waits for T189

- **Chain:** T185 (B1) → T186 (B2) → T187, T188 (B3) → T189 (B4) → T190 (B5). The owner's
  answers are B-Q1–B-Q15 in ADR 0018 §3; how the steps fit is ADR 0018 §2.10.
- **Why:** ADR 0018 §2.1, B-Q9: Image ▸ "Delete Rejected Images" deletes every image of the
  active tab's folder whose rating is reject, with the same confirmation and rules as Delete
  (T189). Only the targets are new.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/deletes.rs`,
  `crates/pw-loupe/src/deletes_text.rs`, `crates/pw-loupe/src/lib.rs` (exports only),
  `crates/pw-loupe/src/shell.rs` (one command), `crates/pw-loupe/src/shell_text.rs` (its text),
  `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`, and this task's own file (status and
  log).
- **What:**
  1. **Command** (`shell.rs`, `shell_text.rs`): `Command::DeleteRejected` "Delete Rejected
     Images", no key, in the Image menu right after `Command::DeleteToRecycleBin`; arms in
     `command_text` (`shell_text.rs`), `command_enabled` and `run_command` (both in `main.rs`;
     greyed while the active tab has no folder); `acts_in` gives `Where::Both` through its
     default arm. Extend the menu tests as T189 did.
  2. **The targets** (`deletes.rs`): `pub fn rejected_images(files: &[PathBuf], is_rejected:
     impl Fn(&Path) -> bool) -> Vec<PathBuf>`: the files for which `is_rejected` is true, in the
     list's order. `main.rs` gives it the active tab's **whole** list (`tab.files`, not only the
     visible files; a filter does not hide a rejected image from this command), resolved with
     `resolve_paths`, and `is_rejected` = `metadata.current_rating(path).stars == Some(-1)`.
  3. **Partners that are not rejected** (`deletes.rs`; **approved by the owner, 2026-11-17,
     ADR 0018 B-Q16**): by B-Q3 a delete takes an image's RAW+JPEG partner along. Here
     that would delete a partner the user did not reject. So `pub fn
     refuse_unrejected_partners(prepared: Vec<PreparedDelete>, is_rejected: impl Fn(&Path) ->
     bool, is_image: impl Fn(&Path) -> bool) -> Vec<PreparedDelete>`: an `Ok` group with another
     image file (`is_image`: the extension is in `SUPPORTED_EXTENSIONS`, without regard to case)
     that is not rejected becomes (a partner's rating is read through the active tab's own
     path for it: the tab file whose resolved path is `same_path` with the partner; a partner
     not in the tab's list counts as not rejected) `Err(DeleteError { file: the image, reason:
     DeleteRefusal::Failed(partner_not_rejected_text().to_owned()), left: vec![] })`, so
     `plan_delete` (T187) lists it under `refused` with that text; everything else is used as for
     Delete.
  4. **Starting it** (`main.rs`, `delete_rejected(&mut self)`): the same guards as
     `delete_images` (T189 point 3: a move busy, a delete busy, the texts as there); when the
     active tab's folder read failed (`metadata.read_error()` is `Some`), that error in the
     status line; when its ratings are otherwise not ready (`ratings_ready()` false),
     `ratings_not_ready_text()`; no rejected image, `no_rejected_text()`; in each case nothing
     else. Otherwise it starts as T189's Delete does (`Preparing`, `delete_folder`), and a
     `delete_rejected: bool` in `Loupe` beside the step (not a field of T187's `DeleteStep`,
     whose tests stay as they are) says it is this command; it is cleared whenever the step is
     `Idle` again and set false by a plain Delete. When the `Prepared` answer comes, T189's rule
     for a tab no longer active applies; then the images no longer rejected (a key during the
     prepare) are left out, point 3 is applied, then `plan_delete`. **While the batch runs**,
     an image whose reject was cleared since (a key during the delete) is not sent:
     `DeleteBatch::skip` (T187), as `send_moves` skips an X image no longer rejected. From there on everything is
     T189's: the window, the batch, the follow-up and the result.
  5. **Texts** (`deletes_text.rs`, rule 13): `no_rejected_text()`: "There are no rejected images
     in this folder."; `ratings_not_ready_text()`: "The ratings of this folder are still being
     read. Try again when they are shown."; `partner_not_rejected_text()`: "its RAW+JPEG partner
     is not rejected" (a reason, used inside `delete_refused_text`).
- **Tests to add:** `deletes.rs`: `rejected_images` keeps the order and leaves out the others;
  `refuse_unrejected_partners` refuses a rejected RAW whose JPG is not rejected, keeps a pair
  where both are, leaves a group without a partner as it is, and refuses a RAW whose JPG is not
  in the tab's list. `deletes_text.rs`: the three
  texts. `shell.rs`, `shell_text.rs`: the item's order and text. The wiring in `main.rs` has no
  unit test; the owner's hand check covers it.
- **Not in this task:** deleting rejected images of several folders or tabs at once.
- **Rules:** `AGENTS.md` §3; `docs/tasks/README.md` rule 13 ("Delete Rejected Images" is in the
  glossary from T187).
- **Hand check by the owner** (ADR 0018 §4, after B5): in a copy of R2's folder with three
  rejected images (one hidden by a filter) and others not, the command asks for 3 images and
  moves only them (with their companion files) to the Recycle Bin; a rejected RAW whose JPG is
  not rejected is named as refused in the window beside another image that can go (alone, it is
  named in the status line and no window opens); with no rejected image it says so; and the main
  use: with "Move Rejected and Picked Images to Subfolders" on, a tab open on `_rejected` deletes
  all of it, and the tab of the parent folder is unchanged.
- **Done when:** `.\tools\check.ps1` is all OK with no clippy warnings and `git diff --stat`
  shows no change from this task outside the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked: waits for T189 | Added for ADR 0018 B5, written with B2–B4 for the chain review. Point 3 (a rejected image whose partner is not rejected is refused) is the reviewer's proposal; the owner's answer goes here and into ADR 0018 |
| 2026-11-17 | Blocked: waits for T189 | Cold read: doable in 8 files (the limit). Changed: the arms' files; the step's flag kept in `Loupe`, not in `DeleteStep`; a folder read error shown as itself, not as "still being read"; a partner's rating read through the tab's path for it (`same_path`), one not in the list counts as not rejected; the images themselves checked again when the answer comes; an answer for a tab no longer active dropped; "RAW+JPEG pair" in the glossary from T187; the hand check covers `_rejected` and a pair refused alone. Point 3 still waits for the owner's answer |
| 2026-11-17 | Blocked: waits for T189 | Point 3 approved by the owner (ADR 0018 B-Q16, written into §2.1 and §3) |
| 2026-11-17 | Blocked: waits for T189 | Chain review (fresh agent, T185–T190 read together): the folder kept as T189's `delete_folder` (an `Option<PathBuf>`), the flag cleared at `Idle` and by a plain Delete; an image un-rejected while the batch runs is skipped, so the command never deletes an image the user no longer rejects |
