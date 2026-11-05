//! Checks on real RAW files (set PW_TESTDATA to the folder with the test RAWs, e.g.
//! D:\PhotoWorkflowData\RAW). Skipped when it is not set.
//!
//! The key guarantee of ADR 0006: a region rendered on its own (with the overlap margin) is
//! pixel-identical to the same area of a whole-image render.

use std::path::PathBuf;

use pw_libraw::{Rect, Session};

fn test_file(prefix: &str) -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("PW_TESTDATA")?);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix))
        })
        .collect();
    files.sort();
    files.into_iter().next()
}

fn check_regions_identical(prefix: &str) {
    let Some(file) = test_file(prefix) else {
        eprintln!("PW_TESTDATA not set or no {prefix}* file; skipped");
        return;
    };
    let mut s = Session::open(&file).unwrap();
    let (w, h) = (s.width(), s.height());
    let full = s.render_full().unwrap();
    assert_eq!(
        (full.width, full.height),
        (w, h),
        "sensor orientation, full size"
    );
    // Tiles of the viewer's grid: interior (on the A7R IVA it contains the highlight that exposed
    // the white-level problem), at the origin, at the right and bottom border.
    let tiles = [
        Rect::new(2048, 2048, 1024, 1024),
        Rect::new(0, 0, 1024, 1024),
        Rect::new(w - 1024, h - 1024, 1024, 1024),
        Rect::new(w - 700, 1024, 700, 1024),
        // A partial tile with odd size at an even origin (the grid only uses even origins).
        Rect::new((w / 3) & !1, (h / 3) & !1, 999, 601),
    ];
    for r in tiles {
        let tile = s.render_region(r, 16).unwrap();
        assert!(
            tile == full.crop(r),
            "{}: region {r:?} differs from the full render",
            file.display()
        );
    }
    // Rendering the same region again gives the same pixels.
    let a = s.render_region(tiles[0], 16).unwrap();
    let b = s.render_region(tiles[0], 16).unwrap();
    assert!(a == b, "repeat differs");
    let half = s.render_half().unwrap();
    assert_eq!((half.width, half.height), (w / 2, h / 2));
    // After a half-size render the same session still gives identical regions.
    for r in tiles {
        let tile = s.render_region(r, 16).unwrap();
        assert!(
            tile == full.crop(r),
            "{}: region {r:?} after half differs",
            file.display()
        );
    }
    // The viewer's case (ADR 0006 step 4): a fresh session that rendered the half-size image
    // first (the background job) is reused for tiles.
    let mut s2 = Session::open(&file).unwrap();
    let _ = s2.render_half().unwrap();
    for r in tiles {
        let tile = s2.render_region(r, 16).unwrap();
        assert!(
            tile == full.crop(r),
            "{}: region {r:?} from a session reused after half differs",
            file.display()
        );
    }
}

#[test]
fn a7r_iva_regions_are_identical_to_full() {
    check_regions_identical("TN1004");
}

#[test]
fn a1_regions_are_identical_to_full() {
    check_regions_identical("TN0059");
}

#[test]
fn panasonic_regions_are_identical_to_full() {
    check_regions_identical("P124");
}

#[test]
fn not_a_raw_file_is_an_error() {
    let dir = std::env::temp_dir().join(format!("pw-libraw-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("not-raw.ARW");
    std::fs::write(&f, b"this is not a raw file").unwrap();
    let e = Session::open(&f).unwrap_err();
    assert!(e.to_string().contains("not-raw.ARW"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn version_is_reported() {
    assert!(
        pw_libraw::version().starts_with("0.22"),
        "{}",
        pw_libraw::version()
    );
}
