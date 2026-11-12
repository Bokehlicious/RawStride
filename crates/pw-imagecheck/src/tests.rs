//! Unit tests of [`RawImageCheck`] on small files built here, in temporary folders. The embedded
//! preview is the 16 × 8 JPEG fixture of `pw-thumbcache`, held in a hand-built TIFF as
//! `JPEGInterchangeFormat`: `pw_raw::inspect` lists no embedded preview for a plain JPEG file
//! (accepted in the review, 2026-11-12).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pw_metadata::reconcile::FileKind;
use pw_metadata::safe_write::ImageCheck;

use super::RawImageCheck;

// The path is relative to `src/` (the folder of this file), hence two `..`.
#[path = "../../pw-thumbcache/src/jpeg_fixture.rs"]
mod jpeg_fixture;

/// A new, empty temporary folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pw-imagecheck-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A little-endian TIFF with IFD0 holding an image size (160 × 120, so `pw_raw::inspect` reads
/// the file also without a preview) and `Make`, and, with `thumbnail`, the JPEG fixture as
/// `JPEGInterchangeFormat` / `JPEGInterchangeFormatLength`.
fn tiff(thumbnail: bool) -> Vec<u8> {
    let jpeg = jpeg_fixture::SMALL;
    let make_at = 96u32;
    let jpeg_at = 128u32;
    let mut entries: Vec<(u16, u16, u32, u32)> = vec![
        (0x0100, 4, 1, 160),
        (0x0101, 4, 1, 120),
        (0x010F, 2, 5, make_at),
    ];
    if thumbnail {
        entries.push((0x0201, 4, 1, jpeg_at));
        entries.push((0x0202, 4, 1, jpeg.len() as u32));
    }
    let mut f = vec![0u8; jpeg_at as usize];
    f[0..4].copy_from_slice(b"II*\0");
    f[4..8].copy_from_slice(&8u32.to_le_bytes());
    f[8..10].copy_from_slice(&(entries.len() as u16).to_le_bytes());
    for (k, (tag, typ, count, value)) in entries.iter().enumerate() {
        let e = 10 + 12 * k;
        f[e..e + 2].copy_from_slice(&tag.to_le_bytes());
        f[e + 2..e + 4].copy_from_slice(&typ.to_le_bytes());
        f[e + 4..e + 8].copy_from_slice(&count.to_le_bytes());
        f[e + 8..e + 12].copy_from_slice(&value.to_le_bytes());
    }
    // The next-IFD offset after the entries stays 0.
    f[make_at as usize..make_at as usize + 5].copy_from_slice(b"TEST\0");
    if thumbnail {
        f.extend_from_slice(jpeg);
    }
    f
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let file = dir.join(name);
    std::fs::write(&file, bytes).unwrap();
    file
}

#[test]
fn the_test_tiff_has_one_embedded_preview() {
    let dir = temp_dir("fixture");
    let with = write(&dir, "with.tif", &tiff(true));
    let without = write(&dir, "without.tif", &tiff(false));
    let info = pw_raw::inspect(&with).unwrap();
    assert_eq!(info.previews.len(), 1);
    assert_eq!((info.previews[0].width, info.previews[0].height), (16, 8));
    assert_eq!(info.previews[0].length, jpeg_fixture::SMALL.len() as u64);
    let info = pw_raw::inspect(&without).unwrap();
    assert!(info.previews.is_empty());
    assert_eq!((info.width, info.height), (Some(160), Some(120)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_and_an_identical_copy_pass() {
    let dir = temp_dir("identical");
    let original = write(&dir, "bild.tif", &tiff(true));
    let copy = write(&dir, "bild.pwtmp-1.tif", &tiff(true));
    let check = RawImageCheck::without_release();
    assert_eq!(check.check_copy(&original, &copy, FileKind::Tiff), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_copy_with_one_byte_changed_in_its_thumbnail_is_refused() {
    let dir = temp_dir("changed");
    let original = write(&dir, "bild.tif", &tiff(true));
    let preview = pw_raw::inspect(&original).unwrap().previews.remove(0);
    let mut bytes = tiff(true);
    // A byte of the entropy-coded data, before the closing FF D9: the size still reads the same.
    let at = (preview.offset + preview.length - 3) as usize;
    bytes[at] ^= 0x01;
    let copy = write(&dir, "bild.pwtmp-1.tif", &bytes);
    let check = RawImageCheck::without_release();
    let error = check
        .check_copy(&original, &copy, FileKind::Tiff)
        .unwrap_err();
    assert_eq!(error, "preview 1 differs");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_copy_without_the_thumbnail_is_refused_with_the_count() {
    let dir = temp_dir("missing");
    let original = write(&dir, "bild.tif", &tiff(true));
    let copy = write(&dir, "bild.pwtmp-1.tif", &tiff(false));
    let check = RawImageCheck::without_release();
    let error = check
        .check_copy(&original, &copy, FileKind::Tiff)
        .unwrap_err();
    assert_eq!(error, "the copy has 0 previews, the original 1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_original_that_cannot_be_inspected_passes_whatever_the_copy_is() {
    let dir = temp_dir("uninspectable");
    let original = write(&dir, "bild.tif", b"not a tiff");
    assert!(pw_raw::inspect(&original).is_err());
    let check = RawImageCheck::without_release();
    for copy in [
        write(&dir, "text.tif", b"also not a tiff"),
        write(&dir, "with.tif", &tiff(true)),
        write(&dir, "without.tif", &tiff(false)),
        dir.join("missing.tif"),
    ] {
        assert_eq!(
            check.check_copy(&original, &copy, FileKind::Tiff),
            Ok(()),
            "{}",
            copy.display()
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_copy_that_cannot_be_inspected_is_refused() {
    let dir = temp_dir("copy-uninspectable");
    let original = write(&dir, "bild.tif", &tiff(true));
    let copy = write(&dir, "bild.pwtmp-1.tif", b"not a tiff");
    let check = RawImageCheck::without_release();
    let error = check
        .check_copy(&original, &copy, FileKind::Tiff)
        .unwrap_err();
    assert!(
        error.starts_with("pw-raw cannot inspect the copy: "),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Accepted in the review of 2026-11-12: for a plain JPEG, `pw_raw::inspect` lists the file itself
/// as its preview; a metadata write changes it, so it is not compared.
#[test]
fn a_jpeg_whose_own_bytes_changed_passes() {
    let dir = temp_dir("jpeg");
    let original = write(&dir, "bild.jpg", jpeg_fixture::SMALL);
    // The fixture with a COM segment after SOI, as a metadata write adds a segment.
    let mut bytes = jpeg_fixture::SMALL[..2].to_vec();
    bytes.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x06, b't', b'e', b's', b't']);
    bytes.extend_from_slice(&jpeg_fixture::SMALL[2..]);
    let copy = write(&dir, "bild.pwtmp-1.jpg", &bytes);
    let info = pw_raw::inspect(&copy).unwrap();
    assert_eq!(info.previews.len(), 1);
    assert_eq!(info.previews[0].offset, 0);
    let check = RawImageCheck::without_release();
    assert_eq!(check.check_copy(&original, &copy, FileKind::Jpeg), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A "DNG" LibRaw cannot open, against an original that does not open either: the write did not
/// make it worse.
#[test]
fn a_dng_copy_libraw_cannot_open_passes_when_the_original_does_not_open_either() {
    let dir = temp_dir("dng-unreadable");
    let original = write(&dir, "bild.dng", &tiff(true));
    let copy = write(&dir, "bild.pwtmp-1.dng", &tiff(true));
    assert!(pw_libraw::Session::open(&copy).is_err());
    let check = RawImageCheck::without_release();
    assert_eq!(check.check_copy(&original, &copy, FileKind::Dng), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn release_calls_the_closure_with_the_path() {
    let seen: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
    let check = RawImageCheck::new({
        let seen = Arc::clone(&seen);
        move |path: &Path| seen.lock().unwrap().push(path.to_path_buf())
    });
    check.release(Path::new("C:/bilder/bild ä.dng"));
    check.release(Path::new("C:/bilder/annan.jpg"));
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            PathBuf::from("C:/bilder/bild ä.dng"),
            PathBuf::from("C:/bilder/annan.jpg")
        ]
    );
}

#[test]
fn without_release_does_nothing() {
    let check = RawImageCheck::without_release();
    check.release(Path::new("C:/bilder/bild.dng"));
    assert_eq!(
        format!("{check:?}"),
        "RawImageCheck { release: false }".to_string()
    );
    let with = RawImageCheck::new(|_: &Path| {});
    assert_eq!(format!("{with:?}"), "RawImageCheck { release: true }");
}

#[test]
fn the_check_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RawImageCheck>();
}
