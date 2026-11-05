# prototypes/phase2/p3-wgpu

ADR 0008 phase 2, **P3: presenter A**. **Not part of the app.** The crate has its own workspace and
builds into `target\prototypes`.

**New dependency:** `wgpu`, with its own dependencies. The owner approved it for this prototype on
2026-11-05. Approval for the app is a separate decision. The crate also uses Tauri's `unstable`
feature, which provides a window with child webviews.

## What it measures

A wgpu surface draws the image area of the window. The UI is a Tauri webview, laid out in one of
two ways:

- **overlay:** the webview covers the whole window, with a transparent background;
- **strip:** the webview is a 48 px toolbar above the image area.

Each swap takes one of two forms:

- **upload:** the RGBA bytes in RAM are uploaded to a texture and then drawn. These are the finished
  views from the phase 1 core.
- **resident:** the image is already a GPU texture (a VRAM cache), so it is only drawn.

The images are the same as in P2: 100 % views of 1920×1080 and 2560×1440, and a whole A1 preview
(8640×5760, 199 MB) fitted to the window.

Run it with `run-p3.cmd`, which takes about 10 minutes:

1. **Timed swaps in Rust** (`p3-bench.json`): 20 swaps per image size, swap form and present mode
   (Fifo, plus Mailbox or Immediate when the GPU offers it). Each swap records:
   - the upload;
   - frame acquisition;
   - the time until present returns;
   - the time until the GPU is done.
   
   The run also records memory (the app and its WebView2 processes) and VRAM, from wgpu's
   allocator report.
2. **On-screen latency** (`p3-latency.csv`): `latency.ps1` presses the right arrow 20 times per
   layout, image size and swap form, and measures the time until the pixels change.

Checked by hand afterwards (ADR 8.3, robustness):

- resize;
- DPI change;
- focus and keyboard;
- moving the window to another screen;
- GPU device loss.
