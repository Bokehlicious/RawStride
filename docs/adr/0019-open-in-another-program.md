# ADR 0019: Opening images in another program

- Status: **accepted by the owner (2026-11-17):** every question answered, all proposals approved
  (§3). Nothing is built, installed or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit.
- **Addition 1 accepted by the owner (2026-11-19):** a right-click menu on images (§2.9,
  E-Q15–E-Q19, step E4), the owner's wish from the hand check of E2–E3; every proposal approved,
  E-Q17 with the owner's two changes.
- Date: 2026-11-17
- Related:
  - ADR 0014 §2.7 (one command on the marked images, otherwise the current one);
  - ADR 0013 and ADR 0007 §9.5 (the rating and metadata writes and their order);
  - ADR 0007 D10 (the settings file and the Settings window);
  - ADR 0017 §2.1 (menus), U5-Q5 (the Tasks error list), §7.2 (wishes);
  - ADR 0018 (deleting), whose choice of images this ADR shares.

## 1. Context

- RawStride is for culling, rating and captioning; the images are developed and retouched in other
  programs. Today the user must leave RawStride, find the file in the file manager and open it in
  that program by hand, one folder at a time.
- The user should send the chosen images to the program of their choice with one key, and the
  other program must find the ratings and metadata RawStride has just written already on disk.
- Starting another program needs only the standard library (`std::process::Command`); no new
  dependency.

## 2. Decision

### 2.1 The program list

- Up to **5 programs**, each with a name and the path to its `.exe`. The first one is the default
  (E-Q1).
- Kept in `settings.txt` as one line per program, `external_program = <name> | <path>`, in the
  list's order. A line without a name, without a path or with a `|` in the name is dropped, as other
  bad settings are.
- Set up in a section **"External Programs"** of the Settings window (E-Q2): "Add…" opens the file
  dialog (`rfd`, already used) for an `.exe` and takes the file's name without `.exe` as the
  program's name; each row has a name field, "Remove" and "Move Up".

### 2.2 Commands and key

- The Image menu gets **"Open in {name}"** for each program, in the list's order. **Ctrl+E** opens
  in the first program, in the contact sheet and the preview (E-Q3).
- With no program in the list, the menu has one item **"Open in Another Program…"** and Ctrl+E does
  the same: both open the Settings window at "External Programs".
- A program whose `.exe` no longer exists is greyed in the menu (E-Q9).

### 2.3 Which files

- In the contact sheet the marked images, otherwise the current one; in the preview only the
  current image (E-Q4, as ADR 0018 B-Q2).
- Only the image file as the tab lists it: not its RAW+JPEG partner, not its sidecars or edit
  files. The other program finds its own files beside the image (E-Q5).

### 2.4 Pending writes first

- If a rating or metadata write of any of the images is waiting in the queue or running, the
  program is started when the last of them has answered; meanwhile the status line shows "Waiting
  for writes …" (E-Q6). A write that fails is reported as usual, and the program is still started
  (the file on disk is then as it was before the write).
- Another command of this kind while one waits replaces the waiting one.

### 2.5 Starting the program

- **One start** with every path as its own argument (E-Q7); `std::process::Command` quotes each
  one. RawStride does not wait for the program and does not watch it.
- How each receiving program takes several files (one window, several windows, only the first
  file) is its own business; the owner's hand check tries it with the owner's RAW editor.
- **Limits** (E-Q8): with more than 100 images RawStride first asks "Open {n} images in
  {name}?" ("Open" / "Cancel", Cancel the default). If the command line would be longer than
  Windows allows (32 767 characters), nothing is started and the message says why and how many
  images fit.

### 2.6 Errors

- A program that does not exist or cannot be started gives a message in the status line that names
  the program and the reason, and the same line goes to the Tasks error list (E-Q9).

### 2.7 When the other program changes the files

- RawStride does not watch the files. A rating or caption that the other program writes is seen
  when the tab is loaded again (E-Q12). A command to load a folder again is a wish in ADR 0017 §7.2.
- RawStride's own writes keep their rules: a file that the other program holds open is refused by
  the safe write (ADR 0007 §9), as today.

### 2.8 Show in File Explorer

- **"Show in File Explorer"** in the Image menu, without a key (E-Q11): opens the folder of the
  current image in File Explorer with the file selected (`explorer.exe /select,<path>`). A step of its
  own (E3).
- **The name (owner, 2026-11-19, while T203 was written):** the item says "File Explorer",
  Windows' own name for the program and the glossary's term (§5, T118), not the shorter
  "Explorer" this ADR first had.

### 2.9 A right-click menu on images (addition 1, accepted by the owner 2026-11-19)

- **Why:** in the hand check of E2–E3 (2026-11-19) the owner expected to right-click an image to
  reach "Show in File Explorer". Today only the folder tree, the tabs, Favourites and Recent have a
  right-click menu (ADR 0017 U4-Q3); the preview draws no menu bar, and "Show in File Explorer"
  has no key (E-Q11), so it cannot be reached from the preview at all.
- **Where** (E-Q15): on an image in the contact sheet and on the image in the preview (in the
  compare too, where it acts on the current image as the keys do). Not on the empty part of the
  contact sheet.
- **Which images** (E-Q16): the image right-clicked becomes the current image; the marking is
  kept as it is. The commands then act on their usual targets (ADR 0014 §2 point 6; E-Q4): a
  right-click on a marked image acts on all the marked images, a right-click on an image that is
  not marked acts on that image alone. In the preview only the current image, as today. Unlike
  File Explorer, a right-click on an image that is not marked does not clear the marking, so a
  slip of the mouse never loses a marking.
- **What it holds** (E-Q17), in this order: "Open in {name}" per program (or "Open in Another
  Program…" with an empty list), "Show in File Explorer"; a separator; the Rating submenu,
  Reject, Move to Picked, the Label submenu with the colours (Red, Yellow, Green, Blue, Purple,
  No label; owner, 2026-11-19); a separator; Metadata…; a separator; Delete to Recycle Bin.
  **Later (owner, 2026-11-19):** when metadata templates exist (ADR 0017 U13), the menu also gets
  a submenu to apply a template to the images, after Metadata…; not part of E4. Left out: the setting "Move Rejected and Picked Images to Subfolders", RAW ↔ JPEG
  and "Delete Rejected Images", which act on the folder or the view, not on the images clicked.
- **How it looks** (E-Q18): the same texts as the menu bar, each command's key shown on the right
  and greyed by the same rules (a missing program, no current image, Move to Picked while the
  setting is off). The menu is drawn from the same entries as the menu bar, so the two cannot
  drift apart.

## 3. Questions (answered by the owner, 2026-11-17: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| E-Q1 | The program list | Up to 5 programs, name and `.exe` path, the first the default; `external_program = <name> \| <path>` lines in `settings.txt` |
| E-Q2 | Where they are set up | A section "External Programs" in the Settings window: "Add…" (file dialog), name field, "Remove", "Move Up" |
| E-Q3 | Menu and key | "Open in {name}" per program in the Image menu; Ctrl+E the first program, in the contact sheet and the preview; with an empty list "Open in Another Program…" opens Settings |
| E-Q4 | Which images | Contact sheet: the marked, otherwise the current one; preview: only the current image |
| E-Q5 | Which files | Only the image file as the tab lists it; no partner, sidecar or edit file |
| E-Q6 | Pending writes | The program starts when the images' queued and running writes have answered; "Waiting for writes …" meanwhile |
| E-Q7 | How it is started | One start with every path as an argument; RawStride does not wait for the program |
| E-Q8 | Limits | A question above 100 images; refused with a reason when the command line would pass 32 767 characters |
| E-Q9 | Errors | Status line and Tasks error list; a program whose path is gone is greyed |
| E-Q10 | Windows' default program | Not now (it would need `ShellExecuteW`, a `windows-sys` feature in `pw-loupe`) |
| E-Q11 | Show in File Explorer | Yes, in the Image menu without a key, a step of its own (E3); named "Show in File Explorer" by the owner on 2026-11-19 (§2.8) |
| E-Q12 | Files changed by the other program | Not watched; seen when the tab is loaded again; a reload command as a wish in ADR 0017 §7.2 |
| E-Q13 | Steps | E1–E3 as §4 |
| E-Q14 | Not included | As §5 |

Addition 1 (answered by the owner, 2026-11-19: every proposal approved, E-Q17 with the colours
written out and the later template submenu):

| # | Question | Answer |
|---|---|---|
| E-Q15 | Where the right-click menu is | On an image in the contact sheet and on the image in the preview (and the compare); not on empty space |
| E-Q16 | Which images it acts on | The image clicked becomes the current one and the marking is kept: a marked image → all the marked, an image not marked → that image alone; the preview → the current image |
| E-Q17 | What it holds | Open in {name} per program, Show in File Explorer; Rating, Reject, Move to Picked, Label with the colours; Metadata…; Delete to Recycle Bin. Later, with ADR 0017 U13: a submenu to apply a metadata template (owner, 2026-11-19) |
| E-Q18 | How it looks | Same texts as the menu bar, keys on the right, greyed by the same rules, drawn from the same entries |
| E-Q19 | Step | One step E4 (one task in the shell), with the owner's hand check after it |

## 4. Steps

Each step ends with `.\tools\check.ps1`; the owner's hand check comes after E2 and E3.

| Step | Content |
|---|---|
| **E1** | The program list: the type and its `settings.txt` lines with their tests, and the section "External Programs" of the Settings window (§2.1); texts by rule 13 and in the glossary |
| **E2** | The menu items and Ctrl+E (§2.2), the targets (§2.3), the wait for writes (§2.4), the start with its limits (§2.5) and the errors (§2.6); plain-Rust parts tested without a window (the arguments, the length check, the wait). Hand check with the owner's RAW editor: one image, several marked images, an image just rated (the rating is in its sidecar when the program opens it), a removed program |
| **E3** | "Show in File Explorer" (§2.8) |
| **E4** | The right-click menu on images (§2.9, addition 1). Hand check: right-click a marked image and an image not marked in the contact sheet, the preview and the compare; "Show in File Explorer" and "Open in {name}" from the preview; a greyed item |

## 5. Not included

- Dragging files from RawStride into other programs.
- Waiting for the other program to close and reading its result back (for example a developed
  TIFF).
- Argument templates or other options per program.
- Windows' default program for a file type and Windows' own "Open with" chooser (E-Q10).
- Watching the folder for changes made by other programs (E-Q12).

## 6. Consequences

- A culled and rated set reaches the developing program in one key, with its ratings and captions
  already written.
- RawStride starts other programs only on the user's command, with only the chosen files, and
  never waits for them.
