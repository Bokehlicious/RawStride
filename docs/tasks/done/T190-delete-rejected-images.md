# T190. ADR 0018 B5: "Delete Rejected Images" in the active tab's folder (pw-loupe) — Done

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
     default arm. Extend the menu tests as T189 did: `shell_text.rs`'s
     `every_menu_item_text_in_menu_order` and `every_menu_key_text_in_menu_order` get the new
     item; `shell.rs`'s `delete_to_recycle_bin_is_the_last_item_of_the_image_menu` (which asserts
     that the Image menu ends with ToggleMode, Separator, DeleteToRecycleBin) is **changed by this
     task** (README rule 5 allows it here): renamed to
     `the_delete_items_are_the_last_items_of_the_image_menu`, it asserts that the menu ends with
     ToggleMode, Separator, DeleteToRecycleBin, DeleteRejected, with the same checks as now.
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
     DeleteRefusal::Failed(partner_not_rejected_text().to_owned()), left: vec![], deleted:
     vec![] })`, so
     `plan_delete` (T187) lists it under `refused` with that text; everything else is used as for
     Delete. "Another image file" is a file of `group.files` after the first (the first is the
     image itself, `DeleteGroup`'s order) for which `is_image` is true. The lookup through the
     tab's list lives in `main.rs`'s `is_rejected` closure (the tab's files resolved with
     `resolve_paths`, compared with `same_path`, the rating read with the tab's own path), not in
     `deletes.rs`, which only calls the closure; `is_image` is `main.rs`'s `is_image_file`.
  4. **Starting it** (`main.rs`, `delete_rejected(&mut self)`): the same guards as
     `delete_images` (T189 point 3: a move busy, a delete busy, the texts as there); when the
     active tab's folder read failed (`metadata.read_error()` is `Some`), that error in the
     status line; when its ratings are otherwise not ready (`ratings_ready()` false),
     `ratings_not_ready_text()`; no rejected image, `no_rejected_text()`; in each case nothing
     else. Otherwise it starts as T189's Delete does (`Preparing`, `delete_folder`), and a
     `delete_rejected: bool` in `Loupe` beside the step (not a field of T187's `DeleteStep`,
     whose tests stay as they are) says it is this command; it is cleared whenever the step is
     `Idle` again (in `delete_idle`, which every way back to `Idle` goes through), set false by a
     plain Delete, and set false in `Loupe`'s initialiser (about line 8261). When the `Prepared` answer comes, T189's rule
     for a tab no longer active applies; then the images no longer rejected (a key during the
     prepare) are left out (when none is left: `no_rejected_text()` in the status line and
     `delete_idle()`), point 3 is applied, then `plan_delete`. **While the batch runs**,
     an image whose reject was cleared since (a key during the delete) is not sent:
     `DeleteBatch::skip` (T187), as `send_moves` skips an X image no longer rejected. From there on everything is
     T189's: the window, the batch, the follow-up and the result.
  5. **Texts** (`deletes_text.rs`, rule 13): `no_rejected_text()`: "There are no rejected images
     in this folder."; `ratings_not_ready_text()`: "The ratings of this folder are still being
     read. Try again when they are shown."; `partner_not_rejected_text()`: "its RAW+JPEG partner
     is not rejected" (a reason, used inside `delete_refused_text`).
  6. **The partners while the batch runs** (added after the review, 2026-11-18; B-Q16):
     when both files of a pair are rejected, `plan_delete` keeps only the first as a batch
     image and the other goes along in its group, so the check of point 4 on the batch's own
     image alone does not protect the partner. In `deletes.rs`, `pub fn image_partners(files:
     &[PathBuf], is_image: impl Fn(&Path) -> bool) -> Vec<PathBuf>`: the files after the first
     for which `is_image` is true (used by `refuse_unrejected_partners` too, so the rule is in
     one place). In `main.rs`, when the confirmed plan becomes `Running` for Delete Rejected
     Images, keep in `Loupe` (beside `delete_rejected`, cleared in `delete_idle`) each batch
     image's partners as the tab's resolved paths (the tab file that is `same_path` with the
     partner, from the `Ok` groups of the prepare's answers). In `follow_deletes`' `Running`
     arm, for Delete Rejected Images: an image is not sent while it **or one of its partners**
     has a rating write pending (`is_pending`; the `to_send` closure), and it is skipped
     (`DeleteBatch::skip`) when it or one of its partners is no longer rejected
     (`current_rating(..).stars != Some(-1)`; a failed write rolled back counts as not
     rejected). A plain Delete is unchanged.
- **Tests to add:** `deletes.rs`: `rejected_images` keeps the order and leaves out the others;
  `refuse_unrejected_partners` refuses a rejected RAW whose JPG is not rejected, keeps a pair
  where both are, leaves a group without a partner as it is, and refuses a RAW whose JPG is not
  in the tab's list. `deletes_text.rs`: the three
  texts; `image_partners` (the image itself, a sidecar and a TIFF of the same name).
  `shell.rs`, `shell_text.rs`: the item's order and text. The wiring in `main.rs` has no
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
| 2026-11-18 | Blocked: waits for T189 | T189 is Done, hand-checked and approved (`0ac3022`); the backlog sorted after B4 (`cd39f01`). Names checked against the code after T189, all present; line numbers today (about): `main.rs` the `delete_folder` field 776, `delete_tab_active` 3087, `send_moves`' skip of an X image no longer rejected 4205, `delete_images` 4465, `delete_idle` 4491, `follow_deletes` 4503 (its `Running` arm, where the deletes are sent, 4543), `delete_prepared` 4592, `command_enabled` 4775, `run_command` 4847, `is_image_file` 6187, `resolve_paths` 6203; `shell.rs` `Command` 438, the Image menu's `DeleteToRecycleBin` 702, the menu tests 1026–1051; `shell_text.rs` `command_text` 305; `deletes.rs` `plan_delete` 54, `DeleteBatch::skip` 170; `lib.rs` the `deletes` exports 49–53. T189 added `delete_tab_active` and `delete_idle`: "T189's rule for a tab no longer active" in point 4 is `delete_tab_active`, and "the step is `Idle` again" is `delete_idle` |
| 2026-11-18 | Open | Cold read (fresh agent, after T189): doable in the named files, line numbers right. Changed: the Image menu test that T189 wrote is changed by this task (named, rule 5); the two menu text lists named; `deleted: vec![]` in the `DeleteError`; "another image file" defined as `group.files` after the first; the partner lookup lives in `main.rs`'s closure; the flag cleared in `delete_idle` and initialised in `Loupe`; none left after the prepare → `no_rejected_text()`; a partner un-rejected while the batch runs named as a known limit. Set Open |
| 2026-11-18 | Ready for review | Task agent: points 1–5 done in the named files. `Command::DeleteRejected` after Delete to Recycle Bin (no key, greyed without a folder); `rejected_images` and `refuse_unrejected_partners` in `deletes.rs`; the three texts in `deletes_text.rs`; in `main.rs` `delete_rejected` (the guards, then the read error as `EditorText::MetadataError`, then `ratings_not_ready_text`, then `no_rejected_text`), the `delete_rejected` flag (set false in the initialiser, by a plain Delete and in `delete_idle`), `still_rejected` used by `delete_prepared` (the images no longer rejected left out, then point 3, the lookup through the tab's resolved files with `same_path`), and the skip in `follow_deletes`' `Running` arm (`current_rating` of the batch's image, which is the tab's resolved path, as `send_moves` does). The Image menu test renamed and extended as point 1 says. 6 new tests (5 `deletes.rs`, 1 `deletes_text.rs`); `.\tools\check.ps1` all OK, 1882 passed, 72 ignored, 0 failed, no clippy warnings |
| 2026-11-18 | Open | Sent back after review (owner). Reviewer: points 1–5 done as written; `.\tools\check.ps1` all OK (1882 passed, 72 ignored, 0 failed; no clippy warnings by the `^warning` grep). Independent code review (fresh agent): no other finding (path keys, the partner check, a pair both rejected, the flag's lifetime, a tab no longer active, §3 all right). **Left to do: point 6 (new)**: the partner of a pair where both are rejected goes along in its image's group, and only the batch's own image is checked while the batch runs, so a partner whose reject is cleared by a key, or whose reject write fails and is rolled back, after the prepare is still deleted (against B-Q16). The known limit that the brief's "Not in this task" named for it is removed: the owner chose to fix it here. The uncommitted changes stay; add point 6 to them. Also noted, not for this task (inherited from T189): the worker gathers the group again at delete time, so a same-name image file that appears after the confirmation goes along unseen; to the backlog when this task is Done |
| 2026-11-18 | Ready for review | Task agent: point 6 added to the earlier changes. `deletes.rs`: `image_partners` (also used by `refuse_unrejected_partners`) and its test. `main.rs`: `delete_partners` in `Loupe` (each kept image's partners as the tab's resolved paths; initialised empty, cleared in `delete_idle`), filled by `still_rejected` from the `Ok` groups after point 3 and stored by `delete_prepared`, since the plan the confirmation holds no longer has the groups (so it is kept from the prepare's answer through `Confirming`, not made when the step becomes `Running`; the content is the same); in `follow_deletes`' `Running` arm, for Delete Rejected Images, `to_send` also waits while a partner `is_pending`, and an image is skipped when it or a partner is no longer rejected; a plain Delete unchanged. A partner not found in the tab's list (not possible after point 3) is kept as the worker gave it, so it counts as not rejected. `lib.rs` exports `image_partners`; `docs/test-inventory.md` counts. `.\tools\check.ps1` all OK, 1883 passed, 72 ignored, 0 failed, no clippy warnings |
| 2026-11-18 | Done | Reviewed again (reviewer): point 6 done in `deletes.rs` (`image_partners`, also used by `refuse_unrejected_partners`) and `main.rs` (`delete_partners` kept from the prepare's answer, cleared in `delete_idle`; while the batch runs an image waits while it or a partner has a rating write pending and is skipped when it or a partner is no longer rejected); points 1–5 as at the first review. `.\tools\check.ps1` all OK (1883 passed, 72 ignored, 0 failed; no clippy warnings by the `^warning` grep); `test-output.txt` newer than the changes. Independent code review (fresh agent, twice): the first found the partner unchecked while the batch runs (point 6, added and sent back); the second found no correctness bug. Leftovers, to `backlog.md` ("Known limits of the delete to the Recycle Bin"): a partner un-rejected after its image's delete was sent still goes (the check is in the shell, the rating write is queued behind the delete); the worker gathers the group again at delete time, so a same-name image file that appears after the confirmation goes along unseen (from T189); a tab closed during the batch makes its remaining images skipped silently (safe; the count is lower with no reason). Awaiting the owner's hand check (ADR 0018 §4, after B5) |
| 2026-11-18 | Done | Owner's hand check: all OK. Image ▸ Delete Rejected Images asks for the rejected images of the tab's whole list (one hidden by a filter included) and moves only them, with their companion files, to the Recycle Bin; a rejected RAW whose JPG is not rejected is named as refused beside another image that goes (alone: the status line, no window); with no rejected image it says so; and the main use, "Move Rejected and Picked Images to Subfolders" then the command in a tab on `_rejected`, deletes all of it with the parent folder's tab unchanged. A network folder not reported (the backlog item stays). Approved |
