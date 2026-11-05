//! Built-in measurement run (ADR 0006: measure every step against M1).
//!
//! `photoworkflow.exe <folder> --bench <out.json> --bench-raw on|off [--bench-half-max <px>]
//! [--bench-reuse on|off]` makes the UI run a fixed
//! scenario (`ui/web/bench.js`), write the timings to `<out.json>` and exit. Without `--bench`
//! nothing here is used.

use serde::Serialize;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize)]
pub struct BenchConfig {
    out: String,
    raw: bool,
    folder: Option<String>,
    /// Long-edge limit for half-size images instead of the screen size (0 = full half size).
    half_max: Option<u32>,
    /// Half-size session reused for tiles (see [`reuse_half_session`]).
    reuse: bool,
}

fn arg_after(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

/// The measurement configuration, if the app was started with `--bench`.
#[tauri::command]
pub fn bench_config() -> Option<BenchConfig> {
    let out = arg_after("--bench")?;
    Some(BenchConfig {
        out,
        raw: arg_after("--bench-raw").as_deref() != Some("off"),
        folder: std::env::args()
            .skip(1)
            .find(|a| !a.starts_with("--") && std::path::Path::new(a).is_dir()),
        half_max: arg_after("--bench-half-max").and_then(|v| v.parse().ok()),
        reuse: reuse_half_session(),
    })
}

/// Whether the engine keeps the half-size job's session for tiles. Always on, except in a
/// measurement run started with `--bench-reuse off`.
pub fn reuse_half_session() -> bool {
    arg_after("--bench").is_none() || arg_after("--bench-reuse").as_deref() != Some("off")
}

/// Writes the results (only to the path given with `--bench`).
#[tauri::command]
pub fn bench_write(json: String) -> Result<(), String> {
    let out = arg_after("--bench").ok_or("not in measurement mode")?;
    std::fs::write(&out, json).map_err(|e| format!("{out}: {e}"))
}

/// Ends the measurement run.
#[tauri::command]
pub fn bench_exit(app: AppHandle) -> Result<(), String> {
    if arg_after("--bench").is_none() {
        return Err("not in measurement mode".into());
    }
    app.exit(0);
    Ok(())
}
