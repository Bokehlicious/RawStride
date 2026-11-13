# T99. ADR 0007 I10f: RawStride's settings file (create IIM, fallback encoding) — Done

- **Can start now.** The owner approved D10 on 2026-11-12. Can run at the same time as T94 (other
  crate). T100 and T102 also add lines to `lib.rs`; if
  they run at the same time, each agent adds only its own `mod` line and its own `pub use` line.
- **Crates:** `pw-loupe` only. Nothing uses the file yet (T106).
- **Files:** new `crates/pw-loupe/src/settings.rs`; `crates/pw-loupe/src/lib.rs` (one `mod` line,
  one `pub use` line); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 (the setting "Skapa IPTC-IIM för kompatibilitet med äldre system", off by
  default) and §5.4/Q10 (the fallback encoding, Windows-1252 by default, changeable). RawStride
  has no settings file yet.
- **What** (`settings.rs`), as D10:
  1. `pub struct Settings { pub create_iim: bool, pub fallback: Fallback }`; `Default`: `false`
     and `Fallback::Windows1252`; `Settings::edit_options(&self) -> EditOptions`.
  2. `pub fn settings_path() -> Option<PathBuf>`: `%LOCALAPPDATA%\RawStride\settings.txt` (`None`
     without `LOCALAPPDATA`).
  3. `pub fn load_settings(path: &Path) -> (Settings, Vec<String>)`: a missing file gives the
     defaults and no problem. The file is UTF-8 text, one `key = value` per line; spaces around
     both are ignored; empty lines and lines starting with `#` are skipped. Keys:
     `create_iim` (`true`/`false`) and `fallback_encoding` (`windows-1252`/`iso-8859-1`). An
     unknown key, a bad value or a line without `=` keeps the default for it and gives one problem
     text in English naming the line number (for the log, `eprintln!` by the caller); a file that
     cannot be read gives the defaults and one problem.
  4. `pub fn save_settings(path: &Path, settings: &Settings) -> io::Result<()>`: creates the
     folder, writes both keys (with a first comment line `# RawStride`) to `settings.txt.tmp` in
     the same folder, then `std::fs::rename` onto `settings.txt` (replaces it on Windows), so a
     crash never leaves half a file.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change (no serde in `pw-loupe`).
- **Tests to add** (unit tests in `settings.rs`, temporary folders): defaults for a missing file;
  save then load gives the same settings for all four combinations; a file with comments, empty
  lines and spaces; an unknown key, a bad value and a line without `=` each give one problem with
  its line number and the defaults for that key while the other key is read; `save` into a folder
  that does not exist creates it; no `.tmp` file is left after `save`; `edit_options`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
