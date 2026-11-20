# T225. ADR 0031 RL1: the "About RawStride" window in a Help menu (pw-loupe) — Open

- **Why:** ADR 0031 (accepted by the owner 2026-11-20), step RL1 and RL-Q9: a new Help menu with
  "About RawStride", a window showing the version, the commit the program was built from, and the
  licence notice GPL-3.0 §0 calls "Appropriate Legal Notices" (copyright, no warranty, where the
  licence and the source are, where the third-party notices are). It closes ADR 0002 §4's line
  "The licence notice in the program" and the backlog's item of the same name. Today nothing in
  the program reads or shows the version (`Cargo.toml` `[workspace.package]` has `0.1.0`). The
  Help menu is a fifth menu after ADR 0017 U1-Q1's four (File, Edit, Image, View); RL-Q9 decided
  it.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/build.rs` (point 1),
  `crates/pw-loupe/src/shell.rs` (points 2–3), `crates/pw-loupe/src/shell_text.rs` (point 4),
  `crates/pw-loupe/src/main.rs` (points 5–6), `crates/pw-loupe/src/lib.rs` (only if an export
  is needed for point 2), `docs/glossary.md` (point 8), `docs/test-inventory.md` (point 9), and
  this task's own file (status and log).
- **What:**
  1. **The commit at build time** (`build.rs`): keep everything the file does today (the manifest,
     `rerun-if-changed=build.rs`) and add, with a line in the file's doc comment naming ADR 0031
     RL-Q9 and T225:
     - run `git rev-parse --verify -q HEAD` with `std::process::Command::output()` (so `git`'s
       stderr is captured, not printed) and the current directory set to `CARGO_MANIFEST_DIR`;
       trim the output. If `git` cannot be started, exits with an error, or the trimmed output is
       not 40 or 64 characters of `0-9a-f`, the commit is the text `unknown` (a build from
       GitHub's source archive has no `.git`, and must still build). Otherwise the commit is its
       **first 12 characters** (`--short` is not used: it prints more than asked when a prefix is
       ambiguous);
     - pass it to the compiler with `cargo:rustc-env=RAWSTRIDE_COMMIT=<commit>` (on every
       platform, not only Windows/MSVC). The literal `unknown` in `build.rs` gets a comment
       naming `shell::UNKNOWN_COMMIT` (point 2), which must be the same text;
     - so a new commit or checkout rebuilds the text, print `cargo:rerun-if-changed=<path>` for
       each of these, **only if the file exists** (a path that does not exist makes Cargo run the
       script on every build): the file `git rev-parse --git-path HEAD` names; the file
       `git rev-parse --git-path logs/HEAD` names (Git appends to it on every commit and checkout,
       which also covers a branch whose ref lives only in `packed-refs`); the file
       `git rev-parse --git-path <ref>` names, where `<ref>` is what `git symbolic-ref -q HEAD`
       prints (nothing for a detached HEAD); and `git rev-parse --git-path packed-refs`. A
       relative path that `git` prints is relative to `CARGO_MANIFEST_DIR`; join it to it. These
       commands work in a linked worktree (`.git` there is a file), which is how this repository
       is checked out, and in CI's depth-1 checkout;
     - no dirty-tree marker, no date, no new dependency;
     - a failure of any of these `git` calls never fails the build and prints no `cargo:warning`.
  2. **The build facts** (`shell.rs`), with doc comments naming ADR 0031 RL-Q9:
     `pub const VERSION: &str = env!("CARGO_PKG_VERSION");`,
     `pub const COMMIT: &str = env!("RAWSTRIDE_COMMIT");`,
     `pub const UNKNOWN_COMMIT: &str = "unknown";` (the text `build.rs` sets when there is no
     commit), `pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");` and
     `pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");` (the last two come from
     `[workspace.package]`; nothing is typed by hand, so no name or address is written into the
     source).
  3. **The menu** (`shell.rs`): a new `MenuKind::Help` and a new `Command::About`, with doc
     comments naming ADR 0031 RL-Q9. `MENUS` gets a fifth menu, last, after View:
     `MenuKind::Help` with one entry, `item(Command::About, None)` (no key). The exhaustive
     matches that need an arm are `run_command` (`main.rs`), `command_text` and `menu_text`
     (`shell_text.rs`). `acts_in` (`shell.rs`), `command_enabled` and `command_checked`
     (`main.rs`) need none: their `_` arms already make `About` act in both views, enabled, and a
     plain item.
  4. **The texts** (`shell_text.rs`, rule 13; whole sentences, one template each):
     - `menu_text(MenuKind::Help)` → `"Help"`; `command_text(Command::About)` →
       `"About RawStride…"`;
     - a new `pub enum AboutText<'a>` and `pub fn about_text(text: AboutText<'_>) -> String`
       (a `String`, unlike `settings_text`'s `&'static str`, since some variants carry values),
       beside `SettingsText` / `settings_text`, with a doc comment naming ADR 0031 RL-Q9 and
       GPL-3.0 §0. Variants and their exact texts:
       - `Title` → `"About RawStride"`;
       - `Version { version: &'a str }` → `"Version {version}"`;
       - `Commit { commit: &'a str }` → `"Built from commit {commit}"`, and for
         `UNKNOWN_COMMIT` → `"Built from an unknown commit"` (decided inside the function);
       - `Copyright { authors: &'a str }` → `"Copyright © 2026 {authors}"`, where `authors` is
         `AUTHORS` with Cargo's `:` separator shown as `", "` (done inside the function);
       - `Licence` → `"RawStride is free software: you can redistribute it and/or modify it under
         the terms of the GNU General Public License as published by the Free Software
         Foundation, either version 3 of the License, or (at your option) any later version."`;
       - `Warranty` → `"RawStride comes with ABSOLUTELY NO WARRANTY, to the extent permitted by
         law. See the GNU General Public License for details."`;
       - `LicenceFile` → `"The licence is in the file LICENSE in the program folder, and at
         https://www.gnu.org/licenses/gpl-3.0.html."`;
       - `Source { repository: &'a str }` → `"The source code is at {repository}."`;
       - `ThirdParty` → `"RawStride includes software by others under their own licences; their
         notices are in the file THIRD-PARTY-NOTICES.txt in the program folder."`;
       - `Close` → `"Close"`.

       (`THIRD-PARTY-NOTICES.txt` is made by RL3 and put in the program folder by RL4; until then
       the text names a file a development build does not have. That is intended.)
  5. **Opening the window** (`main.rs`): a new field `about_window: bool` on the app, `false` in
     the app's constructor (where `settings_window: None` is set). `Command::About` sets it to
     `true` in `run_command`, beside `Command::Settings`. Like every command, it does nothing
     while `dialog_open()` is true (Settings, Metadata, Import and the other dialogs open), and the
     menu item still looks enabled then; that is intended. `about_window` is **not** added to
     `dialog_open` / `other_dialog_open`: the About window blocks nothing, and Settings may open
     over it.
  6. **The window** (`main.rs`): a new `fn draw_about_window(&mut self, ctx: &egui::Context)`,
     called right after the call of `draw_settings_window` among the other `draw_*_window`
     calls, drawing nothing while `about_window` is `false`. An `egui::Window` titled
     `about_text(AboutText::Title)`, not collapsible, not resizable, **without** `.open(&mut …)`
     (so no ✕ in the title bar), with `default_width` and `max_width` set to its own constant
     `ABOUT_WINDOW_WIDTH` of 440.0 points inside `draw_about_window` (as `SETTINGS_WINDOW_WIDTH`
     is inside `draw_settings_window`), so the long sentences wrap. Its rows, in this order:
     `Version` (`VERSION`), `Commit` (`COMMIT`), a space, `Copyright` (`AUTHORS`), `Licence`,
     `Warranty`, `LicenceFile`, `Source` (`REPOSITORY`), `ThirdParty`, a space, and a `Close`
     button that sets `about_window` to `false`. Each text row is drawn with
     `ui.add(egui::Label::new(text).wrap())`, as elsewhere in `main.rs` (selectable and copyable
     by default; the project never turns `selectable_labels` off) — **not** `ui.selectable_label`,
     which is a toggle button. No links, nothing opened, nothing read from disk or the network.
     Only `Close` closes the window: the settings window has no Escape handling either, and
     Escape in the contact sheet already clears the marking, so the window takes no key.
  7. **Tests** (see below).
  8. **Glossary** (`docs/glossary.md`, §3 "Window, menus and commands"): one new row after the
     row "Öppna mapp… / … / Avsluta": Swedish `–`, English `Help; About RawStride…`, Note "the
     fifth menu and its one item, which opens the window \"About RawStride\" (ADR 0031 RL-Q9);
     added by T225". The row of the four menu titles is not changed: it lists the Swedish names
     of ADR 0017's four, and Help has no Swedish name.
  9. **Test inventory** (`docs/test-inventory.md`): pw-loupe's test count, and on line 20 the
     words "the menus File, Edit, Image and View" become "the menus File, Edit, Image, View and
     Help". Line 20 is one very long line: never read the file whole; change it by a script
     (`CLAUDE.md` §3–§4, keeping its line endings).
- **Tests to add:**
  - `shell.rs`: the existing test `the_menus_are_arkiv_redigera_bild_visa_and_every_command_is_listed_once`
    is changed **by this task, and only so**: the expected list gains `MenuKind::Help` last, and
    its name says the five menus (`..._bild_visa_hjalp_...`); everything else in it unchanged. A
    new test: the Help menu is last and holds exactly `Command::About` with no key.
  - `shell_text.rs`, existing tests changed **by this task, and only so**:
    `every_menu_item_text_in_menu_order` gains `"About RawStride…"` last;
    `every_menu_key_text_in_menu_order` gains `None` last; `every_fixed_text_of_the_shell` gains
    `assert_eq!(menu_text(MenuKind::Help), "Help");` after the View line. Nothing else in them
    changes.
  - `shell_text.rs`, new tests for `about_text`: every variant's exact text with sample values
    (not the real `AUTHORS`; no test compares against the author's name); `Commit` with a
    12-character hash and with `UNKNOWN_COMMIT`; `Copyright` with one author and with two
    (`"A:B"` → `"A, B"`).
  - `shell.rs`: `COMMIT` is `UNKNOWN_COMMIT` or exactly 12 characters of `0-9a-f`; `VERSION`
    equals `env!("CARGO_PKG_VERSION")` and is not empty; `REPOSITORY` starts with `https://`;
    `AUTHORS` is not empty.
  - The test `every_menu_key_gives_its_items_command` (`lib.rs`) must still pass unchanged
    (`About` has no key).
- **Not in this task:** the third-party notices file itself (RL3), the version resource and icon
  of the exe (RL2), a dirty-tree marker or build date, links that open a browser, a "Check for
  updates" (RL-Q10: none), any change to ADR 0017 (the reviewer notes the fifth menu there at
  Done), `Cargo.toml`, CI.
- **Rules:** `docs/tasks/README.md` rules 1–13; rule 13 in full (no string literal in the window's
  drawing code). No new dependency. The personal-data step of `check.ps1` must stay OK: no name,
  address or e-mail typed into any file.
- **Hand check (the owner, after Done):** Help ▸ About RawStride… opens the window in the
  contact sheet, in the loupe, and in an empty window with no tab open; it shows `Version 0.1.0`
  and a commit equal to the first 12 characters of `git rev-parse HEAD` at the time of the build
  (the hand check runs before the reviewer's commit, so this is the commit before T225's, which
  is right); the © sign shows correctly; the texts wrap inside the window; Close closes it; text
  can be selected and copied.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Written by the reviewer from ADR 0031 RL1 / RL-Q9 and ADR 0002 §4. Cold read (fresh agent, 2026-11-20): can be done with the named files. Fixed in the text: the commit read with `rev-parse --verify` and cut to 12 (`--short=12` can print more), `logs/HEAD` also watched (a ref only in `packed-refs` was missed), `output()` and trimming; `UNKNOWN_COMMIT` shared; the exhaustive matches named (`run_command`, `command_text`, `menu_text`), the `_` arms of `acts_in`, `command_enabled`, `command_checked` need none; `every_menu_key_text_in_menu_order` and `every_fixed_text_of_the_shell` named as tests this task changes; the app's constructor (not a test one); the window's own width constant, no ✕, `Label::new(text).wrap()` instead of `selectable_label`; `AboutText<'a>` returning `String`; About kept out of `dialog_open`, its being ignored while a dialog is open stated as intended; `test-inventory.md` line 20 and the count as point 9; the hand check gained the empty window, the © sign and which commit it shows |
