# ui

The frontend lives in `web/`. It is plain HTML, CSS and JavaScript with no npm and no build step,
and is embedded into the app by `src-tauri` at compile time.

- `web/index.html`: layout (toolbar, contact sheet, loupe, info panel, status bar)
- `web/app.js`: logic (folders, grid, loupe zoom/pan, orientation, preloading)
- `web/ingest.js`: the import dialog (ADR 0005)
- `web/style.css`: styling
- `tests/filter-select.test.js`: UI regression test (filter, multi-selection, rating keys) against
  a mocked backend. Needs Node and Playwright, which are not part of the app build.
- `tests/ingest-dialog.test.js`: the import dialog against a mocked backend.
