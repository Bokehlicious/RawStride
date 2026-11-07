//! ADR 0008 / GUI_REQUIREMENTS **P7: image presentation in eframe + egui.** Not part of the app.
//!
//! The view core (P5's rules, CPU PPG through LibRaw) prepares fit and 100 % views on worker threads;
//! the egui UI uploads them as textures and draws the current one. Measures, for the glow and the wgpu
//! renderer: image change, Z and pan on screen (with latency.ps1), P5's scenarios, texture memory, and
//! correctness under memory pressure.
//!
//! Usage: `p7-egui-presenter --suite <p5|stress|m0|interactive> --renderer <wgpu|glow> --out <dir>
//!   --tag <name> [--tex-mb <MB>] [--core-mb <MB>] [--zoomed] <files…>`

mod app;
mod core;
mod driver;
mod libraw;
mod mem;
mod view;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use eframe::egui;

use crate::core::Core;
use crate::driver::{Config, Driver, UiShared};

fn arg(a: &[String], name: &str) -> Option<String> {
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn parse() -> Result<(Config, Vec<PathBuf>), String> {
    let a: Vec<String> = std::env::args().collect();
    let with_value = ["--suite", "--renderer", "--out", "--tag", "--tex-mb", "--core-mb"];
    let mut skip = std::collections::HashSet::new();
    for (i, x) in a.iter().enumerate() {
        if with_value.contains(&x.as_str()) {
            skip.insert(i + 1);
        }
    }
    let files: Vec<PathBuf> =
        a.iter().enumerate().skip(1).filter(|(i, x)| !skip.contains(i) && !x.starts_with("--")).map(|(_, x)| PathBuf::from(x)).collect();
    if files.is_empty() {
        return Err("give RAW files".into());
    }
    if let Some(f) = files.iter().find(|f| !f.is_file()) {
        return Err(format!("not a file: {}", f.display()));
    }
    let num = |name: &str, default: usize| -> Result<usize, String> {
        arg(&a, name).map_or(Ok(default), |v| v.parse().map_err(|_| format!("{name}: not a number: {v}")))
    };
    let renderer = arg(&a, "--renderer").unwrap_or_else(|| "wgpu".into());
    if renderer != "wgpu" && renderer != "glow" {
        return Err(format!("--renderer must be wgpu or glow, not {renderer}"));
    }
    let cfg = Config {
        suite: arg(&a, "--suite").unwrap_or_else(|| "interactive".into()),
        out: PathBuf::from(arg(&a, "--out").unwrap_or_else(|| ".".into())),
        tag: arg(&a, "--tag").unwrap_or_else(|| "p7".into()),
        renderer,
        tex_mb: num("--tex-mb", 1024)?,
        core_mb: num("--core-mb", 4096)?,
    };
    Ok((cfg, files))
}

fn run() -> Result<(), String> {
    let (cfg, files) = parse()?;
    std::fs::create_dir_all(&cfg.out).map_err(|e| format!("{}: {e}", cfg.out.display()))?;
    let renderer = if cfg.renderer == "glow" { eframe::Renderer::Glow } else { eframe::Renderer::Wgpu };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("P7 egui presenter").with_maximized(true),
        renderer,
        ..Default::default()
    };
    let core = Core::new(libraw::LibRaw, files, cfg.core_mb << 20, cfg.tex_mb << 20);
    let interactive = cfg.suite == "interactive";
    let zoomed = std::env::args().any(|a| a == "--zoomed");
    let renderer_name = cfg.renderer.clone();
    let mut cfg = Some(cfg);
    eframe::run_native(
        "P7 egui presenter",
        options,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            let wake_ctx = ctx.clone();
            core.set_output(tx, Box::new(move || wake_ctx.request_repaint()));
            core.start()?;
            let ui = Arc::new(UiShared::default());
            let c2 = core.clone();
            let sampler = mem::Sampler::start(Box::new(move || {
                let g = c2.state();
                (g.sess_bytes as f64 / mem::MB, g.tex_bytes as f64 / mem::MB, 0.0)
            }))?;
            if let Some(cfg) = cfg.take().filter(|c| c.suite != "interactive") {
                let wake_ctx = ctx.clone();
                let d = Driver { core: core.clone(), sampler, ui: ui.clone(), wake: Box::new(move || wake_ctx.request_repaint()), cfg };
                std::thread::Builder::new().name("p7-driver".into()).spawn(move || d.run()).map_err(|e| format!("spawn driver: {e}"))?;
            }
            if interactive {
                core.go(0, "start");
                if zoomed {
                    core.zoom(true, "start");
                }
            }
            Ok(Box::new(app::App::new(core.clone(), rx, ui, interactive, renderer_name.clone())))
        }),
    )
    .map_err(|e| format!("{e:?}"))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            eprintln!("p7: run_native returned");
            mem::hard_exit(0, "after run_native");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("p7: {e}");
            ExitCode::FAILURE
        }
    }
}
