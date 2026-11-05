//! PhotoWorkflow – Tauri 2 application shell: browse, view and rate (RAW, DNG, JPEG, TIFF).
//!
//! The frontend (`ui/web`) talks to Rust through a handful of commands and loads image bytes
//! through the `pw://` URI scheme (`http://pw.localhost/…` on Windows). That scheme only serves
//! files of the folder the user opened, addressed by `<generation>/<index>/<size>` – it never
//! accepts a file path, so it cannot be used to read arbitrary files.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod imaging;
mod bench;
mod ingest;
mod ratings;
mod rawview;
mod tiff_view;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use pw_raw::{ImageInfo, PreviewSize};
use serde::Serialize;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// The folder currently open in the viewer.
#[derive(Debug, Default)]
struct Library {
    /// Increases every time a folder is opened; part of every image URL.
    generation: u64,
    folder: Option<PathBuf>,
    files: Vec<PathBuf>,
    info: HashMap<usize, Arc<ImageInfo>>,
}

#[derive(Debug, Default)]
struct AppState(Mutex<Library>);

impl AppState {
    fn lock(&self) -> MutexGuard<'_, Library> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Path and (cached) inspection result of file `index` in folder `generation`.
    fn info(&self, generation: u64, index: usize) -> Result<(PathBuf, Arc<ImageInfo>), String> {
        let path = {
            let lib = self.lock();
            if lib.generation != generation {
                return Err("folder has changed".into());
            }
            if let Some(i) = lib.info.get(&index) {
                return Ok((lib.files[index].clone(), i.clone()));
            }
            lib.files.get(index).cloned().ok_or("no such image")?
        };
        // Inspect without holding the lock (file I/O).
        let info = Arc::new(inspect(&path)?);
        let mut lib = self.lock();
        if lib.generation == generation {
            lib.info.insert(index, info.clone());
        }
        Ok((path, info))
    }

    /// Drops the cached inspection of one file (after its metadata was rewritten in place,
    /// which can move embedded previews inside the file).
    pub(crate) fn forget(&self, generation: u64, index: usize) {
        let mut lib = self.lock();
        if lib.generation == generation {
            lib.info.remove(&index);
        }
    }
}

/// pw-raw inspection; plain TIFFs it cannot read (e.g. BigTIFF) still open with the decoder's
/// image size, without EXIF.
fn inspect(path: &Path) -> Result<ImageInfo, String> {
    match pw_raw::inspect(path) {
        Ok(i) => Ok(i),
        Err(e) if pw_raw::is_tiff_file(path) => {
            let (w, h) = tiff_view::dimensions(path).map_err(|t| format!("{e}; {t}"))?;
            Ok(ImageInfo {
                format: Some(pw_raw::Format::Tiff),
                orientation: 1,
                width: Some(w),
                height: Some(h),
                ..ImageInfo::default()
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Debug, Serialize)]
struct FileEntry {
    index: usize,
    name: String,
    size: u64,
    /// Ratings/labels can be stored for this file (XMP sidecar for RAW, embedded for
    /// DNG/JPEG/TIFF).
    ratable: bool,
}

#[derive(Debug, Serialize)]
struct FolderListing {
    generation: u64,
    folder: String,
    files: Vec<FileEntry>,
}

#[derive(Debug, Serialize)]
struct PreviewDto {
    width: u32,
    height: u32,
    bytes: u64,
    source: String,
}

#[derive(Debug, Serialize)]
struct ImageDetails {
    name: String,
    format: Option<String>,
    make: Option<String>,
    model: Option<String>,
    lens: Option<String>,
    date_time_original: Option<String>,
    exposure_time: Option<String>,
    f_number: Option<f64>,
    iso: Option<u32>,
    focal_length: Option<f64>,
    exposure_bias: Option<f64>,
    orientation: u16,
    /// Sensor (RAW) dimensions, or the JPEG's own size for JPEG files.
    width: Option<u32>,
    height: Option<u32>,
    is_jpeg_file: bool,
    /// Plain TIFF: the previews below are decoded from the image data, not embedded JPEGs.
    is_tiff_file: bool,
    previews: Vec<PreviewDto>,
    small: Option<PreviewDto>,
    medium: Option<PreviewDto>,
    large: Option<PreviewDto>,
}

fn dto(p: Option<&pw_raw::PreviewRef>) -> Option<PreviewDto> {
    p.map(|p| PreviewDto {
        width: p.width,
        height: p.height,
        bytes: p.length,
        source: p.source.clone(),
    })
}

fn list_images(folder: &Path) -> Result<Vec<(PathBuf, u64)>, String> {
    let rd = std::fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut files: Vec<(PathBuf, u64)> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let ext = path.extension()?.to_str()?.to_ascii_lowercase();
            if !pw_raw::SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
                return None;
            }
            let meta = e.metadata().ok()?;
            meta.is_file().then_some((path, meta.len()))
        })
        .collect();
    files.sort_by_key(|(p, _)| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    Ok(files)
}

// ---------- commands ----------

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

#[tauri::command]
fn initial_folder() -> Option<String> {
    std::env::args().skip(1).find(|a| Path::new(a).exists())
}

#[tauri::command]
async fn open_folder(
    path: String,
    state: State<'_, AppState>,
    raw: State<'_, rawview::RawView>,
) -> Result<FolderListing, String> {
    let mut folder = PathBuf::from(&path);
    // A dropped or passed file opens its folder.
    if folder.is_file()
        && let Some(parent) = folder.parent()
    {
        folder = parent.to_path_buf();
    }
    let files = list_images(&folder)?;
    let mut lib = state.lock();
    lib.generation += 1;
    lib.folder = Some(folder.clone());
    lib.info.clear();
    lib.files = files.iter().map(|(p, _)| p.clone()).collect();
    // RAW rendering of the old folder is dropped (queue, caches, LibRaw sessions).
    raw.folder_opened(lib.generation);
    Ok(FolderListing {
        generation: lib.generation,
        folder: folder.display().to_string(),
        files: files
            .iter()
            .enumerate()
            .map(|(index, (p, size))| FileEntry {
                index,
                name: p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                size: *size,
                ratable: pw_metadata::supports_rating(p),
            })
            .collect(),
    })
}

#[tauri::command]
async fn image_info(
    generation: u64,
    index: usize,
    state: State<'_, AppState>,
) -> Result<ImageDetails, String> {
    let (path, i) = state.info(generation, index)?;
    let is_tiff = pw_raw::is_tiff_file(&path);
    // TIFF previews are rendered from the image data at fixed sizes (see tiff_view).
    let decoded = |size: PreviewSize| -> Option<PreviewDto> {
        let (w, h) = i.width.zip(i.height)?;
        let (width, height) = imaging::fit(w, h, tiff_view::max_edge(size));
        Some(PreviewDto {
            width,
            height,
            bytes: 0,
            source: "TIFF-avkodning".into(),
        })
    };
    let pick = |size: PreviewSize| {
        if is_tiff {
            decoded(size)
        } else {
            dto(i.pick(size))
        }
    };
    Ok(ImageDetails {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        format: i.format.map(|f| format!("{f:?}")),
        make: i.make.clone(),
        model: i.model.clone(),
        lens: i.lens.clone(),
        date_time_original: i.date_time_original.clone(),
        exposure_time: i.exposure_time.map(|e| e.to_string()),
        f_number: i.f_number,
        iso: i.iso,
        focal_length: i.focal_length,
        exposure_bias: i.exposure_bias,
        orientation: i.orientation,
        width: i.width,
        height: i.height,
        is_jpeg_file: i.format == Some(pw_raw::Format::Jpeg),
        is_tiff_file: is_tiff,
        previews: i
            .previews
            .iter()
            .map(|p| dto(Some(p)).expect("some"))
            .collect(),
        small: pick(PreviewSize::Small),
        medium: pick(PreviewSize::Medium),
        large: pick(PreviewSize::Large),
    })
}

// ---------- pw:// image protocol ----------

fn respond(status: StatusCode, body: Vec<u8>, content_type: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        // Images are requested with crossorigin="anonymous" so that CSS
        // `image-orientation: none` is honoured; see ui/web/app.js.
        .header("Access-Control-Allow-Origin", "*")
        // The URL contains the folder generation, so cached responses never go stale.
        .header("Cache-Control", "max-age=3600")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn serve(app: &AppHandle, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let not_found = |m: &str| respond(StatusCode::NOT_FOUND, m.as_bytes().to_vec(), "text/plain");
    // Path: /<generation>/<index>/<small|medium|large|rawhalf>, /<generation>/<index>/tile/<c>/<r>,
    // or /bench/half (measurement)
    let parts: Vec<&str> = request.uri().path().trim_matches('/').split('/').collect();
    if parts.as_slice() == ["bench", "half"] {
        return match app.state::<rawview::RawView>().bench_bytes() {
            Some(b) => respond(StatusCode::OK, b, "image/jpeg"),
            None => not_found("no measurement image"),
        };
    }
    if let [g, i, "tile", c, r] = parts.as_slice() {
        // RAW tile at full resolution (ADR 0006 step 4); only requested after `raw-tile`.
        let (Ok(generation), Ok(index), Ok(col), Ok(row)) =
            (g.parse::<u64>(), i.parse::<usize>(), c.parse::<u32>(), r.parse::<u32>())
        else {
            return not_found("bad path");
        };
        return match app.state::<rawview::RawView>().tile(generation, index, col, row) {
            Some(b) => respond(StatusCode::OK, b, "image/jpeg"),
            None => not_found("RAW tile not ready"),
        };
    }
    if let [g, i, "rawhalf"] = parts.as_slice() {
        // RAW half-size image (ADR 0006); only requested after the `raw-ready` event.
        let (Ok(generation), Ok(index)) = (g.parse::<u64>(), i.parse::<usize>()) else {
            return not_found("bad path");
        };
        return match app.state::<rawview::RawView>().half(generation, index) {
            Some(b) => respond(StatusCode::OK, b, "image/jpeg"),
            None => not_found("RAW image not ready"),
        };
    }
    let [g, i, s] = parts.as_slice() else {
        return not_found("bad path");
    };
    let (Ok(generation), Ok(index)) = (g.parse::<u64>(), i.parse::<usize>()) else {
        return not_found("bad path");
    };
    let size = match *s {
        "small" => PreviewSize::Small,
        "medium" => PreviewSize::Medium,
        "large" => PreviewSize::Large,
        _ => return not_found("bad size"),
    };
    let state = app.state::<AppState>();
    let (path, info) = match state.info(generation, index) {
        Ok(r) => r,
        Err(e) => return not_found(&e),
    };
    if pw_raw::is_tiff_file(&path) {
        return match app
            .state::<tiff_view::TiffView>()
            .jpeg(&path, generation, index, size)
        {
            Ok(jpeg) => respond(StatusCode::OK, jpeg.as_ref().clone(), "image/jpeg"),
            Err(e) => respond(
                StatusCode::INTERNAL_SERVER_ERROR,
                e.into_bytes(),
                "text/plain",
            ),
        };
    }
    let Some(preview) = info.pick(size) else {
        return not_found("no embedded preview");
    };
    match pw_raw::read_preview(&path, preview) {
        Ok(mut bytes) => {
            // The viewer rotates from the RAW orientation; stop WebView2 from rotating again.
            pw_raw::neutralize_orientation(&mut bytes);
            respond(StatusCode::OK, bytes, "image/jpeg")
        }
        Err(e) => respond(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string().into_bytes(),
            "text/plain",
        ),
    }
}

/// The RAW renderer's library version (ADR 0006). Not used by the UI yet (step 1 links LibRaw).
#[tauri::command]
fn raw_engine_info() -> String {
    format!(
        "LibRaw {} (capabilities 0x{:x})",
        pw_libraw::version(),
        pw_libraw::capabilities()
    )
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .manage(ratings::Metadata::default())
        .manage(tiff_view::TiffView::default())
        .manage(ingest::IngestState::default())
        .setup(|app| {
            // Start ExifTool in the background so the first rating is instant.
            let handle = app.handle().clone();
            std::thread::spawn(move || handle.state::<ratings::Metadata>().warm_up());
            // RAW rendering (ADR 0006); its events go to the UI.
            app.manage(rawview::RawView::new(app.handle().clone()));
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("pw", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(serve(&app, &request));
            });
        })
        .invoke_handler(tauri::generate_handler![
            pick_folder,
            initial_folder,
            open_folder,
            image_info,
            ratings::read_ratings,
            ratings::set_rating,
            ingest::ingest_volumes,
            ingest::ingest_preview,
            ingest::ingest_start,
            ingest::ingest_cancel,
            raw_engine_info,
            rawview::raw_view,
            rawview::raw_set_enabled,
            rawview::raw_stats,
            rawview::raw_set_half_max,
            rawview::raw_prepare,
            rawview::raw_tiles,
            rawview::raw_bench_half,
            bench::bench_config,
            bench::bench_write,
            bench::bench_exit
        ])
        .run(tauri::generate_context!())
        .expect("error while running PhotoWorkflow");
}
