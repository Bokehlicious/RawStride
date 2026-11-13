# T115. `latency.ps1`: read the screen with DXGI Desktop Duplication, so the full-screen preview can be measured — Done

- **Can start now.** Owner's request 2026-11-13. No repository code changes; it does not touch
  `pw-loupe` and can run beside any task. T111's acceptance waits for this task's measurement.
- **Files:** `D:\PhotoWorkflowData\pm-study\tools\latency.ps1` (outside the repository; first copy it
  unchanged to `latency-gdi.ps1` in the same folder); this file: rule 11. Nothing in the repository
  changes apart from this file's status and log row, so `check.ps1` is not needed.
- **Why:** `latency.ps1` reads the screen with GDI (`Graphics.CopyFromScreen`). In T111's preview
  RawStride covers the whole screen with OpenGL; Windows then shows the window directly, past the
  desktop composition, and GDI (and `PrintWindow` with `PW_RENDERFULLCONTENT`, also tried) gets only
  a white area: all 78 steps of the T111 run (`latency-t111.csv`, labels `t111-…`) recorded no
  change, although the owner saw the images change. T110's maximized window measured fine with GDI.
- **What:**
  1. Replace only the source of the pixels in `Lat.Grab` with **DXGI Desktop Duplication**
     (`IDXGIOutput1::DuplicateOutput`, `AcquireNextFrame`, copy to a CPU-readable staging texture,
     `Map`), written in the script's existing C# block with plain COM interop (`[ComImport]`
     interfaces or function pointers); **no new program, package or DLL** (no SharpDX, no NuGet).
     The output (8-bit grey samples on the same grid, `step`, `Diff`) stays as today.
  2. Everything else unchanged: parameters, keys, timing, `first_ms`/`settled_ms`, the CSV columns
     and decimal comma, `-Region` as fractions of the window. `AcquireNextFrame` with a short
     timeout returning "no new frame" means "same as last frame".
  3. A switch `-Capture gdi|dxgi` (default `dxgi`) so the old method can still be chosen.
  4. If Desktop Duplication cannot be started (another adapter, remote desktop), stop with a clear
     message; never fall back silently to GDI.
- **Check (the agent runs these; the owner must not touch mouse or keyboard while they run):**
  build the release (`cargo build --release --workspace`; if `target\release\pw-loupe.exe` is
  locked, ask the owner to close RawStride), then, from `D:\PhotoWorkflowData\pm-study\tools`:
  1. **Same result as GDI where GDI works:** RawStride started with the folder
     `D:\PhotoWorkflowData\s6\a1` (opens the contact sheet, maximized window); `-Key RIGHT -Count 10
     -Gap 1500` once with `-Capture gdi` and once with `-Capture dxgi`, `-Out` a scratch CSV. Both
     must see every step (10 changes) with medians within about one frame (17 ms) of each other.
  2. **The full-screen preview:** the T111 run again with `-Capture dxgi`. A ready script is
     `t111-latency.ps1` in the reviewer session's scratchpad folder (under `%LOCALAPPDATA%\Temp\claude\`)
     (copy it next to `latency.ps1` if that folder is gone; it backs up and restores
     `%LOCALAPPDATA%\RawStride\settings.txt`, and runs RAW/JPEG fit and 100 % 7 steps/s with the
     filmstrip shown and hidden on the A1 folder). Delete the old `t111-…` rows from
     `latency-t111.csv` first (they are invalid), or write to `latency-t111-dxgi.csv`.
  3. Compare with `latency-t110.csv` and `latency-t109.csv` (median and max of `first_ms`, and how
     many of 13 steps changed in the 7 steps/s runs).
- **Done when:** both checks are done; the report gives the GDI/DXGI comparison of check 1 and the
  T111 table of check 2 next to T109 and T110, with filmstrip shown vs hidden. Set T115 to
  "Ready for review" and add one log row with the numbers.
