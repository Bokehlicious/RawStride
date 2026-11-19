# ADR 0012: Product name RawStride

- Status: **accepted by the owner (2026-11-09).**
- Date: 2026-11-09
- Related: ADR 0009 §11 decision 4 (window title), ADR 0010 §2 (the program may be renamed later),
  GUI_REQUIREMENTS GR8 (the Tauri app stays until the switch).

## Context

- The project has been called PhotoWorkflow, and the egui program shows the window title
  "PhotoWorkflow Loupe". With folder tabs, the contact sheet and the import (ADR 0010, 0011) it is no
  longer only a loupe; it is becoming the main program.
- The owner wanted a name of its own before the project is put in a private GitHub repository.
- Candidates were searched on 2026-11-09 (quick web searches, not a legal trademark search):
  - RawFlow: an Android RAW editor and an iPhone app already use the name;
  - SwiftRaw: free, but "Swift" suggests Apple's language and Mac/iPhone apps;
  - ShotSelect: an existing photo culling app;
  - ShotFlow: an existing company and product (fashion samples);
  - QuickFrame: several programs in other fields (steel framing, structural engineering);
  - FrameScout: a small GitHub project and similar viewfinder apps; suggests film rather than stills;
  - **RawStride:** nothing found. "Stride" (pace, "hit your stride") fits a program whose first goal
    is fast culling.

## Decision

1. The product is called **RawStride**.
2. **Now (step 1):** the window title of the egui program is "RawStride"; the README, the
   architecture notes and the crate description use the new name. A private GitHub repository will
   be called `RawStride`.
3. **Later (step 2, its own task):** the binary is renamed from `pw-loupe` to `rawstride`, at the
   latest when the egui program replaces the Tauri app (GR8).
4. **Unchanged:** the crate names (`pw-core`, `pw-raw`, …, `pw-loupe`), the folders
   `C:\Development\PhotoWorkflow` and `PhotoWorkflow-GUI`, the branch names, `PW_TESTDATA` and
   `D:\PhotoWorkflowData`. Renaming them gives no benefit to the user and would break paths and
   scripts. The Tauri app keeps its title "PhotoWorkflow" until it is retired.
5. Earlier ADRs keep the old name as written; they are records of their time.

## Consequences

- `latency.ps1` must target the window title "RawStride" (ADR 0009 §11 decision 4 is amended).
- Before any public release, a proper trademark search is still needed (ADR 0002 is open).
- **Trademark search (owner, 2026-11-19, public-release plan PR-Q3):** "RawStride" in TMview
  (tmdn.org, the Trade Mark and Design Network's search over EUIPO, the national offices such as
  PRV, and offices outside the EU such as USPTO and WIPO), reached from EUIPO's availability page:
  0 hits. "Stride" in TMview in Nice classes 9 and 42 (software): many hits; the owner's quick
  read of them found none that looked like a conflict with RawStride.
- **Step 2 done (2026-11-19, T218):** the binary is `rawstride` (`rawstride.exe`); the crate is still `pw-loupe` (decision 4).
