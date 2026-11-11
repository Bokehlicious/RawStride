//! Tests for the decoder: generated files (`tiff::encoder::TiffEncoder`, or by hand for a
//! palette TIFF), limits, the decode lock and error kinds.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use moxcms::ColorProfile;
use pw_colour::{Profile, TransformCache};
use tiff::encoder::compression::Lzw;
use tiff::encoder::{TiffEncoder, TiffValue, colortype};
use tiff::tags::Tag;

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

// --- decode_rgb8_for_display (ADR 0016 C3, T70) ---

/// (input, expected output), 8-bit RGB. Copied from `pw-colour`'s tests (T68); computed
/// independently of moxcms. Each channel must be within 1.
type Pair = ([u8; 3], [u8; 3]);

const SRGB_TO_ADOBE: [Pair; 8] = [
    ([255, 0, 0], [219, 0, 0]),
    ([0, 255, 0], [144, 255, 60]),
    ([0, 0, 255], [0, 0, 250]),
    ([255, 255, 255], [255, 255, 255]),
    ([0, 0, 0], [0, 0, 0]),
    ([128, 128, 128], [127, 127, 127]),
    ([200, 100, 50], [177, 100, 56]),
    ([30, 160, 220], [93, 159, 217]),
];

const PRO_PHOTO_TO_SRGB: [Pair; 7] = [
    ([255, 255, 255], [255, 255, 255]),
    ([0, 0, 0], [0, 0, 0]),
    ([128, 128, 128], [146, 146, 146]),
    ([150, 110, 90], [200, 118, 105]),
    ([100, 140, 120], [64, 165, 136]),
    ([180, 160, 140], [213, 171, 154]),
    ([120, 120, 160], [125, 139, 181]),
];

fn pro_photo_icc() -> Vec<u8> {
    ColorProfile::new_pro_photo_rgb().encode().unwrap()
}

fn gray_icc() -> Vec<u8> {
    ColorProfile::new_gray_with_gamma(2.2).encode().unwrap()
}

fn inputs(pairs: &[Pair]) -> Vec<u8> {
    pairs.iter().flat_map(|(input, _)| *input).collect()
}

fn inputs16(pairs: &[Pair]) -> Vec<u16> {
    inputs(pairs).iter().map(|&v| u16::from(v) * 257).collect()
}

/// Like `encode`, with the ICC profile written to tag 34675 (as type BYTE).
fn encode_icc<C: colortype::ColorType>(
    width: u32,
    height: u32,
    data: &[C::Inner],
    icc: &[u8],
) -> Vec<u8>
where
    [C::Inner]: TiffValue,
{
    let mut out = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut out).unwrap();
        let mut image = encoder.new_image::<C>(width, height).unwrap();
        image
            .encoder()
            .write_tag(Tag::Unknown(ICC_PROFILE_TAG), icc)
            .unwrap();
        image.write_data(data).unwrap();
    }
    out.into_inner()
}

/// An 8-bit RGB TIFF of `pixels.len() / 3` × 1 pixels with the ICC profile in tag 34675 as type
/// UNDEFINED (7), as Lightroom writes it, built by hand.
fn undefined_icc_tiff(pixels: &[u8], icc: &[u8]) -> Vec<u8> {
    const ENTRIES: u16 = 10;
    let width = (pixels.len() / 3) as u32;
    let ifd_end = 8 + 2 + 12 * u32::from(ENTRIES) + 4;
    let bits_at = ifd_end;
    let icc_at = bits_at + 6;
    let pixels_at = icc_at + icc.len() as u32 + (icc.len() as u32 % 2);
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
    const UNDEFINED: u16 = 7;
    entry(256, LONG, 1, width); // ImageWidth
    entry(257, SHORT, 1, 1); // ImageLength
    entry(258, SHORT, 3, bits_at); // BitsPerSample 8, 8, 8
    entry(259, SHORT, 1, 1); // Compression: none
    entry(262, SHORT, 1, 2); // PhotometricInterpretation: RGB
    entry(273, LONG, 1, pixels_at); // StripOffsets
    entry(277, SHORT, 1, 3); // SamplesPerPixel
    entry(278, SHORT, 1, 1); // RowsPerStrip
    entry(279, LONG, 1, pixels.len() as u32); // StripByteCounts
    entry(ICC_PROFILE_TAG, UNDEFINED, icc.len() as u32, icc_at); // InterColorProfile
    b.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    assert_eq!(b.len() as u32, bits_at);
    for _ in 0..3 {
        b.extend_from_slice(&8u16.to_le_bytes());
    }
    assert_eq!(b.len() as u32, icc_at);
    b.extend_from_slice(icc);
    b.resize(pixels_at as usize, 0);
    b.extend_from_slice(pixels);
    b
}

fn display(path: &Path, destination: &Profile) -> (Rgb8, SourceColour) {
    decode_rgb8_for_display(path, None, destination, &TransformCache::default()).unwrap()
}

/// Each channel of each pixel within 1 of the expected output.
fn assert_pairs(data: &[u8], pairs: &[Pair], what: &str) {
    assert_eq!(data.len(), pairs.len() * 3, "{what}: length");
    for (pixel, (input, expected)) in data.chunks(3).zip(pairs) {
        for c in 0..3 {
            assert!(
                pixel[c].abs_diff(expected[c]) <= 1,
                "{what}: {input:?} gave {pixel:?}, expected {expected:?} within 1"
            );
        }
    }
}

#[test]
fn rgb8_with_a_pro_photo_profile_is_converted_to_srgb() {
    let pixels = inputs(&PRO_PHOTO_TO_SRGB);
    let width = PRO_PHOTO_TO_SRGB.len() as u32;
    let path = temp_file(
        "icc-rgb8-prophoto.tif",
        &encode_icc::<colortype::RGB8>(width, 1, &pixels, &pro_photo_icc()),
    );
    let (image, colour) = display(&path, &Profile::srgb());
    assert_eq!(colour, SourceColour::Embedded);
    assert_eq!((image.width, image.height), (width, 1));
    assert_pairs(&image.data, &PRO_PHOTO_TO_SRGB, "8-bit ProPhoto → sRGB");
}

#[test]
fn an_undefined_icc_tag_as_lightroom_writes_it_is_read() {
    let pixels = inputs(&PRO_PHOTO_TO_SRGB);
    let path = temp_file(
        "icc-undefined.tif",
        &undefined_icc_tiff(&pixels, &pro_photo_icc()),
    );
    // Without colour management the hand-built file decodes to its own pixels.
    assert_eq!(decode(&path).data, pixels);
    let (image, colour) = display(&path, &Profile::srgb());
    assert_eq!(colour, SourceColour::Embedded);
    assert_pairs(
        &image.data,
        &PRO_PHOTO_TO_SRGB,
        "UNDEFINED tag ProPhoto → sRGB",
    );
}

#[test]
fn rgb16_with_a_pro_photo_profile_is_converted_to_srgb() {
    let samples = inputs16(&PRO_PHOTO_TO_SRGB);
    let width = PRO_PHOTO_TO_SRGB.len() as u32;
    let path = temp_file(
        "icc-rgb16-prophoto.tif",
        &encode_icc::<colortype::RGB16>(width, 1, &samples, &pro_photo_icc()),
    );
    let (image, colour) = display(&path, &Profile::srgb());
    assert_eq!(colour, SourceColour::Embedded);
    assert_pairs(&image.data, &PRO_PHOTO_TO_SRGB, "16-bit ProPhoto → sRGB");
}

#[test]
fn rgba16_with_a_pro_photo_profile_is_converted_and_drops_the_alpha() {
    let samples: Vec<u16> = inputs16(&PRO_PHOTO_TO_SRGB)
        .chunks(3)
        .flat_map(|p| [p[0], p[1], p[2], 0x1234])
        .collect();
    let width = PRO_PHOTO_TO_SRGB.len() as u32;
    let path = temp_file(
        "icc-rgba16-prophoto.tif",
        &encode_icc::<colortype::RGBA16>(width, 1, &samples, &pro_photo_icc()),
    );
    let (image, colour) = display(&path, &Profile::srgb());
    assert_eq!(colour, SourceColour::Embedded);
    assert_pairs(
        &image.data,
        &PRO_PHOTO_TO_SRGB,
        "16-bit RGBA ProPhoto → sRGB",
    );
}

#[test]
fn rgb8_without_a_profile_is_treated_as_srgb() {
    let pixels = inputs(&SRGB_TO_ADOBE);
    let width = SRGB_TO_ADOBE.len() as u32;
    let path = temp_file(
        "noicc-rgb8-adobe.tif",
        &encode::<colortype::RGB8>(width, 1, &pixels),
    );
    let (image, colour) = display(&path, &Profile::adobe_rgb());
    assert_eq!(colour, SourceColour::AssumedSrgb);
    assert_pairs(&image.data, &SRGB_TO_ADOBE, "sRGB (assumed) → Adobe RGB");
}

#[test]
fn without_a_profile_srgb_to_srgb_equals_decode_rgb8() {
    let rgb8: Vec<u8> = (0..=255u8).flat_map(|v| [v, 255 - v, v / 2]).collect();
    let rgb16: Vec<u16> = (0..256 * 3).map(|i| (i * 85 + 7) as u16).collect();
    let gray: Vec<u8> = (0..=255u8).collect();
    let cmyk: Vec<u8> = (0..256 * 4).map(|i| (i * 37 % 256) as u8).collect();
    let files = [
        ("id-rgb8.tif", encode::<colortype::RGB8>(16, 16, &rgb8)),
        ("id-rgb16.tif", encode::<colortype::RGB16>(16, 16, &rgb16)),
        ("id-gray8.tif", encode::<colortype::Gray8>(16, 16, &gray)),
        ("id-cmyk8.tif", encode::<colortype::CMYK8>(16, 16, &cmyk)),
    ];
    for (name, bytes) in files {
        let path = temp_file(name, &bytes);
        let (image, colour) = display(&path, &Profile::srgb());
        assert_eq!(colour, SourceColour::AssumedSrgb, "{name}");
        let plain = decode(&path);
        assert_eq!((image.width, image.height), (plain.width, plain.height));
        assert_eq!(image.data, plain.data, "{name}: not byte for byte");
    }
}

#[test]
fn a_garbage_or_gray_profile_on_an_rgb_file_is_unusable() {
    let pixels = inputs(&PRO_PHOTO_TO_SRGB);
    let width = PRO_PHOTO_TO_SRGB.len() as u32;
    let garbage: Vec<u8> = (0..100u32).map(|i| (i * 131 % 251) as u8).collect();
    for (name, icc) in [("bad-garbage.tif", garbage), ("bad-gray.tif", gray_icc())] {
        let path = temp_file(
            name,
            &encode_icc::<colortype::RGB8>(width, 1, &pixels, &icc),
        );
        let (image, colour) = display(&path, &Profile::srgb());
        assert!(
            matches!(colour, SourceColour::Unusable(_)),
            "{name}: expected Unusable, got {colour:?}"
        );
        assert_eq!(image.data, decode(&path).data, "{name}");
    }
}

#[test]
fn a_gray_file_with_a_gray_profile_is_treated_as_srgb() {
    let gray: Vec<u8> = (0..=255u8).collect();
    let path = temp_file(
        "gray-with-icc.tif",
        &encode_icc::<colortype::Gray8>(16, 16, &gray, &gray_icc()),
    );
    let (image, colour) = display(&path, &Profile::srgb());
    assert_eq!(colour, SourceColour::AssumedSrgb);
    assert_eq!(image.data, decode(&path).data);
}

#[test]
fn display_decode_errors_are_those_of_decode_rgb8() {
    let cache = TransformCache::default();
    let srgb = Profile::srgb();

    let (w, h) = (64u32, 64u32);
    let data = vec![100u8; (w * h * 3) as usize];
    let path = temp_file(
        "display-limit.tif",
        &encode_icc::<colortype::RGB8>(w, h, &data, &pro_photo_icc()),
    );
    let err = decode_rgb8_for_display(&path, Some(1024), &srgb, &cache).unwrap_err();
    assert_eq!(err, TiffError::TooLarge { limit: 1024 });
    assert_eq!(err, decode_rgb8(&path, Some(1024)).unwrap_err());

    let (w, h) = (32u32, 32u32);
    let data: Vec<u8> = (0..w * h * 3).map(|i| (i % 256) as u8).collect();
    let full = encode_icc::<colortype::RGB8>(w, h, &data, &pro_photo_icc());
    let path = temp_file("display-truncated.tif", &full[..full.len() / 2]);
    let err = decode_rgb8_for_display(&path, None, &srgb, &cache).unwrap_err();
    assert_eq!(err, decode_rgb8(&path, None).unwrap_err());

    let path = std::env::temp_dir().join("pw-tiff-display-does-not-exist.tif");
    let err = decode_rgb8_for_display(&path, None, &srgb, &cache).unwrap_err();
    assert!(matches!(err, TiffError::Io(_)), "expected Io, got {err:?}");
    assert_eq!(err, decode_rgb8(&path, None).unwrap_err());
}

#[test]
fn sixteen_bit_converted_before_the_reduction_keeps_more_steps() {
    // 4 096 dark grey values (0 to 4 095) as a 64 × 64 16-bit ProPhoto image, and the same ramp
    // reduced to 8 bit first (`v >> 8`, as `u16_to_u8`).
    let ramp: Vec<u16> = (0..4096u16).flat_map(|v| [v, v, v]).collect();
    let reduced = u16_to_u8(&ramp);
    let icc = pro_photo_icc();
    let wide = temp_file(
        "ramp16.tif",
        &encode_icc::<colortype::RGB16>(64, 64, &ramp, &icc),
    );
    let narrow = temp_file(
        "ramp8.tif",
        &encode_icc::<colortype::RGB8>(64, 64, &reduced, &icc),
    );
    let distinct = |data: &[u8]| data.iter().collect::<std::collections::BTreeSet<_>>().len();
    let (from_16, colour_16) = display(&wide, &Profile::srgb());
    let (from_8, colour_8) = display(&narrow, &Profile::srgb());
    assert_eq!(colour_16, SourceColour::Embedded);
    assert_eq!(colour_8, SourceColour::Embedded);
    let (n16, n8) = (distinct(&from_16.data), distinct(&from_8.data));
    println!("distinct 8-bit values: converted at 16 bit {n16}, reduced first {n8}");
    assert!(
        n16 >= n8,
        "the 16-bit path gave {n16} distinct values, the 8-bit path {n8}"
    );
}

/// Every TIFF (`.tif` or `.tiff`, any case) in `PW_TESTDATA`; at least one must exist.
fn real_tiff_files() -> Vec<PathBuf> {
    let directory = std::env::var_os("PW_TESTDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("PW_TESTDATA is not set"));
    let mut files: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("tif") || e.eq_ignore_ascii_case("tiff")
                })
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no .tif file in PW_TESTDATA: {}",
        directory.display()
    );
    files
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn real_tiffs_decode_with_their_embedded_profile() {
    let cache = TransformCache::default();
    for path in real_tiff_files() {
        let (image, colour) = decode_rgb8_for_display(&path, None, &Profile::srgb(), &cache)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        println!(
            "{}: {}×{}, {colour:?}",
            path.display(),
            image.width,
            image.height
        );
        assert_eq!(colour, SourceColour::Embedded, "{}", path.display());
    }
}
