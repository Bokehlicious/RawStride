# T40. ADR 0010 M7d: eject the card after the import — Done

- **Start after T39 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`, `crates/pw-loupe/src/import_worker.rs`,
  `crates/pw-loupe/src/import_text.rs`, `crates/pw-loupe/src/tabs.rs`, `crates/pw-loupe/src/main.rs`.
- **What (ADR 0010 §3.2 and the risk in §10; the Tauri `run`):**
  1. `ImportForm` gets `eject: bool` (default `false`).
  2. **Worker,** after the copy and the IPTC step, when `eject` is set:
     - if the result is cancelled or has failures: `ImportDone::eject = Some("Kortet matades inte ut
       eftersom importen inte blev komplett.")`;
     - otherwise send `ImportEvent::Ejecting { id }`, forget the cached scan, and set
       `eject = Some(pw_ingest::eject(&source))` (its message as it is).
  3. **Tabs:** `Tabs::clear_inside(&mut self, root: &Path) -> bool` empties every tab whose folder is
     the root or below it (compare paths component by component, case-insensitively on Windows, with no
     file access) and returns whether the active tab was among them.
  4. **Dialog (`main.rs`):** a checkbox "Mata ut kortet när importen är klar (inget raderas från kortet)".
     When a run starts with `eject` set, call `clear_inside(source)` first and reload the active tab if
     it was emptied, so no tab reads from the card while it is ejected (ADR 0010 §10). While ejecting,
     the progress text is "Matar ut {source} …". After `Done` with an `eject` message, ask the worker
     for `cards()` again so the ejected card disappears from the buttons.
  5. **Texts:** `done_lines` adds the eject message as a normal line after the IPTC errors.
- **Rules:** `pw_ingest::eject` runs only on the worker thread (it waits for Windows); rules 6 and 7.
- **Tests to add:** eject not requested → `eject` is `None`; requested with a cancelled or failed result
  → the "inte komplett" message and no eject attempt (use a run that is cancelled before it starts);
  `clear_inside` (the root itself, a subfolder, a folder with a similar name such as `E:\DCIM2` next to
  `E:\DCIM`, different case, the active tab returned); the dialog state and `done_lines` with an eject
  message. A real eject is checked by hand.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: an import with the
  checkbox ejects the card at the end and the message says so; a cancelled import with the checkbox
  does not eject and says why; a tab showing the card is emptied before the eject.
