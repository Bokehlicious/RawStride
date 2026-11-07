//! Checks on real RAW files (set PW_TESTDATA to the folder with the test RAWs, e.g.
//! D:\PhotoWorkflowData\RAW). Skipped when it is not set.
//!
//! The key guarantee of ADR 0006: a region rendered on its own (with the overlap margin) is
//! pixel-identical to the same area of a whole-image render. It is checked for AHD (the default)
//! and for PPG (ADR 0008 §11, the egui loupe of ADR 0009), and for a session that switches
//! between the two.

use std::path::PathBuf;

use pw_libraw::{Demosaic, Rect, Session};

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

/// Regions as the egui loupe renders them (ADR 0008 §12): the exact visible 100 % region and the
/// ring of twice its size, at even origins, plus the border cases of the grid test above.
fn loupe_regions(w: u32, h: u32) -> Vec<Rect> {
    vec![
        Rect::new(2048, 2048, 1024, 1024),
        Rect::new(0, 0, 1024, 1024),
        Rect::new(w - 1024, h - 1024, 1024, 1024),
        Rect::new((w / 3) & !1, (h / 3) & !1, 999, 601),
        // A 1920x1080 view and its ring (3840x2160), centred.
        Rect::new(((w - 1920) / 2) & !1, ((h - 1080) / 2) & !1, 1920, 1080),
        Rect::new(((w - 3840) / 2) & !1, ((h - 2160) / 2) & !1, 3840, 2160),
        // A vertical image's view in sensor coordinates (1080 wide, 1920 high).
        Rect::new(((w - 1080) / 2) & !1, ((h - 1920) / 2) & !1, 1080, 1920),
    ]
}

fn check_ppg_regions_identical(prefix: &str) {
    let Some(file) = test_file(prefix) else {
        eprintln!("PW_TESTDATA not set or no {prefix}* file; skipped");
        return;
    };
    let mut s = Session::open(&file).unwrap();
    let (w, h) = (s.width(), s.height());
    let full = s.render_full_with(Demosaic::Ppg).unwrap();
    assert_eq!((full.width, full.height), (w, h), "PPG, full size");
    for r in loupe_regions(w, h) {
        let region = s.render_region_with(r, 16, Demosaic::Ppg).unwrap();
        assert!(
            region == full.crop(r),
            "{}: PPG region {r:?} differs from the full PPG render",
            file.display()
        );
    }
    // A session reused after a half-size render (the loupe's background job) gives the same.
    let mut s2 = Session::open(&file).unwrap();
    let _ = s2.render_half().unwrap();
    for r in loupe_regions(w, h) {
        let region = s2.render_region_with(r, 16, Demosaic::Ppg).unwrap();
        assert!(
            region == full.crop(r),
            "{}: PPG region {r:?} from a session reused after half differs",
            file.display()
        );
    }
}

/// The method is a per-render choice: switching on one session never leaks into the next render,
/// and PPG really differs from AHD (so the choice reaches LibRaw).
fn check_methods_switch_cleanly(prefix: &str) {
    let Some(file) = test_file(prefix) else {
        eprintln!("PW_TESTDATA not set or no {prefix}* file; skipped");
        return;
    };
    let mut s = Session::open(&file).unwrap();
    let (w, h) = (s.width(), s.height());
    let r = Rect::new(((w - 1920) / 2) & !1, ((h - 1080) / 2) & !1, 1920, 1080);
    let ahd_full = s.render_full().unwrap();
    let ppg_full = s.render_full_with(Demosaic::Ppg).unwrap();
    assert!(
        ahd_full != ppg_full,
        "{}: PPG and AHD gave identical images",
        file.display()
    );
    // AHD after PPG, PPG after AHD, and the default API after PPG.
    let a = s.render_region_with(r, 16, Demosaic::Ahd).unwrap();
    assert!(a == ahd_full.crop(r), "{}: AHD after PPG", file.display());
    let p = s.render_region_with(r, 16, Demosaic::Ppg).unwrap();
    assert!(p == ppg_full.crop(r), "{}: PPG after AHD", file.display());
    let d = s.render_region(r, 16).unwrap();
    assert!(
        d == ahd_full.crop(r),
        "{}: default after PPG",
        file.display()
    );
    assert!(
        s.render_full().unwrap() == ahd_full,
        "{}: default full render after PPG",
        file.display()
    );
}

#[test]
fn a7r_iva_ppg_regions_are_identical_to_full() {
    check_ppg_regions_identical("TN1004");
}

#[test]
fn a1_ppg_regions_are_identical_to_full() {
    check_ppg_regions_identical("TN0059");
}

#[test]
fn panasonic_ppg_regions_are_identical_to_full() {
    check_ppg_regions_identical("P124");
}

#[test]
fn a7r_iva_methods_switch_cleanly() {
    check_methods_switch_cleanly("TN1004");
}

#[test]
fn panasonic_methods_switch_cleanly() {
    check_methods_switch_cleanly("P124");
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
