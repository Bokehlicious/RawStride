# prototypes/gui/p7-egui-presenter

ADR 0008 / `docs/GUI_REQUIREMENTS.md` **P7: image presentation in eframe + egui.** This is a
prototype and **not part of the app**.

- The crate has its own workspace and builds into `target\prototypes`.
- Its only new dependency is the owner-approved eframe 0.37.0 (GUI_REQUIREMENTS §6), with `wgpu`,
  `glow`, `accesskit`, `default_fonts` and `clipboard`.
- LibRaw comes from the repository's vcpkg install, through a copy of the phase 1 shim (`csrc/proto.cpp`).
- PDH comes with Windows.

## What it does

- **Core (`core.rs`).**
  - P5's scheduling rules: two workers (interactive and background), a 150 ms dwell, prefetch of +1
    and +2 in the direction of travel plus −1 behind, admission and LRU eviction, and a core budget for
    LibRaw sessions.
  - CPU PPG renders the views:
    - **fit:** the whole image at half size;
    - **zoom:** the exact visible 100 % region at the V6 position;
    - **ring:** twice that region, for panning.
- **Presentation (`app.rs`).**
  - Each finished view is uploaded as an **egui texture**. Textures are the view cache, under their
    own byte budget.
  - An image change or Z is a draw only when the texture is resident.
  - Orientation costs nothing: each corner of the upright view gets its own texture coordinate
    (`view.rs`).
  - Swap rule (V2): the previous view stays until the new one is complete, and a neutral indicator
    appears after 1 s.
- **Measurement (`driver.rs`, `mem.rs`).**
  - P5's RAW-mode scenarios (S1, S2, S4, S6, S5, NAT, MIX), with the same positions and random
    sequence as P5 and P6c.
  - Stress tests M1–M5, as in P6c.
  - The M0 texture sweep.
  - Memory in P6c's four layers (L1 own accounting, L2 process memory, L3 PDH GPU memory, L4 system
    commit), sampled every 250 ms.

## Running it

**Build and check:** run `build-p7.cmd`. It runs `cargo build --release`, `cargo test` and
`cargo clippy -D warnings`, and writes `build.txt`.

**Measure:** run `run-p7.cmd [A|B|C|D|all]`. Results go to `D:\PhotoWorkflowData\analysis\gui\p7\`.
Don't use the computer while it runs.

| Part | Contents | Time |
|---|---|---|
| A | M0 texture memory, per renderer | about 5 min |
| B | P5 scenarios: A7R at 2048 / 1024 / 512 MB of textures, A1 at 1024 MB, per renderer; RW2 with wgpu | about 55 min |
| C | Stress M1–M5, per renderer | about 16 min |
| D | On screen (latency.ps1): Z, RIGHT at 1500 and 400 ms, pan at 100 %, A1 image change | about 10 min |

Output per configuration:

- `<tag>.txt`: summary, snapshots, M0/M2/M3/M4;
- `<tag>-events.csv`: P5's event format plus the draw time and the miss cause;
- `<tag>-mem.csv`: memory per layer every 250 ms.

## Known limits

- **"Shown" is measured in the app**, when the UI draws the view. Presentation on screen is measured
  only in part D.
- **Correctness is checked on the data:** a hash of each view's pixels, same file and region giving the
  same hash. Screen pixels are not read back.
- **JPEG mode and S3 are not run** (RAW mode only), as in P6c.
- **DPI change is postponed** by the owner. Resizing, minimise and restore, and keyboard focus are
  checked by hand after the run.
- **The process ends with `TerminateProcess` once the results are written.** In the first run the
  wgpu process stayed alive after its window had closed, until the script's timeout; glow exited
  normally. The stderr markers (`<tag>-stderr.txt`) show that `run_native` returns, so the process
  stops somewhere in its own exit. This shortcut is for measurement only; the cause is not examined.
