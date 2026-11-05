//! RAW rendering in the loupe (ADR 0006, step 3: half-size images).
//!
//! The job engine (`pw-rawview`) runs with the LibRaw decoder. The UI tells it which image the
//! loupe shows (and its neighbours) with `raw_view`; when a half-size image is ready the engine's
//! event becomes a `raw-ready` Tauri event and the UI loads `pw://<gen>/<index>/rawhalf`.
//!
//! - **M1 stays the fallback:** the embedded preview is always shown first; RAW pixels only replace
//!   it once decoded. A failed decode emits `raw-failed` and the preview stays.
//! - **"RAW-rendering" off** (`raw_set_enabled(false)`) clears the engine (queue, caches, sessions)
//!   and ignores `raw_view`: exactly the M1 code paths.
//! - **Only when needed:** a half-size image is requested only for RAW files whose largest
//!   embedded preview is smaller than the screen (A7R IVA: 1616 px; the A1's 8640 px preview is
//!   enough, so the A1 keeps using it).
//! - The contact sheet does not use RAW rendering (`raw_view` with no image clears the current
//!   image in the engine).
//! - **Half-size images are scaled to the screen** (long edge = the screen's long edge in physical
//!   pixels) before JPEG encoding: measured in step 3, the full half size cost 3–6 × more in
//!   encoding, transfer and WebView2 decoding for no visible gain at "fit".
//! - **No unpacking ahead on dwell:** in step 3 it cost CPU and memory for every RAW file shown,
//!   so it is off (`Config::unpack_ahead`).
//! - **Tiles (step 4):** the loupe asks for full-resolution tiles (`raw_tiles`, served at
//!   `pw://<gen>/<index>/tile/<col>/<row>` after a `raw-tile` event) once the image it shows has
//!   too little detail for the zoom level. Only RAW files whose embedded preview is smaller than
//!   the RAW get tiles (the A1's full-size preview is enough). The file is unpacked on the first
//!   zoom action (`raw_prepare`), or the half-size job's session is reused
//!   (`Config::reuse_half_session`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pw_rawview::libraw::LibRawBackend;
use pw_rawview::{Backend, Config, Engine, Event, Image, ImageKey, Lookup, Rect, Target, TileId};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{AppState, imaging};

/// File types LibRaw decodes for the viewer (JPEG/TIFF are shown as they are).
fn is_raw(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| {
            pw_raw::SUPPORTED_EXTENSIONS.contains(&e.as_str())
                && !matches!(e.as_str(), "jpg" | "jpeg" | "tif" | "tiff")
        })
}

/// LibRaw, with an optional limit on the long edge of half-size images (0 = full half size).
/// The limit exists to measure whether scaling before encoding pays off (step 3).
#[derive(Debug, Clone)]
pub struct ScaledBackend {
    inner: LibRawBackend,
    half_max: Arc<AtomicU32>,
}

impl Backend for ScaledBackend {
    type Session = <LibRawBackend as Backend>::Session;

    fn open(&self, path: &Path) -> Result<Self::Session, String> {
        self.inner.open(path)
    }

    fn size(&self, s: &Self::Session) -> (u32, u32) {
        self.inner.size(s)
    }

    fn render_half(&self, s: &mut Self::Session) -> Result<Image, String> {
        let img = self.inner.render_half(s)?;
        Ok(limit(img, self.half_max.load(Ordering::Relaxed)))
    }

    fn render_region(&self, s: &mut Self::Session, r: Rect, margin: u32) -> Result<Image, String> {
        self.inner.render_region(s, r, margin)
    }

    fn encode(&self, img: &Image) -> Result<Vec<u8>, String> {
        self.inner.encode(img)
    }
}

/// Area-averages `img` down so its long edge is at most `max_edge` (0 = unchanged).
fn limit(img: Image, max_edge: u32) -> Image {
    if max_edge == 0 || img.width.max(img.height) <= max_edge {
        return img;
    }
    let (tw, th) = imaging::fit(img.width, img.height, max_edge);
    let src = imaging::Rgb8 {
        width: img.width,
        height: img.height,
        data: img.data,
    };
    let d = imaging::downscale(&src, tw, th);
    Image {
        width: d.width,
        height: d.height,
        data: d.data,
    }
}

/// App state: the engine and the "RAW-rendering" setting.
pub struct RawView {
    engine: Engine<ScaledBackend>,
    enabled: AtomicBool,
    half_max: Arc<AtomicU32>,
    /// The half-size limit follows the screen (false once a measurement set it explicitly).
    half_auto: AtomicBool,
    /// Highest `seq` of a `raw_view` call applied so far. Calls can finish out of order (each
    /// inspects its file first); an older one must not make an earlier image current again.
    view_seq: Mutex<u64>,
    /// Output of the last `raw_bench_half` (served at `pw://bench/half`).
    bench: Mutex<Option<Vec<u8>>>,
}

impl std::fmt::Debug for RawView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawView")
            .field("enabled", &self.enabled.load(Ordering::Relaxed))
            .finish()
    }
}

#[derive(Debug, Clone, Serialize)]
struct RawEventDto {
    generation: u64,
    index: usize,
    kind: &'static str,
    message: Option<String>,
    /// Size of the full RAW image (what 100 % zoom means), for "half".
    full_width: Option<u32>,
    full_height: Option<u32>,
}

impl RawView {
    pub fn new(app: AppHandle) -> RawView {
        let half_max = Arc::new(AtomicU32::new(0));
        let engine = Engine::new(
            ScaledBackend {
                inner: LibRawBackend,
                half_max: half_max.clone(),
            },
            Config {
                unpack_ahead: false,
                // `--bench-reuse off` (measurement only) turns reuse off to compare.
                reuse_half_session: crate::bench::reuse_half_session(),
                ..Config::default()
            },
        );
        // Full sizes reported by the engine (a Grid event comes just before each Half event).
        let sizes: Mutex<HashMap<ImageKey, (u32, u32)>> = Mutex::new(HashMap::new());
        engine.set_listener(move |ev| {
            let dto = match ev {
                Event::Grid(k, g) => {
                    let mut m = sizes.lock().unwrap_or_else(|e| e.into_inner());
                    m.retain(|kk, _| kk.generation == k.generation);
                    m.insert(k, (g.width, g.height));
                    drop(m);
                    let _ = app.emit(
                        "raw-grid",
                        RawGridDto {
                            generation: k.generation,
                            index: k.index,
                            width: g.width,
                            height: g.height,
                            tile: g.tile,
                        },
                    );
                    return;
                }
                Event::Tile(k, t) => {
                    let _ = app.emit(
                        "raw-tile",
                        RawTileDto {
                            generation: k.generation,
                            index: k.index,
                            col: t.col,
                            row: t.row,
                        },
                    );
                    return;
                }
                Event::Half(k) => {
                    let full = sizes
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(&k)
                        .copied();
                    RawEventDto {
                        generation: k.generation,
                        index: k.index,
                        kind: "half",
                        message: None,
                        full_width: full.map(|f| f.0),
                        full_height: full.map(|f| f.1),
                    }
                }
                Event::Failed(k, m) => RawEventDto {
                    generation: k.generation,
                    index: k.index,
                    kind: "failed",
                    message: Some(m),
                    full_width: None,
                    full_height: None,
                },
            };
            let name = if dto.kind == "failed" {
                "raw-failed"
            } else {
                "raw-ready"
            };
            let _ = app.emit(name, dto);
        });
        RawView {
            engine,
            enabled: AtomicBool::new(true),
            half_max,
            half_auto: AtomicBool::new(true),
            view_seq: Mutex::new(0),
            bench: Mutex::new(None),
        }
    }

    /// A folder was opened (called from `open_folder`).
    pub fn folder_opened(&self, generation: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.engine.set_folder(generation);
        }
    }

    /// Encoded half-size image, if it is ready (for the `pw://…/rawhalf` route).
    pub fn half(&self, generation: u64, index: usize) -> Option<Vec<u8>> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        match self.engine.half(
            ImageKey { generation, index },
            Duration::from_millis(50),
        ) {
            Lookup::Ready(b) => Some(b.as_ref().clone()),
            _ => None,
        }
    }

    /// Encoded tile, if it is ready (for the `pw://…/tile/<col>/<row>` route).
    pub fn tile(&self, generation: u64, index: usize, col: u32, row: u32) -> Option<Vec<u8>> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        match self.engine.tile(
            ImageKey { generation, index },
            TileId { col, row },
            Duration::from_millis(50),
        ) {
            Lookup::Ready(b) => Some(b.as_ref().clone()),
            _ => None,
        }
    }

    pub fn bench_bytes(&self) -> Option<Vec<u8>> {
        self.bench.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[derive(Debug, Clone, Serialize)]
struct RawGridDto {
    generation: u64,
    index: usize,
    width: u32,
    height: u32,
    tile: u32,
}

#[derive(Debug, Clone, Serialize)]
struct RawTileDto {
    generation: u64,
    index: usize,
    col: u32,
    row: u32,
}

#[derive(Debug, Serialize)]
pub struct RawViewDto {
    enabled: bool,
    /// The loupe should expect a half-size RAW image for this image.
    want_half: bool,
    /// LibRaw can render tiles of this file (the loupe decides when it needs them).
    can_tile: bool,
    /// Full RAW size and tile size, if already known (otherwise a `raw-grid` event follows).
    grid: Option<(u32, u32, u32)>,
}

/// Longest edge of the largest embedded preview (0 if none).
fn preview_edge(info: &pw_raw::ImageInfo) -> u32 {
    info.previews
        .iter()
        .map(|p| p.width.max(p.height))
        .max()
        .unwrap_or(0)
}

/// The loupe shows `index` (None: the loupe was left). `neighbours` are prepared in the
/// background. `screen_edge` is the screen's long edge in physical pixels.
#[tauri::command]
pub async fn raw_view(
    generation: u64,
    index: Option<usize>,
    neighbours: Vec<usize>,
    screen_edge: u32,
    seq: Option<u64>,
    state: State<'_, AppState>,
    raw: State<'_, RawView>,
) -> Result<RawViewDto, String> {
    // Applies `f` to the engine unless a later raw_view call was applied already.
    let apply = |f: &dyn Fn()| {
        let mut last = raw.view_seq.lock().unwrap_or_else(|e| e.into_inner());
        match seq {
            Some(n) if n < *last => {} // stale: a later image is current
            Some(n) => {
                *last = n;
                f();
            }
            None => f(),
        }
    };
    if !raw.enabled.load(Ordering::Relaxed) {
        return Ok(RawViewDto {
            enabled: false,
            want_half: false,
            can_tile: false,
            grid: None,
        });
    }
    let Some(index) = index else {
        apply(&|| raw.engine.clear_current());
        return Ok(RawViewDto {
            enabled: true,
            want_half: false,
            can_tile: false,
            grid: None,
        });
    };
    let target = |i: usize| -> Option<Target> {
        let (path, info) = state.info(generation, i).ok()?;
        let want_half = is_raw(&path) && preview_edge(&info) < screen_edge;
        Some(Target {
            key: ImageKey {
                generation,
                index: i,
            },
            path,
            want_half,
        })
    };
    let Some(current) = target(index) else {
        // Unreadable file: nothing to render, and the previous image's work stops.
        apply(&|| raw.engine.clear_current());
        return Ok(RawViewDto {
            enabled: true,
            want_half: false,
            can_tile: false,
            grid: None,
        });
    };
    let want_half = current.want_half;
    let can_tile = is_raw(&current.path);
    // Scale half-size images to the screen. A change (other screen) invalidates cached ones.
    if raw.half_auto.load(Ordering::Relaxed)
        && raw.half_max.swap(screen_edge, Ordering::Relaxed) != screen_edge
    {
        raw.engine.set_folder(0);
    }
    let others: Vec<Target> = neighbours.into_iter().filter_map(target).collect();
    let key = current.key;
    apply(&|| {
        raw.engine.set_folder(generation);
        raw.engine.set_current(current.clone(), &others);
    });
    let grid = raw.engine.grid(key).map(|g| (g.width, g.height, g.tile));
    Ok(RawViewDto {
        enabled: true,
        want_half,
        can_tile,
        grid,
    })
}

/// First zoom action on the current image: unpack it now so tiles can follow (no-op if a session
/// is kept or the image is not current).
#[tauri::command]
pub fn raw_prepare(generation: u64, index: usize, raw: State<'_, RawView>) {
    if raw.enabled.load(Ordering::Relaxed) {
        raw.engine.prepare(ImageKey { generation, index });
    }
}

/// The tiles the loupe needs now, most important first (under the cursor, then the rest of the
/// visible area). Replaces the previous request.
#[tauri::command]
pub fn raw_tiles(generation: u64, index: usize, tiles: Vec<(u32, u32)>, raw: State<'_, RawView>) {
    if !raw.enabled.load(Ordering::Relaxed) {
        return;
    }
    let ids: Vec<TileId> = tiles
        .into_iter()
        .map(|(col, row)| TileId { col, row })
        .collect();
    raw.engine
        .request_tiles(ImageKey { generation, index }, &ids);
}

/// The "RAW-rendering" setting. Off clears all RAW work and memory (M1 behaviour).
#[tauri::command]
pub fn raw_set_enabled(enabled: bool, generation: u64, raw: State<'_, RawView>) {
    raw.enabled.store(enabled, Ordering::Relaxed);
    // Generation 0 is never a folder: switching to it drops every queue, cache and session.
    raw.engine
        .set_folder(if enabled { generation } else { 0 });
}

#[derive(Debug, Serialize)]
pub struct RawStatsDto {
    enabled: bool,
    sessions_kept: usize,
    halves: usize,
    half_bytes: usize,
    tiles: usize,
    tile_bytes: usize,
    queued: usize,
    running: usize,
    jobs_dropped: u64,
    results_discarded: u64,
    sessions_opened: u64,
    sessions_evicted: u64,
    halves_evicted: u64,
    tiles_evicted: u64,
    sessions_reused: u64,
}

/// Engine counters (measurements).
#[tauri::command]
pub fn raw_stats(raw: State<'_, RawView>) -> RawStatsDto {
    let s = raw.engine.stats();
    RawStatsDto {
        enabled: raw.enabled.load(Ordering::Relaxed),
        sessions_kept: s.sessions_kept,
        halves: s.halves,
        half_bytes: s.half_bytes,
        tiles: s.tiles,
        tile_bytes: s.tile_bytes,
        queued: s.queued,
        running: s.running,
        jobs_dropped: s.jobs_dropped,
        results_discarded: s.results_discarded,
        sessions_opened: s.sessions_opened,
        sessions_evicted: s.sessions_evicted,
        halves_evicted: s.halves_evicted,
        tiles_evicted: s.tiles_evicted,
        sessions_reused: s.sessions_reused,
    }
}

/// Long-edge limit for half-size images (0 = none), overriding the screen size. Only used by
/// the measurement run (see docs/analyses/2026-11-04-m2-pipeline-steps.md, step 3).
#[tauri::command]
pub fn raw_set_half_max(max_edge: u32, generation: u64, raw: State<'_, RawView>) {
    raw.half_auto.store(false, Ordering::Relaxed);
    raw.half_max.store(max_edge, Ordering::Relaxed);
    // Cached half-size images were made with the old limit.
    raw.engine.set_folder(0);
    if raw.enabled.load(Ordering::Relaxed) {
        raw.engine.set_folder(generation);
    }
}

#[derive(Debug, Serialize)]
pub struct BenchHalfDto {
    open_ms: f64,
    render_ms: f64,
    scale_ms: f64,
    encode_ms: f64,
    width: u32,
    height: u32,
    bytes: usize,
}

/// Measurement: renders the half-size image of one file outside the engine, limited to
/// `max_edge`, and keeps the JPEG for `pw://bench/half` so the UI can time its decoding.
#[tauri::command]
pub async fn raw_bench_half(
    generation: u64,
    index: usize,
    max_edge: u32,
    state: State<'_, AppState>,
    raw: State<'_, RawView>,
) -> Result<BenchHalfDto, String> {
    let (path, _) = state.info(generation, index)?;
    let (dto, bytes) = tauri::async_runtime::spawn_blocking(move || {
        let b = LibRawBackend;
        let t = Instant::now();
        let mut s = b.open(&path)?;
        let open_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let img = b.render_half(&mut s)?;
        let render_ms = t.elapsed().as_secs_f64() * 1000.0;
        drop(s);
        let t = Instant::now();
        let img = limit(img, max_edge);
        let scale_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let bytes = b.encode(&img)?;
        let encode_ms = t.elapsed().as_secs_f64() * 1000.0;
        Ok::<_, String>((
            BenchHalfDto {
                open_ms,
                render_ms,
                scale_ms,
                encode_ms,
                width: img.width,
                height: img.height,
                bytes: bytes.len(),
            },
            bytes,
        ))
    })
    .await
    .map_err(|e| e.to_string())??;
    *raw.bench.lock().unwrap_or_else(|e| e.into_inner()) = Some(bytes);
    Ok(dto)
}
