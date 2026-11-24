//! `Textures` against a real `ViewCore` with a simulated decoder and a headless egui context (no
//! window, no GPU: egui only records the texture uploads).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use pw_render::Textures;
use pw_viewcore::{
    Backend, Config, DecodeError, Frame, ImageSize, Rect, Rgb, Source, Status, ViewCore,
};

const VIEW: [u32; 2] = [1000, 800];
const WAIT: Duration = Duration::from_secs(10);

/// 4000×3000 RAW, 1600×1200 JPEG, flat grey.
struct Fake;

impl Backend for Fake {
    type Session = (u32, u32);

    fn open(&self, _path: &Path, source: Source) -> Result<((u32, u32), ImageSize), DecodeError> {
        let (w, h) = match source {
            Source::Raw => (4000, 3000),
            Source::Jpeg => (1600, 1200),
        };
        Ok(((w, h), ImageSize { w, h, flip: 0 }))
    }

    fn render_half(&self, s: &mut (u32, u32)) -> Result<Rgb, DecodeError> {
        let (w, h) = (s.0 / 2, s.1 / 2);
        Ok(Rgb {
            width: w,
            height: h,
            data: vec![128; (w * h * 3) as usize],
        })
    }

    fn render_region(&self, _s: &mut (u32, u32), r: Rect) -> Result<Rgb, DecodeError> {
        Ok(Rgb {
            width: r.w,
            height: r.h,
            data: vec![128; (r.w * r.h * 3) as usize],
        })
    }
}

fn start() -> ViewCore<Fake> {
    match ViewCore::start(Fake, Config::default()) {
        Ok(c) => c,
        Err(e) => panic!("start: {e}"),
    }
}

fn until(
    tex: &mut Textures,
    ctx: &egui::Context,
    core: &ViewCore<Fake>,
    max_side: usize,
    done: impl Fn(&Frame) -> bool,
) -> Frame {
    let end = Instant::now() + WAIT;
    loop {
        tex.sync(ctx, core, max_side);
        let f = core.frame(VIEW);
        if done(&f) || Instant::now() > end {
            return f;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn finished_views_become_textures_and_the_drawn_one_is_resident() {
    let ctx = egui::Context::default();
    let core = start();
    core.open_folder(vec![PathBuf::from("a"), PathBuf::from("b")]);
    let mut tex = Textures::new();
    let f = until(&mut tex, &ctx, &core, 16384, |f| f.status == Status::Ready);
    assert_eq!(f.status, Status::Ready);
    let id = f.draw.map(|d| d.id).unwrap_or(0);
    assert!(tex.texture(id).is_some(), "the drawn view has a texture");
    // the core counts the fit view, reduced to 1000×750 (ADR 0027 W1), at its RGBA size
    let (_, bytes) = core.memory();
    assert!(bytes >= 1000 * 750 * 4);
}

#[test]
fn a_view_larger_than_the_gpu_allows_is_rejected_and_shown_as_an_error() {
    let ctx = egui::Context::default();
    let core = start();
    core.open_folder(vec![PathBuf::from("a")]);
    let mut tex = Textures::new();
    // the fit view is 1000×750 (ADR 0027 W1): too large for a GPU limit of 512
    let f = until(&mut tex, &ctx, &core, 512, |f| {
        matches!(f.status, Status::Error(_))
    });
    assert!(matches!(f.status, Status::Error(ref e) if e.message.contains("larger than the GPU")));
    assert!(tex.is_empty());
}

#[test]
fn a_new_folder_releases_the_old_textures() {
    let ctx = egui::Context::default();
    let core = start();
    core.open_folder(vec![PathBuf::from("a")]);
    let mut tex = Textures::new();
    until(&mut tex, &ctx, &core, 16384, |f| f.status == Status::Ready);
    assert!(!tex.is_empty());
    core.open_folder(vec![PathBuf::from("b")]);
    let f = until(&mut tex, &ctx, &core, 16384, |f| {
        f.status == Status::Ready && f.path == Some(PathBuf::from("b"))
    });
    assert_eq!(f.status, Status::Ready);
    // only the new folder's textures are left
    let id = f.draw.map(|d| d.id).unwrap_or(0);
    assert!(tex.texture(id).is_some());
    assert!(tex.len() <= 4, "{} textures", tex.len());
}
