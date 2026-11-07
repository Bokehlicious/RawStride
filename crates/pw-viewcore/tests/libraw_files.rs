//! The core with LibRaw (PPG) on the real test RAWs. Needs the `libraw` feature and PW_TESTDATA
//! (the folder with the test RAWs, e.g. D:\PhotoWorkflowData\RAW); skipped when it is not set.
#![cfg(feature = "libraw")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use pw_viewcore::libraw::LibRawBackend;
use pw_viewcore::{Config, Frame, Status, ViewCore, ViewKind, ViewOut, Zoom};

const VIEW: [u32; 2] = [1920, 1080];
const WAIT: Duration = Duration::from_secs(30);

fn testdata(prefix: &str) -> Vec<PathBuf> {
    let Some(dir) = std::env::var_os("PW_TESTDATA").map(PathBuf::from) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with(prefix))
                        && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                            matches!(e.to_ascii_lowercase().as_str(), "arw" | "rw2" | "dng")
                        })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

struct Screen<'a> {
    core: &'a ViewCore<LibRawBackend>,
    textures: HashMap<u64, ViewOut>,
}

impl Screen<'_> {
    fn frame(&mut self) -> Frame {
        for v in self.core.take_views() {
            let bytes = u64::from(v.image.width) * u64::from(v.image.height) * 4;
            if let Some(id) = self.core.accept(&v, bytes) {
                self.textures.insert(id, v);
            }
        }
        for id in self.core.take_dropped() {
            self.textures.remove(&id);
        }
        self.core.frame(VIEW)
    }

    fn until_ready(&mut self) -> Frame {
        let end = Instant::now() + WAIT;
        loop {
            let f = self.frame();
            if f.status == Status::Ready
                || matches!(f.status, Status::Error(_))
                || Instant::now() > end
            {
                return f;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn start() -> ViewCore<LibRawBackend> {
    match ViewCore::start(LibRawBackend, Config::default()) {
        Ok(c) => c,
        Err(e) => panic!("start: {e}"),
    }
}

#[test]
fn a7r_vertical_images_fit_and_zoom_with_correct_sizes() {
    let files = testdata("TN1004");
    if files.is_empty() {
        eprintln!("PW_TESTDATA not set or no TN1004* files; skipped");
        return;
    }
    let core = start();
    core.open_folder(files);
    let mut screen = Screen {
        core: &core,
        textures: HashMap::new(),
    };
    let f = screen.until_ready();
    assert_eq!(f.status, Status::Ready);
    let size = f.size.unwrap_or_else(|| panic!("no size"));
    assert!(
        size.flip == 5 || size.flip == 6,
        "A7R test files are vertical: {size:?}"
    );
    let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
    assert_eq!(tex.kind, ViewKind::Half);
    assert_eq!(
        (tex.image.width, tex.image.height),
        (size.w / 2, size.h / 2)
    );
    // 100 %: a sensor region of the view's size, swapped for the vertical image
    core.toggle_zoom(None);
    let f = screen.until_ready();
    assert_eq!(f.zoom, Zoom::Scale(1.0));
    let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
    assert!(matches!(tex.kind, ViewKind::Zoom | ViewKind::Ring));
    assert!(
        tex.region.w >= 1080 && tex.region.h >= 1920,
        "{:?}",
        tex.region
    );
    assert_eq!(
        (tex.image.width, tex.image.height),
        (tex.region.w, tex.region.h)
    );
    // the next image at 100 % (V6 keeps the level)
    core.step(1);
    let f = screen.until_ready();
    assert_eq!(f.status, Status::Ready);
    assert_eq!(f.zoom, Zoom::Scale(1.0));
}

#[test]
fn a_file_that_is_not_raw_shows_an_error() {
    if std::env::var_os("PW_TESTDATA").is_none() {
        eprintln!("PW_TESTDATA not set; skipped");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("pw-viewcore-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let f = tmp.join("not-raw.ARW");
    let _ = std::fs::write(&f, b"this is not a raw file");
    let core = start();
    core.open_folder(vec![f]);
    let mut screen = Screen {
        core: &core,
        textures: HashMap::new(),
    };
    let frame = screen.until_ready();
    assert!(
        matches!(frame.status, Status::Error(_)),
        "{:?}",
        frame.status
    );
    assert!(frame.draw.is_none());
    let _ = std::fs::remove_dir_all(&tmp);
}
