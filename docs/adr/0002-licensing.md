# ADR 0002: Licensing and distribution

- Status: **accepted by the owner (2026-11-13)** for RawStride's own licence, LibRaw's licence
  option and shipping ExifTool; how ExifTool is shipped, the notices file and the trademark search are still open (§4).
- Date: 2026-11-03 (opened), 2026-11-13 (decided)

## 1. Context

- **LibRaw** is dual-licensed under LGPL-2.1 or CDDL-1.0. It is linked statically (vcpkg triplet
  `x64-windows-static-md`, `docs/libraw-build.md`), together with LittleCMS 2 (MIT), JasPer
  (JasPer License 2.0), libjpeg-turbo (IJG, BSD-3-Clause, zlib) and zlib.
- **ExifTool** uses the same terms as Perl (Artistic License or GPL). RawStride runs it as a
  separate, unmodified program, and today it is not shipped (`docs/distribution.md`).
- **Rust dependencies:** a `cargo metadata` survey on 2026-11-13 (616 packages, all platforms)
  found only permissive licences (MIT, Apache-2.0, BSD, ISC, Zlib, BSL-1.0, Unicode-3.0, IJG, CC0,
  Unlicense) and a few with MPL-2.0 (`cssparser`, `selectors` and others, through the Tauri app);
  packages with a choice that includes the GPL (`self_cell`, `r-efi`) also offer a permissive
  licence. egui's bundled fonts carry OFL-1.1 and the Ubuntu Font Licence (GUI_REQUIREMENTS §6).
- The code is written by the owner with coding agents; git records one author. Before a release
  it was asked whether RawStride should be open source, and under which licence.

## 2. Decision

1. **RawStride is licensed under the GNU General Public License, version 3 or later
   (`GPL-3.0-or-later`).** (Owner, 2026-11-13.) The full text is in `LICENSE` at the repository
   root. Anyone may use, study, change and share RawStride; whoever distributes a changed version
   must share its source under the same licence, so no closed copy can be made of it.
   - Options weighed: `MIT OR Apache-2.0` (permissive; allows closed copies), `MPL-2.0` (file-level
     copyleft), and waiting / closed source. The GPL was chosen; comparable programs (darktable,
     RawTherapee) use it as well.
   - The owner, as the only copyright holder so far, can still offer RawStride under other terms
     later. Contributions from others would make that need their consent (for example a
     contributor agreement); that is decided when the first outside contribution comes.
2. **LibRaw is used under LGPL-2.1.** CDDL-1.0 is not compatible with the GPL, so it cannot be the
   option. The LGPL-2.1 allows the combination (it may be upgraded to the GPL), and its relinking
   condition for static linking is met because RawStride's complete source and build instructions
   are public. The release names LibRaw's version and where its source is.
3. **Every dependency found is compatible with `GPL-3.0-or-later`**: the permissive licences and
   MPL-2.0 can be combined with it; for dual-licensed packages the non-GPL-2.0-only option is used.
   The Microsoft Visual C++ and OpenMP runtimes (`VCRUNTIME140.dll`, `VCOMP140.DLL`, …) are system
   libraries in the GPL's sense and are shipped under the Visual Studio redistribution terms.
   ExifTool is a separate program and is not affected.
4. **A new dependency** keeps needing the owner's approval (`AGENTS.md` §3); its licence must be
   compatible with `GPL-3.0-or-later`.
5. **ExifTool ships with RawStride** (owner, 2026-11-13). Reasons: without ExifTool nothing can be
   saved (ratings, labels, the import template, the metadata window), installing it on PATH is the
   hardest step for a user, and the safe write (ADR 0007 §9) was measured with one ExifTool
   version and works around one of its faults (T92), so a known version should run.
   - The Windows package from exiftool.org (`exiftool.exe` and its `exiftool_files` folder, which
     holds a Perl) is shipped **unmodified** in the program folder, under its own licence (Perl's:
     Artistic License or GPL). It stays a separate program, so it does not affect RawStride's
     licence. Its licence texts and the place of its source (exiftool.org) go into
     `THIRD-PARTY-NOTICES.txt`.
   - The shipped version is one the real-file tests of `pw-metadata` and `pw-imagecheck` passed
     with (`--ignored`, `PW_TESTDATA`); today **13.59**. A newer version is shipped only after those
     tests pass with it.
   - RawStride looks for `exiftool.exe` in its own program folder first and then on PATH, so a
     developer build and a user's own installation still work (T120).

## 3. Consequences

- `LICENSE` (the GPL-3.0 text) is added at the repository root. Each crate's `Cargo.toml` gets
  `license = "GPL-3.0-or-later"` through `[workspace.package]` (the commented "TBD" line is
  replaced).
- A release ships `LICENSE` and a `THIRD-PARTY-NOTICES.txt` with every dependency's notice (§4).
- RawStride stays clean-room (`docs/clean-room.md`): the licence does not change what may be
  studied or used from other products.

## 4. Still open

- [ ] A RawStride distribution folder (there is none today; `tools/package-dist.ps1`, which packed only
  the Tauri app, was removed with it on 2026-11-19, T208; `docs/distribution.md` keeps its findings):
  the exe, the runtime DLLs, ExifTool (§2.5), `LICENSE` and the notices.
- [ ] `THIRD-PARTY-NOTICES.txt` before the first release: the Rust packages (from `cargo
  metadata`), LibRaw, LittleCMS, JasPer, libjpeg-turbo, zlib, the fonts, the MSVC runtime, and
  ExifTool with its Perl.
- [ ] The licence notice in the program (GPL-3.0 §0, "Appropriate Legal Notices"): an "About
  RawStride" window with the version, the commit, copyright, no warranty, where the licence and
  the source are, and the third-party notices (owner, 2026-11-20, ADR 0031 RL-Q9, step RL1).
- [ ] A proper trademark search for the name RawStride before a public release (ADR 0012).
- [ ] Source headers (`SPDX-License-Identifier: GPL-3.0-or-later`) in the source files: optional;
  decide before the release.

## Revision history

| Revision | Date | Changes |
|---|---|---|
| 1 | 2026-11-03 | Opened: LibRaw and ExifTool licences noted; RawStride's licence pending |
| 2 | 2026-11-13 | Owner decision: `GPL-3.0-or-later`; LibRaw under LGPL-2.1; ExifTool ships with RawStride (§2.5); dependency survey; open points listed |
