# T93. ADR 0007 I9g: the IIM checksum stays current when no XMP is left — Done

- **Can start now.** No dependency change. Can run at the same time as T91 (no shared files).
- **Crates:** `pw-metadata` only.
- **Files:** `crates/pw-metadata/src/write_plan.rs`; `crates/pw-metadata/tests/write_plan.rs`
  (new tests only); `crates/pw-metadata/tests/template.rs` (one new test only);
  `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 and §12 (the checksum is updated by every write that changes IIM). Found
  in T88: in a JPEG with IIM and a stored `IPTCDigest` whose only XMP value is City, a template
  `Clear` of City writes `-XMP-photoshop:City=` and `-IPTC:City=` but no `-IPTCDigest=new`
  (`plan` writes the checksum only when IIM **and XMP** exist after the write). The old checksum
  is then out of date, ExifTool's validation warns, and the copy check (T84) refuses the write
  (the original is untouched, but the Clear can never be saved). Measured by the reviewer with
  ExifTool 13.59 (see the step plan): with `-IPTCDigest=new` added the stored and the current
  checksum are equal and `Validate` is `OK`.
- **What:**
  1. **`write_plan.rs`, the checksum rule:** besides today's case (IIM and XMP both exist after
     the write), `-IPTCDigest=new` is written when the plan **writes IIM** and the file **has a
     stored checksum** (`state.digest` is `Matches` or `Differs`), even when no XMP is left. Q1 is
     unchanged: an unresolved (`Open`) conflict still gives `DigestNotWritten` (such conflicts
     only exist without a stored checksum, so the new case never meets one). A file without a
     stored checksum and without XMP after the write still gets none. Update the module comment
     (the checksum bullet) and the comment at the rule.
  2. Nothing else changes; for a TIFF or DNG the digest pass of T92 follows from the argument.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `safe_write.rs`,
  `verify.rs` or `template.rs`; do not change existing tests (the T88 test that gives the JPEG a
  keyword stays as it is).
- **Tests to add:**
  - unit tests in `write_plan.rs`: a JPEG with IIM, `digest` `Matches`, City the only value,
    City cleared → the plan holds `-XMP-photoshop:City=`, `-IPTC:City=` and `-IPTCDigest=new`;
    the same with `digest` `Differs` → also `-IPTCDigest=new`; with `digest` `Missing` and no XMP
    left → no `-IPTCDigest=new` and no `DigestNotWritten`; a TIFF with the same change → also
    `-IPTCDigest=new`; a plan that does not write IIM (only `xmp:Rating` in a file whose XMP
    is otherwise empty and whose IIM is in line) → unchanged from today;
  - `tests/write_plan.rs` (ExifTool required): the JPEG fixture given City in IIM and XMP and a
    checksum (`-IPTCDigest=new`), then the plan that clears City applied with `-o`: `IPTCDigest`
    equals `CurrentIPTCDigest` and `read_metadata` shows no City;
  - `tests/template.rs` (ExifTool required): the same JPEG (City the only XMP value, **no**
    keyword) and a template `Clear` of `photoshop:City` through `apply_template_safely` →
    `Written`, City gone from XMP and IIM, the checksum current.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
