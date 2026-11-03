# src-tauri

The Tauri 2 application shell. See [docs/adr/0003-m1-viewer-architecture.md](../docs/adr/0003-m1-viewer-architecture.md).

```powershell
cargo run -p photoworkflow --release                              # start
cargo run -p photoworkflow --release -- D:\PhotoWorkflowData\RAW  # start with a folder
```

- Commands: `pick_folder`, `initial_folder`, `open_folder`, `image_info`
- Image scheme: `http://pw.localhost/<generation>/<index>/<small|medium|large>`. It only serves
  files in the folder the user opened.
- The frontend lives in `../ui/web` and is embedded at compile time, so rebuild after UI changes.
