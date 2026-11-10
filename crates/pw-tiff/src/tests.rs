//! Tests for the decoder: generated files (`tiff::encoder::TiffEncoder`, or by hand for a
//! palette TIFF), limits, the decode lock and error kinds.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use tiff::encoder::compression::Lzw;
use tiff::encoder::{TiffEncoder, TiffValue, colortype};

use super::*;

fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pw-tiff-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn encode<C: colortype::ColorType>(width: u32, height: u32, data: &[C::Inner]) -> Vec<u8>
where
    [C::Inner]: TiffValue,
{
    let mut out = Cursor::new(Vec::new());
    TiffEncoder::new(&mut out)
        .unwrap()
        .write_image::<C>(width, height, data)
        .unwrap();
    out.into_inner()
}

fn encode_lzw<C: colortype::ColorType>(width: u32, height: u32, data: &[C::Inner]) -> Vec<u8>
where
    [C::Inner]: TiffValue,
{
    let mut out = Cursor::new(Vec::new());
    TiffEncoder::new(&mut out)
        .unwrap()
        .write_image_with_compression::<C, _>(width, height, Lzw, data)
        .unwrap();
    out.into_inner()
}

fn decode(path: &Path) -> Rgb8 {
    decode_rgb8(path, None).unwrap()
}

/// 2×2 RGB pixels, every byte different.
const RGB_2X2: [u8; 12] = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];

#[test]
fn rgb8_decodes_to_the_same_bytes() {
    let path = temp_file("rgb8.tif", &encode::<colortype::RGB8>(2, 2, &RGB_2X2));
    let img = decode(&path);
    assert_eq!((img.width, img.height), (2, 2));
    assert_eq!(img.data, RGB_2X2.to_vec());
}

#[test]
fn rgb16_decodes_to_the_top_bytes() {
    let samples: Vec<u16> = RGB_2X2.iter().map(|&v| u16::from(v) << 8 | 0xAB).collect();
    let path = temp_file("rgb16.tif", &encode::<colortype::RGB16>(2, 2, &samples));
    assert_eq!(decode(&path).data, RGB_2X2.to_vec());
}

#[test]
fn gray8_decodes_to_grey_rgb() {
    let path = temp_file(
        "gray8.tif",
        &encode::<colortype::Gray8>(2, 2, &[0, 64, 128, 255]),
    );
    assert_eq!(
        decode(&path).data,
        vec![0, 0, 0, 64, 64, 64, 128, 128, 128, 255, 255, 255]
    );
}

#[test]
fn rgba8_drops_the_alpha() {
    let rgba = [
        10, 20, 30, 1, 40, 50, 60, 2, 70, 80, 90, 3, 100, 110, 120, 4,
    ];
    let path = temp_file("rgba8.tif", &encode::<colortype::RGBA8>(2, 2, &rgba));
    assert_eq!(decode(&path).data, RGB_2X2.to_vec());
}

#[test]
fn cmyk8_is_converted_naively() {
    let cmyk = [
        0, 0, 0, 0, // white
        255, 255, 255, 0, // black
        0, 255, 0, 0, // magenta -> (255, 0, 255)
        0, 0, 0, 255, // full black
    ];
    let path = temp_file("cmyk8.tif", &encode::<colortype::CMYK8>(2, 2, &cmyk));
    assert_eq!(
        decode(&path).data,
        vec![255, 255, 255, 0, 0, 0, 255, 0, 255, 0, 0, 0]
    );
}

#[test]
fn rgb_float_is_clamped_to_0_1() {
    let samples = [
        0.0f32, 1.0, 0.5, //
        2.0, -1.0, 0.25, //
        1.0, 1.0, 1.0, //
        0.0, 0.0, 0.0,
    ];
    let path = temp_file(
        "rgbf32.tif",
        &encode::<colortype::RGB32Float>(2, 2, &samples),
    );
    assert_eq!(
        decode(&path).data,
        vec![0, 255, 128, 255, 0, 64, 255, 255, 255, 0, 0, 0]
    );
}

#[test]
fn lzw_decodes_like_its_uncompressed_twin() {
    let (w, h) = (16u32, 8u32);
    let data: Vec<u8> = (0..w * h * 3).map(|i| (i * 7 % 251) as u8).collect();
    let plain = temp_file("twin-plain.tif", &encode::<colortype::RGB8>(w, h, &data));
    let lzw = temp_file("twin-lzw.tif", &encode_lzw::<colortype::RGB8>(w, h, &data));
    assert_ne!(
        std::fs::read(&plain).unwrap(),
        std::fs::read(&lzw).unwrap(),
        "the LZW file must really be compressed differently"
    );
    let a = decode(&plain);
    let b = decode(&lzw);
    assert_eq!(a.data, data);
    assert_eq!(b.data, a.data);
}

#[test]
fn dimensions_give_the_encoded_size() {
    let path = temp_file("dims.tif", &encode::<colortype::Gray8>(5, 3, &[0u8; 15]));
    assert_eq!(dimensions(&path).unwrap(), (5, 3));
}

/// A 2×2 8-bit palette TIFF (PhotometricInterpretation 3 with a ColorMap), built by hand.
fn palette_tiff() -> Vec<u8> {
    const ENTRIES: u16 = 10;
    let ifd_end = 8 + 2 + 12 * u32::from(ENTRIES) + 4;
    let colormap_at = ifd_end;
    let pixels_at = colormap_at + 3 * 256 * 2;
    let mut b = Vec::new();
    b.extend_from_slice(b"II");
    b.extend_from_slice(&42u16.to_le_bytes());
    b.extend_from_slice(&8u32.to_le_bytes());
    b.extend_from_slice(&ENTRIES.to_le_bytes());
    let mut entry = |tag: u16, kind: u16, count: u32, value: u32| {
        b.extend_from_slice(&tag.to_le_bytes());
        b.extend_from_slice(&kind.to_le_bytes());
        b.extend_from_slice(&count.to_le_bytes());
        b.extend_from_slice(&value.to_le_bytes());
    };
    const SHORT: u16 = 3;
    const LONG: u16 = 4;
    entry(256, SHORT, 1, 2); // ImageWidth
    entry(257, SHORT, 1, 2); // ImageLength
    entry(258, SHORT, 1, 8); // BitsPerSample
    entry(259, SHORT, 1, 1); // Compression: none
    entry(262, SHORT, 1, 3); // PhotometricInterpretation: palette
    entry(273, LONG, 1, pixels_at); // StripOffsets
    entry(277, SHORT, 1, 1); // SamplesPerPixel
    entry(278, SHORT, 1, 2); // RowsPerStrip
    entry(279, LONG, 1, 4); // StripByteCounts
    entry(320, SHORT, 3 * 256, colormap_at); // ColorMap
    b.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    assert_eq!(b.len() as u32, colormap_at);
    for channel in 0..3u32 {
        for i in 0..256u32 {
            let v = ((i * (channel + 1)) % 256) as u16 * 257;
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    assert_eq!(b.len() as u32, pixels_at);
    b.extend_from_slice(&[0, 1, 2, 3]);
    b
}

#[test]
fn a_palette_tiff_is_unsupported() {
    let path = temp_file("palette.tif", &palette_tiff());
    let err = decode_rgb8(&path, None).unwrap_err();
    assert!(
        matches!(err, TiffError::Unsupported(_)),
        "expected Unsupported, got {err:?}"
    );
}

#[test]
fn a_truncated_file_gives_an_error() {
    let (w, h) = (32u32, 32u32);
    let data: Vec<u8> = (0..w * h * 3).map(|i| (i % 256) as u8).collect();
    let full = encode::<colortype::RGB8>(w, h, &data);
    let path = temp_file("truncated.tif", &full[..full.len() / 2]);
    assert!(decode_rgb8(&path, None).is_err());
}

#[test]
fn a_limit_smaller_than_the_image_gives_too_large() {
    let (w, h) = (64u32, 64u32);
    let data = vec![100u8; (w * h * 3) as usize];
    let path = temp_file("limit.tif", &encode::<colortype::RGB8>(w, h, &data));

    let err = decode_rgb8(&path, Some(1024)).unwrap_err();
    assert_eq!(err, TiffError::TooLarge { limit: 1024 });

    assert_eq!(decode_rgb8(&path, None).unwrap().data, data);
    assert_eq!(decode_rgb8(&path, Some(1024 * 1024)).unwrap().data, data);
}

#[test]
fn too_large_names_the_limit_in_mb() {
    let err = TiffError::TooLarge {
        limit: 512 * 1024 * 1024,
    };
    assert_eq!(
        err.to_string(),
        "TIFF: the file needs more memory than the limit of 512 MB"
    );
}

#[test]
fn a_decode_waits_for_the_decode_lock() {
    let path = temp_file("lock.tif", &encode::<colortype::RGB8>(2, 2, &RGB_2X2));
    let guard = hold_decode_lock();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _ = tx.send(decode_rgb8(&path, None));
    });
    assert!(matches!(
        rx.recv_timeout(Duration::from_millis(200)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    drop(guard);
    let img = rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
    assert_eq!(img.data, RGB_2X2.to_vec());
    worker.join().unwrap();
}

#[test]
fn dimensions_do_not_take_the_decode_lock() {
    let path = temp_file("lock-dims.tif", &encode::<colortype::RGB8>(2, 2, &RGB_2X2));
    let _guard = hold_decode_lock();
    assert_eq!(dimensions(&path).unwrap(), (2, 2));
}

#[test]
fn a_missing_file_gives_io() {
    let path = std::env::temp_dir().join("pw-tiff-does-not-exist.tif");
    let err = decode_rgb8(&path, None).unwrap_err();
    assert!(matches!(err, TiffError::Io(_)), "expected Io, got {err:?}");
    assert!(err.to_string().starts_with(&path.display().to_string()));
    assert!(matches!(dimensions(&path), Err(TiffError::Io(_))));
}
