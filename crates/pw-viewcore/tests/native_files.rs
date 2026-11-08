//! The core with LibRaw (PPG) and libjpeg-turbo on the real test RAWs. These tests are ignored by
//! default; run them with
//! `$env:PW_TESTDATA = "<folder>"; cargo test -p pw-viewcore --features native -- --ignored`.
#![cfg(feature = "native")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use pw_viewcore::native::NativeBackend;
use pw_viewcore::{Config, Frame, Source, Status, ViewCore, ViewKind, ViewOut, Zoom};

const VIEW: [u32; 2] = [1920, 1080];
const WAIT: Duration = Duration::from_secs(30);

fn testdata() -> PathBuf {
    let Some(dir) = std::env::var_os("PW_TESTDATA") else {
        panic!("PW_TESTDATA is not set; set it to the real camera test-data folder");
    };
    let dir = PathBuf::from(dir);
    assert!(
        dir.is_dir(),
        "PW_TESTDATA folder does not exist: {}",
        dir.display()
    );
    dir
}

fn test_files(prefix: &str) -> Vec<PathBuf> {
    let dir = testdata();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| {
        panic!(
            "could not read PW_TESTDATA folder {}: {error}",
            dir.display()
        )
    });
    let mut files: Vec<PathBuf> = entries
        .map(|entry| {
            entry
                .unwrap_or_else(|error| {
                    panic!(
                        "could not read an entry in PW_TESTDATA folder {}: {error}",
                        dir.display()
                    )
                })
                .path()
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        matches!(ext.to_ascii_lowercase().as_str(), "arw" | "rw2" | "dng")
                    })
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no RAW test file with prefix {prefix} was found in PW_TESTDATA folder {}",
        dir.display()
    );
    files
}

struct Screen<'a> {
    core: &'a ViewCore<NativeBackend>,
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

fn start() -> ViewCore<NativeBackend> {
    match ViewCore::start(NativeBackend, Config::default()) {
        Ok(c) => c,
        Err(e) => panic!("start: {e}"),
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a7r_vertical_images_fit_and_zoom_with_correct_sizes() {
    let files = test_files("TN1004");
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

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn jpeg_mode_shows_the_embedded_jpeg_upright_at_its_own_size() {
    for prefix in ["TN1004", "TN0059", "P124"] {
        let files = test_files(prefix);
        let core = start();
        core.set_mode(Source::Jpeg);
        core.open_folder(files.clone());
        let mut screen = Screen {
            core: &core,
            textures: HashMap::new(),
        };
        let f = screen.until_ready();
        assert_eq!(f.status, Status::Ready, "{prefix}");
        assert_eq!(f.source, Source::Jpeg);
        let size = f.size.unwrap_or_else(|| panic!("no size"));
        let info = pw_raw::inspect(&files[0]).unwrap_or_else(|e| panic!("{e}"));
        let large = info
            .pick(pw_raw::PreviewSize::Large)
            .unwrap_or_else(|| panic!("{prefix}: no preview"));
        assert_eq!((size.w, size.h), (large.width, large.height), "{prefix}");
        // the RAW file's orientation tag decides the rotation
        let expected = match info.orientation {
            3 => 3,
            6 => 6,
            8 => 5,
            _ => 0,
        };
        assert_eq!(size.flip, expected, "{prefix}");
        // 100 % of the JPEG
        core.toggle_zoom(None);
        let f = screen.until_ready();
        assert_eq!(f.status, Status::Ready, "{prefix} at 100 %");
        let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
        assert_eq!(tex.source, Source::Jpeg);
        assert_eq!(
            (tex.image.width, tex.image.height),
            (tex.region.w, tex.region.h)
        );
    }
}
