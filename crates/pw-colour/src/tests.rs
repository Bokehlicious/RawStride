//! Tests with reference values computed independently of `moxcms` (floating point, Bradford
//! adaptation, sRGB piecewise curve, Adobe RGB gamma 563/256, ProPhoto gamma 1.8). Each channel
//! must be within 1 of the reference (ADR 0016 F1, §8.1). No file from the owner's computer.

use std::sync::Arc;

use moxcms::ColorProfile;

use super::*;

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

fn adobe_rgb_icc() -> Vec<u8> {
    match ColorProfile::new_adobe_rgb().encode() {
        Ok(bytes) => bytes,
        Err(e) => panic!("encoding the Adobe RGB profile failed: {e}"),
    }
}

fn gray_icc() -> Vec<u8> {
    match ColorProfile::new_gray_with_gamma(2.2).encode() {
        Ok(bytes) => bytes,
        Err(e) => panic!("encoding the gray profile failed: {e}"),
    }
}

fn transform(source: &Profile, destination: &Profile) -> Transform {
    match Transform::new(source, destination) {
        Ok(t) => t,
        Err(e) => panic!("building the transform failed: {e}"),
    }
}

/// Every channel of `actual` is within 1 of `expected`.
fn assert_within_one(input: [u8; 3], actual: [u8; 3], expected: [u8; 3], what: &str) {
    let close = actual
        .iter()
        .zip(expected.iter())
        .all(|(a, e)| a.abs_diff(*e) <= 1);
    assert!(
        close,
        "{what}: {input:?} gave {actual:?}, reference {expected:?}"
    );
}

fn convert8(t: &Transform, input: [u8; 3]) -> [u8; 3] {
    let mut px = input;
    if let Err(e) = t.convert_rgb8(&mut px) {
        panic!("converting {input:?} failed: {e}");
    }
    px
}

fn check_pairs_8bit(t: &Transform, pairs: &[Pair], what: &str) {
    for &(input, expected) in pairs {
        assert_within_one(input, convert8(t, input), expected, what);
    }
}

/// The input colours of both tables plus all 256 grey values.
fn greys_and_colours() -> Vec<u8> {
    let mut data: Vec<u8> = (0..=255u8).flat_map(|v| [v, v, v]).collect();
    for (input, _) in SRGB_TO_ADOBE.iter().chain(PRO_PHOTO_TO_SRGB.iter()) {
        data.extend_from_slice(input);
    }
    data
}

#[test]
fn srgb_to_adobe_rgb_8_bit_matches_the_references() {
    let t = transform(&Profile::srgb(), &Profile::adobe_rgb());
    assert!(!t.is_identity());
    check_pairs_8bit(&t, &SRGB_TO_ADOBE, "sRGB → Adobe RGB");
}

#[test]
fn pro_photo_to_srgb_8_bit_matches_the_references() {
    let t = transform(&Profile::pro_photo_rgb(), &Profile::srgb());
    check_pairs_8bit(&t, &PRO_PHOTO_TO_SRGB, "ProPhoto → sRGB");
}

#[test]
fn pro_photo_to_srgb_16_bit_reduced_to_8_bit_matches_the_references() {
    let t = transform(&Profile::pro_photo_rgb(), &Profile::srgb());
    let mut data: Vec<u16> = PRO_PHOTO_TO_SRGB
        .iter()
        .flat_map(|(input, _)| input.map(|v| u16::from(v) * 257))
        .collect();
    if let Err(e) = t.convert_rgb16(&mut data) {
        panic!("16-bit conversion failed: {e}");
    }
    let (pixels, rest) = data.as_chunks::<3>();
    assert!(rest.is_empty());
    assert_eq!(pixels.len(), PRO_PHOTO_TO_SRGB.len());
    for ((input, expected), px) in PRO_PHOTO_TO_SRGB.iter().zip(pixels) {
        // Reduced as `pw_tiff::u16_to_u8` does.
        let reduced = px.map(|v| (v >> 8) as u8);
        assert_within_one(*input, reduced, *expected, "ProPhoto → sRGB, 16 bit");
    }
}

#[test]
fn the_same_built_in_profile_is_an_identity_that_leaves_the_data_unchanged() {
    let t = transform(&Profile::srgb(), &Profile::srgb());
    assert!(t.is_identity());
    check_identity(&t);
}

#[test]
fn two_profiles_from_the_same_icc_bytes_are_an_identity() {
    let bytes = adobe_rgb_icc();
    let (a, b) = match (Profile::from_icc(&bytes), Profile::from_icc(&bytes)) {
        (Ok(a), Ok(b)) => (a, b),
        _ => panic!("parsing the Adobe RGB ICC bytes failed"),
    };
    let t = transform(&a, &b);
    assert!(t.is_identity());
    check_identity(&t);
}

fn check_identity(t: &Transform) {
    let original = greys_and_colours();
    let mut data8 = original.clone();
    assert_eq!(t.convert_rgb8(&mut data8), Ok(()));
    assert_eq!(data8, original, "8 bit changed");

    let original16: Vec<u16> = original.iter().map(|&v| u16::from(v) * 257).collect();
    let mut data16 = original16.clone();
    assert_eq!(t.convert_rgb16(&mut data16), Ok(()));
    assert_eq!(data16, original16, "16 bit changed");
}

#[test]
fn an_adobe_rgb_profile_from_icc_bytes_converts_like_the_built_in_one() {
    let adobe = match Profile::from_icc(&adobe_rgb_icc()) {
        Ok(p) => p,
        Err(e) => panic!("parsing the Adobe RGB ICC bytes failed: {e}"),
    };
    let t = transform(&Profile::srgb(), &adobe);
    assert!(!t.is_identity());
    check_pairs_8bit(&t, &SRGB_TO_ADOBE, "sRGB → Adobe RGB from ICC bytes");
}

#[test]
fn bad_profiles_give_errors() {
    let garbage: Vec<u8> = (0..100u32).map(|i| (i * 37 % 251) as u8).collect();
    let full = adobe_rgb_icc();
    let half = full.get(..full.len() / 2).unwrap_or_default();
    for (what, bytes) in [
        ("empty", &[][..]),
        ("garbage", &garbage[..]),
        ("half", half),
    ] {
        assert!(
            Profile::from_icc(bytes).is_err(),
            "{what} bytes gave a profile"
        );
    }

    match Profile::from_icc(&gray_icc()) {
        Err(ColourError::Unsupported(space)) => {
            assert_eq!(space, "Gray");
            assert_eq!(
                ColourError::Unsupported(space).to_string(),
                "ICC profile is not RGB (Gray)"
            );
        }
        other => panic!("a gray profile gave {other:?}"),
    }
}

#[test]
fn error_texts() {
    assert_eq!(
        ColourError::Profile("x".into()).to_string(),
        "ICC profile: x"
    );
    assert_eq!(
        ColourError::Transform("y".into()).to_string(),
        "colour transform: y"
    );
    assert_eq!(
        ColourError::Length { len: 4 }.to_string(),
        "pixel data of 4 values is not whole RGB pixels"
    );
}

#[test]
fn a_length_that_is_not_whole_pixels_is_refused_and_leaves_the_data_unchanged() {
    for t in [
        transform(&Profile::srgb(), &Profile::adobe_rgb()),
        transform(&Profile::srgb(), &Profile::srgb()),
    ] {
        let mut data8 = [10u8, 20, 30, 40];
        assert_eq!(
            t.convert_rgb8(&mut data8),
            Err(ColourError::Length { len: 4 })
        );
        assert_eq!(data8, [10, 20, 30, 40]);

        let mut data16 = [1000u16, 2000, 3000, 4000];
        assert_eq!(
            t.convert_rgb16(&mut data16),
            Err(ColourError::Length { len: 4 })
        );
        assert_eq!(data16, [1000, 2000, 3000, 4000]);

        assert_eq!(t.convert_rgb8(&mut []), Ok(()));
        assert_eq!(t.convert_rgb16(&mut []), Ok(()));
    }
}

#[test]
fn chunked_conversion_equals_converting_each_pixel_on_its_own() {
    let colours: Vec<[u8; 3]> = SRGB_TO_ADOBE
        .iter()
        .chain(PRO_PHOTO_TO_SRGB.iter())
        .map(|(input, _)| *input)
        .collect();
    let pixels = CHUNK_PIXELS * 2 + 7;
    let image: Vec<[u8; 3]> = colours.iter().copied().cycle().take(pixels).collect();

    let t = transform(&Profile::srgb(), &Profile::adobe_rgb());
    let mut whole: Vec<u8> = image.iter().flatten().copied().collect();
    assert_eq!(t.convert_rgb8(&mut whole), Ok(()));
    let one_by_one: Vec<u8> = image.iter().flat_map(|&px| convert8(&t, px)).collect();
    assert_eq!(whole.len(), pixels * 3);
    assert!(whole == one_by_one, "8-bit chunked result differs");

    let mut whole16: Vec<u16> = image
        .iter()
        .flatten()
        .map(|&v| u16::from(v) * 257)
        .collect();
    assert_eq!(t.convert_rgb16(&mut whole16), Ok(()));
    let mut one_by_one16 = Vec::with_capacity(pixels * 3);
    for px in &image {
        let mut p = px.map(|v| u16::from(v) * 257);
        assert_eq!(t.convert_rgb16(&mut p), Ok(()));
        one_by_one16.extend_from_slice(&p);
    }
    assert!(whole16 == one_by_one16, "16-bit chunked result differs");
}

#[test]
fn the_cache_builds_each_pair_once() {
    let cache = TransformCache::default();
    let (srgb, adobe, pro_photo) = (
        Profile::srgb(),
        Profile::adobe_rgb(),
        Profile::pro_photo_rgb(),
    );
    let (a, b, c) = match (
        cache.get(&srgb, &adobe),
        cache.get(&srgb, &adobe),
        cache.get(&pro_photo, &srgb),
    ) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        _ => panic!("building a cached transform failed"),
    };
    assert!(Arc::ptr_eq(&a, &b));
    assert!(!Arc::ptr_eq(&a, &c));
}

#[test]
fn the_cache_returns_a_failed_build_and_does_not_keep_it() {
    // `from_icc` refuses a gray profile, so the pair is built by hand to reach the transform.
    let bytes = gray_icc();
    let gray_profile = match ColorProfile::new_from_slice(&bytes) {
        Ok(p) => p,
        Err(e) => panic!("parsing the gray ICC bytes failed: {e}"),
    };
    let gray = Profile {
        id: ProfileId::Icc(Arc::from(&bytes[..])),
        profile: Arc::new(gray_profile),
    };

    let cache = TransformCache::default();
    match cache.get(&gray, &Profile::srgb()) {
        Err(ColourError::Transform(_)) => {}
        other => panic!("a gray source gave {other:?}"),
    }
    let cached = match cache.transforms.lock() {
        Ok(map) => map.len(),
        Err(e) => panic!("cache lock poisoned: {e}"),
    };
    assert_eq!(cached, 0);
}

#[test]
fn transforms_and_the_cache_can_be_shared_between_threads() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Transform>();
    send_sync::<TransformCache>();
    send_sync::<Profile>();
}
