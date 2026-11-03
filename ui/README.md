# ui

The frontend lives in `web/`. It is plain HTML, CSS and JavaScript with no npm and no build step,
and is embedded into the app by `src-tauri` at compile time.

- `web/index.html`: layout (toolbar, contact sheet, loupe, info panel, status bar)
- `web/app.js`: logic (folders, grid, loupe zoom/pan, orientation, preloading)
- `web/style.css`: styling
