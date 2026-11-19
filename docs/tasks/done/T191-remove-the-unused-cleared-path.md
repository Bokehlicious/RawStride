# T191. Remove the unused `Cleared` path (ADR 0007 Q2, changed 2026-11-18) — Done

- **Why:** the owner changed ADR 0007 Q2 on 2026-11-18 (revision 19, §6.4, §17.1): a field
  cleared in a manufacturer RAW file's sidecar is no longer marked; the camera's value fills it
  again like any absent property, and D1 (a RawStride XMP namespace for `Cleared`) is dropped.
  The reconciliation still has the never-used path for `Cleared`: `Readings::cleared` (always
  empty, `read.rs` never fills it), the guard in `Work::fill`, the notice
  `Notice::HiddenByUser`, and its text in the info panel. This task removes that path. What the
  program does does not change, since `cleared` is always empty today.
- **Crates:** `pw-metadata` and `pw-loupe` (one task, not split: removing the variant
  `Notice::HiddenByUser` from `pw-metadata` breaks the `match` in `pw-loupe` until both are
  changed). **Files:**
  - `crates/pw-metadata/src/reconcile.rs`
  - `crates/pw-metadata/src/read.rs`
  - `crates/pw-loupe/src/info_panel.rs`
  - `docs/test-inventory.md`
- **What:**
  1. `reconcile.rs`, module comment (lines 15–16): remove point 5 (**`Cleared`**). Renumber the
     following point (Date Created, line 17, becomes 5) and the step comment `// 6.` in
     `reconcile` (line 250) to match.
  2. `reconcile.rs`, `Readings` (lines 98–99): remove the field `cleared` and its doc comment.
  3. `reconcile.rs`, `Notice` (lines 113–119): remove the variant `HiddenByUser` and its doc
     comment.
  4. `reconcile.rs`, `Work` (lines 143–145): remove the fields `cleared` and `hidden`, and the
     lifetime `'a` of `struct Work<'a>`. Then write `Work` instead of `Work<'_>` in all six
     places that use it (otherwise error E0107): `impl Work<'_>` (line 148) and the
     `work: &mut Work<'_>` parameters of `reconcile_iim` (265), `reconcile_camera_xmp` (296),
     `reconcile_exif` (327), the function that starts at about line 362 (364) and
     `suggest_date_created` (399).
  5. `reconcile.rs`, `Work::fill` (doc comment lines 160–162, body to line 175): remove the guard
     for a cleared property so that `fill` only stores the value as `insert` does. Keep `fill` as
     a function (its callers stay unchanged); its doc comment says it fills an absent property
     from the RAW file or another secondary representation, without the sentence about cleared
     properties.
  6. `reconcile.rs`, `reconcile` (lines 202, 209–210 and 248): remove `cleared` from the
     destructuring of `Readings` and `cleared` and `hidden` from `Work`, and remove the comment
     line `// 5. \`Cleared\` (§6.4) is applied by \`Work::fill\` in steps 2–4.`
  7. `reconcile.rs`, tests: remove `cleared: BTreeSet::new()` from the helper `readings` (line
     620), and remove these five tests, which test only the removed path:
     - `a_cleared_property_is_not_filled_from_the_raw_file`
     - `a_cleared_property_hides_the_camera_xmp_value`
     - `a_cleared_property_is_not_filled_from_exif`
     - `a_sidecar_value_for_a_cleared_property_is_used`
     - `a_cleared_rating_and_a_camera_rating_0_give_no_hidden_notice`

     Removing these named tests is part of this task (README rule 5 does not apply to them). Do
     not remove or change any other test.
  8. `read.rs`: remove the module comment line 32 (`` `Cleared` (owner decision D1) is not read
     yet; `Readings::cleared` stays empty. ``) and the blank `//!` line 33 after it (lines 31 and
     33 are both blank, so one goes), and remove `cleared: BTreeSet::new(),` at lines 178 and
     954.
  9. `info_panel.rs`: in the module comment (line 9) remove "values hidden by the user (§6.4), ";
     in the doc comment of `notices` (line 375) remove "the values hidden by the user, "; remove
     the `out.extend(…)` block for `Notice::HiddenByUser` (lines 396–405). In the test
     `every_notice_text_in_order` remove the `Notice::HiddenByUser { … }` entry from `notices`
     (lines 1109–1113) and the expected line
     `"Creator: the camera's value Kamera Ägare is hidden by the user"` (line 1172); the rest of
     that test is unchanged.
  10. Remove the imports that become unused: `use std::collections::BTreeSet;` in `read.rs`
      (line 40; lines 178 and 954 are its only uses) and in `reconcile.rs` (line 23; no remaining
      code or test uses it). No import in `info_panel.rs` becomes unused. `.\tools\check.ps1`
      (clippy) must show no new warning.
  11. `docs/test-inventory.md`:
      - the `pw-metadata` unit-test row (line 22): remove the clause "`Cleared` (not filled from
        IIM, camera XMP or Exif, one `HiddenByUser` with the first value that would have filled, a
        sidecar value used), " and the words "no `HiddenByUser` for a cleared rating" in the T113
        part; add the new test of **Tests to add**; the count goes from 445 to 441;
      - the `pw-loupe` unit-test row (line 21): remove "hidden by the user, " from the
        description of `every_notice_text_in_order`; its count (936) stays;
      - the header (line 4): 1883 passed becomes 1879 (72 ignored unchanged), and "after T87–T190"
        becomes "after T87–T191".
- **Tests to add** (in `reconcile.rs`, in the `// --- RAW ---` section, where the removed tests
  were):
  - `a_raw_file_exif_fills_a_property_the_sidecar_lacks`: a `FileKind::Raw` reading whose
    sidecar XMP has no Creator and whose Exif has `artist: Some("A")` gives Creator `["A"]` with
    origin `Origin::Exif`, no conflict and no notice (ADR 0007 §6.4, Q2 as changed on
    2026-11-18). It passes on today's code too; it records the rule, not a new behaviour.
- **Not in this task:** the ADRs and other documents (the reviewer changed them in commit
  `e05718a`); any change to how a field is cleared or written (`edit.rs`, `write_plan.rs`); the
  word "cleared" elsewhere in the code (for example the import's cleared fields, D14), which has
  nothing to do with `Cleared`.
- **Rules:** `docs/tasks/README.md`; no change in behaviour beyond removing the unused path; no
  user-visible text is added.
- **Done when:** `.\tools\check.ps1` is all OK,
  `grep -rn "HiddenByUser\|cleared: BTreeSet\|Readings::cleared\|hidden by the user" crates docs/test-inventory.md`
  finds nothing, and `git diff --stat` shows only the files above and this task's file.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-18 | Open | Added (ADR 0007 revision 19, Part C of the Q2/D1 change) |
| 2026-11-18 | Open | Cold read: a fresh agent found that removing `'a` from `Work` needs `Work<'_>` changed in six places (point 4), that `test-inventory.md` line 21 and its header also change (point 11), and that the unused imports and the place of the new test were not named (point 10, Tests to add); all written in, with the line numbers it checked. No other file uses the removed items |
| 2026-11-18 | Ready for review | Done as written (points 1–11, the five named tests removed, `a_raw_file_exif_fills_a_property_the_sidecar_lacks` added). `.\tools\check.ps1` all OK: 1879 passed, 72 ignored, 0 failed; no clippy warning (only the usual `raw-decode-bench` build-script line). The `grep` of Done when finds nothing. No other file changed |
| 2026-11-18 | Done | Reviewed: all points done as written, `test-output.txt` newer than the last change (fmt, audit, test, clippy OK; 1879 passed, 72 ignored; no clippy warning), the Done-when grep finds nothing. Extra code review (two crates) by a fresh agent: no bug, nothing to fix; two optional nits not taken (`Work::fill` is now a pass-through to `insert`, kept as the task says; the new test's comment). No behaviour change, so no hand check. Nothing left over |
