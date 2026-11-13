# T44. ADR 0013 B4: finish pending rating writes on exit — Done

- **Start after T43 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/ratings.rs` or
  `metadata.rs` only for a plain, tested helper.
- **What (ADR 0013 Q6, R4):**
  1. When the program exits (`eframe::App::on_exit`, or the equivalent hook in eframe 0.37), keep
     calling `pump` and `poll` with a short sleep (about 20 ms) until `pending()` is 0 or **5 s** have
     passed. Put the loop in a plain helper with an injectable clock and sleep, so it can be tested.
  2. If writes are still pending, or some failed during the wait, show one message box listing the file
     names (at most 20, then "… och {n} till"), titled "Betyg som inte sparades", with
     `rfd::MessageDialog` (`rfd` is already a dependency of `pw-loupe`). If `MessageDialog` is not
     available with the pinned features, **stop and report** instead of adding a feature.
  3. If the window stays visible during the wait, say so in the report; the owner decides whether that
     is acceptable.
- **Rules:** as T42; no new dependencies and no feature changes.
- **Tests to add:** the helper returns at once when nothing is pending; ends when the last write is
  confirmed; gives up after 5 s with the pending files listed (fake clock); the list text with more
  than 20 files.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: press a rating key and
  close the window at once; reopen the folder and the value is there.
