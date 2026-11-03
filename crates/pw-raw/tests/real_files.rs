//! Checks `pw-raw` against the project's RAW test set. The expected values come from the
//! ExifTool survey (docs/analyses/2026-11-03-pw-raw-embedded-previews.md).
//!
//! Runs only when `PW_TESTDATA` points to the RAW folder, e.g.
//! `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test -p pw-raw`

use std::path::PathBuf;

fn file(name: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("PW_TESTDATA")?;
    let p = PathBuf::from(dir).join(name);
    if p.exists() {
        Some(p)
    } else {
        eprintln!("skipping: {} not found", p.display());
        None
    }
}

fn widths(info: &pw_raw::ImageInfo) -> Vec<u32> {
    info.previews.iter().map(|p| p.width).collect()
}

#[test]
fn sony_a1() {
    let Some(p) = file("TN005956.ARW") else {
        return;
    };
    let i = pw_raw::inspect(&p).unwrap();
    assert_eq!(i.model.as_deref(), Some("ILCE-1"));
    assert_eq!((i.width, i.height), (Some(8672), Some(5784)));
    assert_eq!(widths(&i), vec![160, 1616, 8640]);
    let big = i.pick(pw_raw::PreviewSize::Large).unwrap();
    assert_eq!(
        (big.offset, big.length, big.height),
        (544_768, 6_630_855, 5760)
    );
    assert_eq!(i.orientation, 1);
    let bytes = pw_raw::read_preview(&p, big).unwrap();
    assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
}

#[test]
fn sony_a7r_iva_orientation_8() {
    let Some(p) = file("TN100423.ARW") else {
        return;
    };
    let i = pw_raw::inspect(&p).unwrap();
    assert_eq!(i.model.as_deref(), Some("ILCE-7RM4A"));
    assert_eq!((i.width, i.height), (Some(9600), Some(6376)));
    assert_eq!(widths(&i), vec![160, 1616]);
    assert_eq!(i.orientation, 8);
    // The embedded preview has no EXIF orientation of its own.
    let mut bytes = pw_raw::read_preview(&p, i.pick(pw_raw::PreviewSize::Medium).unwrap()).unwrap();
    assert!(!pw_raw::neutralize_orientation(&mut bytes));
}

#[test]
fn panasonic_rw2() {
    let Some(p) = file("P1240231.RW2") else {
        return;
    };
    let i = pw_raw::inspect(&p).unwrap();
    assert_eq!(i.format, Some(pw_raw::Format::Panasonic));
    assert_eq!(i.model.as_deref(), Some("DMC-GM1"));
    assert_eq!((i.width, i.height), (Some(4592), Some(3448)));
    assert_eq!(widths(&i), vec![1920]);
    assert!(
        i.exposure_time.is_some(),
        "EXIF read from the embedded JPEG"
    );
}

#[test]
fn dng_preview_orientation_is_neutralized() {
    let Some(p) = file("IMG_20261026_062851.dng") else {
        return;
    };
    let i = pw_raw::inspect(&p).unwrap();
    assert_eq!((i.width, i.height), (Some(4096), Some(3072)));
    assert_eq!(widths(&i), vec![4096]);
    assert_eq!(i.orientation, 6);
    // This preview carries its own Orientation=6; WebView2 would rotate it a second time.
    let mut bytes = pw_raw::read_preview(&p, &i.previews[0]).unwrap();
    assert!(pw_raw::neutralize_orientation(&mut bytes));
}
