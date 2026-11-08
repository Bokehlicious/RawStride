//! Star ratings and colour labels (milestone 3, first part).
//!
//! Stored as `xmp:Rating` / `xmp:Label` in XMP sidecars next to RAW files, written through one
//! persistent ExifTool process (`pw-metadata`). DNG, JPEG and TIFF carry the values embedded in
//! the file itself (only the metadata is rewritten; the modification date is kept).

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use pw_metadata::{ExifTool, Rating};
use serde::Serialize;
use tauri::State;

use crate::AppState;

/// The shared ExifTool process, started on first use.
#[derive(Debug, Default)]
pub struct Metadata(Mutex<Option<ExifTool>>);

impl Metadata {
    pub(crate) fn with<T>(
        &self,
        f: impl FnOnce(&mut ExifTool) -> pw_metadata::Result<T>,
    ) -> Result<T, String> {
        let mut guard: MutexGuard<'_, Option<ExifTool>> =
            self.0.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = Some(ExifTool::start().map_err(|e| e.to_string())?);
        }
        let Some(et) = guard.as_mut() else {
            return Err("ExifTool is not available".into());
        };
        let result = f(et);
        // A broken process (I/O failure or unexpected exit) refuses every further command;
        // drop it so the next call starts a new one.
        if guard.as_ref().is_some_and(ExifTool::is_broken) {
            *guard = None;
        }
        result.map_err(|e| e.to_string())
    }

    /// Starts ExifTool in the background so the first rating is not delayed.
    pub fn warm_up(&self) {
        let _ = self.with(|et| et.execute(&["-ver".to_string()]));
    }
}

#[derive(Debug, Serialize)]
pub struct RatingDto {
    index: usize,
    stars: Option<i8>,
    label: Option<String>,
}

fn files_of(state: &AppState, generation: u64) -> Result<Vec<PathBuf>, String> {
    let lib = state.lock();
    if lib.generation != generation {
        return Err("folder has changed".into());
    }
    Ok(lib.files.clone())
}

/// Ratings of all images in the open folder that have a sidecar with a rating or label.
#[tauri::command]
pub async fn read_ratings(
    generation: u64,
    state: State<'_, AppState>,
    meta: State<'_, Metadata>,
) -> Result<Vec<RatingDto>, String> {
    let files = files_of(&state, generation)?;
    let found = meta.with(|et| et.read_ratings(&files))?;
    Ok(files
        .iter()
        .enumerate()
        .filter_map(|(index, f)| {
            found.get(f).map(|r| RatingDto {
                index,
                stars: r.stars,
                label: r.label.clone(),
            })
        })
        .collect())
}

/// Sets rating and label of one image (both values; `None` clears).
#[tauri::command]
pub async fn set_rating(
    generation: u64,
    index: usize,
    stars: Option<i8>,
    label: Option<String>,
    state: State<'_, AppState>,
    meta: State<'_, Metadata>,
) -> Result<RatingDto, String> {
    let files = files_of(&state, generation)?;
    let path = files.get(index).ok_or("no such image")?;
    let stars = stars.filter(|&s| s != 0);
    let label = label.filter(|l| !l.is_empty());
    let rating = Rating {
        stars,
        label: label.clone(),
    };
    meta.with(|et| et.write_rating(path, &rating))?;
    if pw_metadata::supports_embedded(path) {
        // Rewriting the metadata can move embedded previews (e.g. in DNG); inspect again.
        state.forget(generation, index);
    }
    Ok(RatingDto {
        index,
        stars,
        label,
    })
}
