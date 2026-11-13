# T73. ADR 0016 §5: the loupe says when an embedded colour profile cannot be used — Done

- **Can start now.** T72 is done. Independent of T74's text, but T74 describes it, so T73 first.
- **Crates:** `pw-colour`, `pw-viewcore`, `pw-loupe`.
- **Files:** `crates/pw-colour/src/lib.rs`, `crates/pw-colour/src/tests.rs`;
  `crates/pw-viewcore/src/backend.rs`, `crates/pw-viewcore/src/core.rs`,
  `crates/pw-viewcore/src/state.rs`, `crates/pw-viewcore/src/native.rs`; `crates/pw-loupe/src/lib.rs`;
  any test file that builds a `Frame` literal (today `crates/pw-loupe/src/lib.rs`; add
  `profile_unusable: false` and nothing else there); `docs/test-inventory.md`. No `Cargo.toml`
  change; `Cargo.lock` must not change.
- **Why:** ADR 0016 §3.1 ("Display") and §5 (refined 2026-11-11), GR9: the user must see when the
  colours rest on a fallback because the file's own profile was broken.
- **What:**
  1. **`pw-colour`:** `pub fn jpeg_source_profile_checked(icc: Option<&[u8]>, exif_adobe_rgb: bool)
     -> (Profile, bool)`: the same choice as `jpeg_source_profile`, plus `true` when an ICC profile
     was present but could not be used. `jpeg_source_profile` becomes a wrapper that drops the flag
     (its behaviour and tests unchanged).
  2. **`pw-viewcore` `Backend`:** a new method with a default,
     `fn profile_unusable(&self, _session: &Self::Session) -> bool { false }`, so the fake backends
     of the tests need no change.
  3. **Core and state:** after a successful open the worker asks
     `backend.profile_unusable(&session)` and the state keeps the keys for which it is true in a new
     `HashSet<Key>` next to `sizes`: cleared where `sizes` is cleared, moved by `set_files` exactly
     as `sizes` is moved. `Frame` gets `pub profile_unusable: bool` (true when the current key is in
     the set), documented like the other fields.
  4. **`native.rs`:** the flag per session. TIFF: `SourceColour::Unusable(_)` from
     `decode_rgb8_for_display` (keep it next to the image in `NativeSession::Tiff`). JPEG:
     `jpeg_source_profile_checked`, plus `true` when `NativeBackend::transform` had to fall back to
     sRGB because no transform could be built from the source profile. RAW: always `false`.
     `profile_unusable` returns the session's flag.
  5. **`pw-loupe` `status_line`:** when `frame.profile_unusable` and the source text is shown (not
     "kan inte visas", not "förbereder …"), insert the part
     **"Inbäddad färgprofil kunde inte användas"** right after the source text, with the usual
     `"  ·  "`. Nothing else in the line changes. The grid's status line is not changed.
- **Rules:** rules 3–7 and 10–12. Existing tests keep their assertions; the only allowed change to
  existing test code is the new field in `Frame` literals.
- **Tests to add:**
  - `pw-colour`: `jpeg_source_profile_checked`: no ICC → `false` (with and without the marker); a
    usable Adobe RGB ICC → `false`; a garbage ICC → `true` with sRGB, and with the marker `true` with
    Adobe RGB; a gray ICC → `true`.
  - `pw-viewcore` `native.rs`: a TIFF with a garbage ICC tag → `profile_unusable` true; with a
    ProPhoto ICC and without ICC → false; the fixture JPEG with a garbage APP2 ICC → true; without
    APP2 and with a usable one → false.
  - `pw-viewcore` state (with the simulated backend of the existing state or core tests; a fake
    that reports `true` for one path): the frame of that image has `profile_unusable` true and the
    others false; after `set_files` reorders the list the flag follows the path; after
    `open_folder` it is gone.
  - `pw-loupe`: `status_line` of a ready frame with the flag contains
    "TIFF-fil 6 × 4  ·  Inbäddad färgprofil kunde inte användas  ·  " (or the JPEG equivalent);
    without the flag the line equals today's; a `Preparing` frame without size and an `Error` frame
    with the flag do not show the part.
- **Check by hand (owner, after review):** a copy of a JPEG with its APP2 profile damaged (the
  reviewer provides one) shows the part in the loupe status line; the DxO and Sony files do not.
- **Done when:** `.\tools\check.ps1` is all OK; `git diff Cargo.lock` is empty;
  `docs/test-inventory.md` has the new tests and totals; `git diff --stat` shows only the files above.
