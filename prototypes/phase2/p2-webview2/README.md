# prototypes/phase2/p2-webview2

ADR 0008 phase 2, **P2: presenter B2**. **Not part of the app.** The crate has its own workspace and
builds into `target\prototypes`.

It uses only crates that are already in the app's dependency tree: Tauri, and through it
webview2-com and windows-core. The latter two are used directly to reach the WebView2 API.

## What it measures

Finished pixels (RGBA) travel from Rust to the page and onto the screen.

| Path | How the pixels reach the page |
|---|---|
| **B1** | `fetch` from a custom scheme, like today's `pw://`. The bytes are copied into the response |
| **B2-new** | A new WebView2 shared buffer per image (`CreateSharedBuffer` + copy + `PostSharedBufferToScript`) |
| **B2-pool** | Shared buffers created and filled beforehand. Only `PostSharedBufferToScript` is timed. This is what the view core would do if it rendered straight into shared memory |
| **B2-res** | Added 2026-11-05 after the decision matrix. Each image arrives once (pooled shared buffer) and stays as its own WebGL texture, a VRAM cache like P3's "resident" swap. Only drawing is timed |

In every path, the page uploads the pixels to a texture and draws it with **WebGL2**. When the
runtime has **WebGPU**, B2-pool is also drawn with WebGPU.

Images:

- `v1080` and `v1440`: 100 % views, 1:1 on screen;
- `full`: a whole A1 preview, 8640×5760 (199 MB), fitted to the window. It is also drawn once with
  mipmaps generated after upload.

Run it with `run-p2.cmd`, which takes about 10 minutes:

1. **In-page timings** (`p2-bench.json`): 20 swaps per path, API and image, after 3 warm-up swaps.
   Each swap records:
   - the time to the page;
   - the texture upload;
   - the GPU finishing;
   - the next animation frame;
   - Rust's copy and post time.
   
   The run also records memory (private bytes of the app and all its WebView2 processes) after each
   step, and a buffer-lifetime test.
2. **On-screen latency** (`p2-latency.csv`): `latency.ps1` presses the right arrow 20 times per
   path and image size, and measures the time until the pixels on screen change.

Inputs: two A1 files from `D:\PhotoWorkflowData\RAW`. Output: `D:\PhotoWorkflowData\analysis\phase2\p2\`.
