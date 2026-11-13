# T53. ADR 0014 F3: one rating key on several images, and the batch failure text (plain Rust) — Done

- **Review 2026-11-10: accepted.** Commit together with T54.

- **Can start now.** Independent of T51, T52 and T54.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/ratings.rs`, `crates/pw-loupe/src/metadata.rs`,
  and `crates/pw-loupe/src/main.rs` **only** to use the new failure text (point 4). Not `pw-metadata`.
- **Why:** ADR 0014 §2.7, §4.3, Q6. `apply` toggles per image, so several targets need one decision
  for the whole batch.
- **What:**
  1. `RatingStore::set_many(&mut self, paths: &[PathBuf], key: RatingKey) -> Result<usize, RatingRefused>`:
     - Paths without `supports_rating` are skipped. If none is left → `Err(RatingRefused::Unsupported)`,
       nothing changes. Resolve every remaining path to absolute first; if one fails, return that error
       before anything changes.
     - Decide **once**, from the shown value of each ratable path (as `set` reads it): `Label(l)` →
       if all have `l`, clear the label on all, otherwise set `l` on all; `Reject` → if all have −1,
       clear the stars on all, otherwise set −1 on all; `Stars(n)` and `ClearLabel` → as `apply`. The
       other half of each rating is kept.
     - Then, for each path in the given order, the same as `set` with that explicit value: `shown` at
       once, an unsent queued write for the path is replaced, otherwise a new write is queued. Share
       this with `set` through a private helper rather than copying it.
     - Returns the number of images set.
  2. With one path, `set_many` gives the same result as `set` for every key (the toggle included).
  3. Failure text (Q6): `RatingStore` counts write failures since the last `set` or `set_many` and
     keeps the file name and error of the last one. `pub fn failure_text(&self) -> Option<String>`:
     none → `None`; one → today's text "Kunde inte spara betyget för {fil}: {fel}"; more →
     "Kunde inte spara betyget för {n} bilder. Senast: {fil}: {fel}". `set` and `set_many` reset the
     count. The strings returned by `pump`, `written` and `fail_sent` are unchanged (the exit list of
     T44 uses them).
  4. `MetadataCache`: `set_ratings(&mut self, paths: &[PathBuf], key) -> Result<usize, RatingRefused>`
     and `failure_text()`, passing through. `main.rs`: where the last message of `poll` is put in the
     status line, use `failure_text()` instead when `poll` returned a failure. No other change.
- **Rules:** as T42; rules 6, 7, 10–12. Existing tests are not changed.
- **Tests to add** (with the existing fake writer): the table of point 1 — label on a mix → set on all;
  label on all that have it → cleared on all; X on a mix → −1 on all; X on all rejected → cleared;
  `Stars(3)` and `ClearLabel` on several; the other half kept in each case; an unsupported file
  (`x.png`) among the targets is skipped; only unsupported → `Unsupported` and nothing queued; order of
  writes follows the paths; two batches before a `pump` coalesce per path; one failed image of a batch
  rolls back only that image; `set_many` with one path equals `set` for every key on a few starting
  values; `failure_text` for 0, 1 and 3 failures, and reset by the next key. The `app.js` case: five
  marked images (one unsupported), label Yellow → four writes with Yellow; Yellow again → four writes
  without a label.
- **Done when:** `.\tools\check.ps1` is all OK.
