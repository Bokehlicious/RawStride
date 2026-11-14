use std::fmt;
use std::fs;
use std::path::Path;

use pw_colour::{Profile, TransformCache, jpeg_source_profile};
use pw_libjpeg::{Decoder, Rgb8, Scale};
use pw_raw::{ExifColourSpace, Format, ImageInfo, PreviewRef};

/// Selects the smallest embedded preview whose long edge reaches `cell`, or the largest preview
/// when none is large enough.
pub fn choose_preview(info: &ImageInfo, cell: u32) -> Option<&PreviewRef> {
    info.previews
        .iter()
        .filter(|preview| preview.width.max(preview.height) >= cell)
        .min_by_key(|preview| preview.width.max(preview.height))
        .or_else(|| {
            info.previews
                .iter()
                .max_by_key(|preview| preview.width.max(preview.height))
        })
}

/// Errors while extracting a thumbnail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThumbError {
    /// The file has no embedded preview that can be used as a thumbnail.
    NoPreview,
    /// The file could not be read, inspected or decoded.
    Unreadable(String),
}

impl fmt::Display for ThumbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThumbError::NoPreview => f.write_str("no preview"),
            ThumbError::Unreadable(message) => write!(f, "unreadable thumbnail: {message}"),
        }
    }
}

impl std::error::Error for ThumbError {}

/// Chooses the smallest decode scale whose long edge still reaches `cell`.
///
/// `cell` is in physical pixels: on a high-DPI screen the caller multiplies the cell size in
/// points by the display scale before asking for a thumbnail.
pub fn choose_scale(width: u32, height: u32, cell: u32) -> Scale {
    let target = cell;
    [Scale::Eighth, Scale::Quarter, Scale::Half, Scale::Full]
        .into_iter()
        .find(|scale| {
            let (scaled_width, scaled_height) = scale.size(width, height);
            scaled_width.max(scaled_height) >= target
        })
        .unwrap_or(Scale::Full)
}

/// Applies the EXIF orientation to the decoded pixels. Unknown values leave the image unchanged.
pub fn orient(image: Rgb8, orientation: u16) -> Rgb8 {
    if !(2..=8).contains(&orientation) {
        return image;
    }

    let (width, height) = (image.width, image.height);
    let (output_width, output_height) = if matches!(orientation, 5..=8) {
        (height, width)
    } else {
        (width, height)
    };
    let Some(data_len) = width
        .checked_mul(height)
        .and_then(|count| usize::try_from(count).ok())
        .and_then(|count| count.checked_mul(3))
    else {
        return image;
    };
    if image.data.len() != data_len {
        return image;
    }

    let Some(output_len) = output_width
        .checked_mul(output_height)
        .and_then(|count| usize::try_from(count).ok())
        .and_then(|count| count.checked_mul(3))
    else {
        return image;
    };
    let mut data = Vec::new();
    if data.try_reserve_exact(output_len).is_err() {
        return image;
    }

    for y in 0..output_height {
        for x in 0..output_width {
            let (source_x, source_y) = match orientation {
                2 => (width - 1 - x, y),
                3 => (width - 1 - x, height - 1 - y),
                4 => (x, height - 1 - y),
                5 => (y, x),
                6 => (y, height - 1 - x),
                7 => (width - 1 - y, height - 1 - x),
                8 => (width - 1 - y, x),
                _ => (x, y),
            };
            let Some(source_offset) = source_y
                .checked_mul(width)
                .and_then(|offset| offset.checked_add(source_x))
                .and_then(|offset| usize::try_from(offset).ok())
                .and_then(|offset| offset.checked_mul(3))
            else {
                return image;
            };
            let Some(source_end) = source_offset.checked_add(3) else {
                return image;
            };
            let Some(pixel) = image.data.get(source_offset..source_end) else {
                return image;
            };
            data.extend_from_slice(pixel);
        }
    }

    Rgb8 {
        width: output_width,
        height: output_height,
        data,
    }
}

/// Memory limit of one TIFF decode for a thumbnail: the default core budget. The thumbnail worker
/// does not get the user's budget (ADR 0015 §9.1).
const TIFF_LIMIT: usize = 4 << 30;

/// Extracts and orients a thumbnail without decoding RAW image data, converted from its source
/// profile to `destination` (the monitor profile, ADR 0016 §4.1 "Thumbnails").
///
/// A JPEG (a plain file or an embedded preview) is converted from its APP2 ICC profile, else the
/// Exif Adobe RGB marker, else sRGB (ADR 0016 §3.1), after the scaled decode. A TIFF is decoded
/// from its image data, converted from its embedded profile and reduced to the cell with a box
/// filter; its embedded previews are not used (ADR 0015 §4.1, Q7). With destination sRGB, sRGB
/// sources are left unchanged (F3).
pub fn thumbnail(
    path: &Path,
    cell: u32,
    decoder: &mut Decoder,
    destination: &Profile,
    cache: &TransformCache,
) -> Result<Rgb8, ThumbError> {
    if pw_raw::is_tiff_file(path) {
        return tiff_thumbnail(path, cell, destination, cache);
    }
    let info = pw_raw::inspect(path).map_err(|error| ThumbError::Unreadable(error.to_string()))?;
    let bytes = if info.format == Some(Format::Jpeg) {
        fs::read(path).map_err(|error| ThumbError::Unreadable(error.to_string()))?
    } else {
        let preview = choose_preview(&info, cell).ok_or(ThumbError::NoPreview)?;
        pw_raw::read_preview(path, preview)
            .map_err(|error| ThumbError::Unreadable(error.to_string()))?
    };
    let icc = decoder
        .icc_profile(&bytes)
        .map_err(|error| ThumbError::Unreadable(error.to_string()))?;
    let (width, height) = decoder
        .header(&bytes)
        .map_err(|error| ThumbError::Unreadable(error.to_string()))?;
    let scale = choose_scale(width, height, cell);
    let mut image = decoder
        .decode(&bytes, scale)
        .map_err(|error| ThumbError::Unreadable(error.to_string()))?;
    let source = jpeg_source_profile(
        icc.as_deref(),
        info.colour_space == ExifColourSpace::AdobeRgb,
    );
    // the thumbnail is small, so converting it after the scaled decode is cheap; a source profile
    // no transform can be built from is treated as sRGB
    cache
        .get(&source, destination)
        .or_else(|_| cache.get(&Profile::srgb(), destination))
        .and_then(|transform| transform.convert_rgb8(&mut image.data))
        .map_err(|error| ThumbError::Unreadable(error.to_string()))?;
    Ok(orient(image, info.orientation))
}

/// Decodes a TIFF with `pw-tiff` under `TIFF_LIMIT` and converts it to `destination`, reduces it
/// so the long edge is at most `cell` (never upscales) and applies the orientation of
/// `pw_raw::inspect`, or none when inspect fails (for example a BigTIFF).
fn tiff_thumbnail(
    path: &Path,
    cell: u32,
    destination: &Profile,
    cache: &TransformCache,
) -> Result<Rgb8, ThumbError> {
    let (full, _source) =
        pw_tiff::decode_rgb8_for_display(path, Some(TIFF_LIMIT), destination, cache)
            .map_err(|error| ThumbError::Unreadable(error.to_string()))?;
    let (width, height) = pw_tiff::fit(full.width, full.height, cell);
    let small = pw_tiff::downscale(&full, width, height);
    drop(full);
    let image = Rgb8 {
        width: small.width,
        height: small.height,
        data: small.data,
    };
    let orientation = pw_raw::inspect(path).map_or(1, |info| info.orientation);
    Ok(orient(image, orientation))
}

#[cfg(test)]
#[path = "jpeg_fixture.rs"]
mod jpeg_fixture;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

    struct TempFile(PathBuf);

    impl TempFile {
        fn new(extension: &str, bytes: &[u8]) -> Self {
            let id = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pw-thumbcache-{}-{id}.{extension}",
                std::process::id()
            ));
            fs::write(&path, bytes).unwrap_or_else(|error| panic!("write test image: {error}"));
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn preview(width: u32, height: u32) -> PreviewRef {
        PreviewRef {
            offset: 0,
            length: 0,
            width,
            height,
            source: String::new(),
        }
    }

    #[test]
    fn chooses_smallest_preview_large_enough() {
        let info = ImageInfo {
            previews: vec![preview(800, 600), preview(320, 240), preview(160, 120)],
            ..ImageInfo::default()
        };

        assert_eq!(choose_preview(&info, 200).map(|p| p.width), Some(320));
    }

    #[test]
    fn preview_exactly_at_cell_long_edge_is_large_enough() {
        let info = ImageInfo {
            previews: vec![preview(199, 100), preview(200, 80), preview(240, 160)],
            ..ImageInfo::default()
        };

        assert_eq!(
            choose_preview(&info, 200).map(|p| (p.width, p.height)),
            Some((200, 80))
        );
    }

    #[test]
    fn chooses_largest_preview_when_none_is_large_enough() {
        let info = ImageInfo {
            previews: vec![preview(160, 120), preview(800, 600), preview(320, 240)],
            ..ImageInfo::default()
        };

        assert_eq!(choose_preview(&info, 1000).map(|p| p.width), Some(800));
    }

    #[test]
    fn no_previews_returns_none() {
        assert!(choose_preview(&ImageInfo::default(), 200).is_none());
    }

    #[test]
    fn portrait_preview_uses_height_as_long_edge() {
        let info = ImageInfo {
            previews: vec![preview(160, 320), preview(240, 400), preview(120, 600)],
            ..ImageInfo::default()
        };

        assert_eq!(
            choose_preview(&info, 300).map(|p| (p.width, p.height)),
            Some((160, 320))
        );
    }

    #[test]
    fn choose_scale_selects_required_resolution_and_exact_boundaries() {
        // 1616 / 8 = 202 reaches 200; 1616 / 8 does not reach 300, 1616 / 4 = 404 does.
        assert_eq!(choose_scale(1616, 1080, 200), Scale::Eighth);
        assert_eq!(choose_scale(1616, 1080, 300), Scale::Quarter);
        assert_eq!(choose_scale(160, 120, 200), Scale::Full);
        assert_eq!(choose_scale(1080, 1616, 200), Scale::Eighth);
        // Exact boundaries: 1600 / 8 = 200 is enough; 1592 / 8 = 199 is not.
        assert_eq!(choose_scale(1600, 1080, 200), Scale::Eighth);
        assert_eq!(choose_scale(1592, 1080, 200), Scale::Quarter);
    }

    #[test]
    fn orientations_transform_dimensions_and_pixels() {
        let source = Rgb8 {
            width: 2,
            height: 3,
            data: (1..=6).flat_map(|value| [value, 0, 0]).collect(),
        };
        let cases: [(u16, u32, u32, [u8; 6]); 8] = [
            (1, 2, 3, [1, 2, 3, 4, 5, 6]),
            (2, 2, 3, [2, 1, 4, 3, 6, 5]),
            (3, 2, 3, [6, 5, 4, 3, 2, 1]),
            (4, 2, 3, [5, 6, 3, 4, 1, 2]),
            (5, 3, 2, [1, 3, 5, 2, 4, 6]),
            (6, 3, 2, [5, 3, 1, 6, 4, 2]),
            (7, 3, 2, [6, 4, 2, 5, 3, 1]),
            (8, 3, 2, [2, 4, 6, 1, 3, 5]),
        ];

        for (orientation, width, height, expected) in cases {
            let image = orient(source.clone(), orientation);
            assert_eq!((image.width, image.height), (width, height));
            let actual: Vec<_> = image
                .data
                .as_chunks::<3>()
                .0
                .iter()
                .map(|pixel| pixel[0])
                .collect();
            assert_eq!(actual, expected, "orientation {orientation}");
        }
    }

    #[test]
    fn unknown_orientation_leaves_image_unchanged() {
        let image = Rgb8 {
            width: 1,
            height: 1,
            data: vec![1, 2, 3],
        };
        assert_eq!(orient(image.clone(), 9), image);
    }

    #[test]
    fn thumbnail_reports_missing_and_invalid_files_as_unreadable() {
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));
        let missing =
            std::env::temp_dir().join(format!("pw-thumbcache-missing-{}.jpg", std::process::id()));
        assert!(matches!(
            thumbnail(
                &missing,
                200,
                &mut decoder,
                &Profile::srgb(),
                &TransformCache::default()
            ),
            Err(ThumbError::Unreadable(_))
        ));

        let invalid = TempFile::new("img", b"not an image");
        assert!(matches!(
            thumbnail(
                invalid.path(),
                200,
                &mut decoder,
                &Profile::srgb(),
                &TransformCache::default()
            ),
            Err(ThumbError::Unreadable(_))
        ));
    }

    #[test]
    fn plain_jpeg_file_decodes_to_expected_size() {
        let file = TempFile::new("jpg", jpeg_fixture::SMALL);
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));

        let image = thumbnail(
            file.path(),
            200,
            &mut decoder,
            &Profile::srgb(),
            &TransformCache::default(),
        )
        .unwrap_or_else(|error| panic!("thumbnail failed: {error}"));
        assert_eq!((image.width, image.height), (16, 8));
    }

    // ---------- colour conversion to the monitor profile (ADR 0016 C4) ----------

    fn jpeg_thumbnail(destination: &Profile) -> Rgb8 {
        let file = TempFile::new("jpg", jpeg_fixture::SMALL);
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));
        thumbnail(
            file.path(),
            200,
            &mut decoder,
            destination,
            &TransformCache::default(),
        )
        .unwrap_or_else(|error| panic!("thumbnail failed: {error}"))
    }

    #[test]
    fn a_jpeg_thumbnail_is_converted_to_the_destination() {
        let srgb_thumb = jpeg_thumbnail(&Profile::srgb());
        let adobe = Profile::adobe_rgb();
        let adobe_thumb = jpeg_thumbnail(&adobe);

        let mut expected = srgb_thumb.data.clone();
        TransformCache::default()
            .get(&Profile::srgb(), &adobe)
            .and_then(|transform| transform.convert_rgb8(&mut expected))
            .unwrap_or_else(|error| panic!("{error}"));
        assert_ne!(
            expected, srgb_thumb.data,
            "the conversion changes the fixture"
        );
        assert_eq!(
            (adobe_thumb.width, adobe_thumb.height),
            (srgb_thumb.width, srgb_thumb.height)
        );
        assert_eq!(adobe_thumb.data, expected);
    }

    #[test]
    fn a_jpeg_thumbnail_in_srgb_equals_the_plain_decode() {
        // today's path: the scaled decode, then the orientation (1 here)
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));
        let scale = choose_scale(16, 8, 200);
        let plain = decoder
            .decode(jpeg_fixture::SMALL, scale)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(jpeg_thumbnail(&Profile::srgb()), plain);
    }

    fn test_file(name: &str) -> PathBuf {
        let directory = std::env::var_os("PW_TESTDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| panic!("PW_TESTDATA is not set"));
        let path = directory.join(name);
        assert!(path.is_file(), "missing real test file: {}", path.display());
        path
    }

    #[test]
    #[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
    fn real_sony_arw_thumbnail() {
        let path = test_file("TN005956.ARW");
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));

        let image = thumbnail(
            &path,
            200,
            &mut decoder,
            &Profile::srgb(),
            &TransformCache::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(image.width > 0 && image.height > 0);
    }

    #[test]
    #[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
    fn real_panasonic_rw2_thumbnail() {
        let path = test_file("P1240231.RW2");
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));

        let image = thumbnail(
            &path,
            200,
            &mut decoder,
            &Profile::srgb(),
            &TransformCache::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(image.width > 0 && image.height > 0);
    }

    #[test]
    #[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
    fn real_dng_thumbnail() {
        let path = test_file("IMG_20261026_062851.dng");
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));

        let image = thumbnail(
            &path,
            200,
            &mut decoder,
            &Profile::srgb(),
            &TransformCache::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(image.width > 0 && image.height > 0);
    }

    #[test]
    #[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
    fn real_a7r_iva_orientation_8_thumbnail_has_swapped_dimensions() {
        let path = test_file("TN100423.ARW");
        let info = pw_raw::inspect(&path).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(info.orientation, 8);
        let preview = choose_preview(&info, 200).unwrap_or_else(|| panic!("no embedded preview"));
        let (width, height) =
            choose_scale(preview.width, preview.height, 200).size(preview.width, preview.height);
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));

        let image = thumbnail(
            &path,
            200,
            &mut decoder,
            &Profile::srgb(),
            &TransformCache::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!((image.width, image.height), (height, width));
    }

    mod tiff_files {
        use std::io::Cursor;
        use std::path::{Path, PathBuf};

        use tiff::encoder::{TiffEncoder, colortype};
        use tiff::tags::Tag;

        use super::super::*;

        /// Writes `bytes` to `name` in a test folder of its own.
        fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
            let dir =
                std::env::temp_dir().join(format!("pw-thumbcache-tiff-{}", std::process::id()));
            std::fs::create_dir_all(&dir)
                .unwrap_or_else(|error| panic!("create test folder: {error}"));
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap_or_else(|error| panic!("write test file: {error}"));
            path
        }

        /// The colour of every pixel of the uniform test image.
        const COLOUR: [u8; 3] = [200, 100, 50];

        fn uniform(width: u32, height: u32) -> Vec<u8> {
            COLOUR.repeat((width * height) as usize)
        }

        /// An 8-bit RGB TIFF with optional Orientation and JPEGInterchangeFormat (offset, length)
        /// tags in IFD0.
        fn rgb_tiff(
            width: u32,
            height: u32,
            data: &[u8],
            orientation: Option<u16>,
            jpeg: Option<(u32, u32)>,
        ) -> Vec<u8> {
            let mut out = Cursor::new(Vec::new());
            let mut encoder =
                TiffEncoder::new(&mut out).unwrap_or_else(|error| panic!("encoder: {error}"));
            let mut image = encoder
                .new_image::<colortype::RGB8>(width, height)
                .unwrap_or_else(|error| panic!("new image: {error}"));
            if let Some(o) = orientation {
                image
                    .encoder()
                    .write_tag(Tag::Orientation, o)
                    .unwrap_or_else(|error| panic!("orientation tag: {error}"));
            }
            if let Some((offset, length)) = jpeg {
                image
                    .encoder()
                    .write_tag(Tag::Unknown(0x0201), offset)
                    .unwrap_or_else(|error| panic!("JPEGInterchangeFormat tag: {error}"));
                image
                    .encoder()
                    .write_tag(Tag::Unknown(0x0202), length)
                    .unwrap_or_else(|error| panic!("JPEGInterchangeFormatLength tag: {error}"));
            }
            image
                .write_data(data)
                .unwrap_or_else(|error| panic!("write data: {error}"));
            out.into_inner()
        }

        fn thumb(path: &Path, cell: u32) -> Result<Rgb8, ThumbError> {
            let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));
            thumbnail(
                path,
                cell,
                &mut decoder,
                &Profile::srgb(),
                &TransformCache::default(),
            )
        }

        // ---------- colour conversion to the monitor profile (ADR 0016 C4) ----------

        /// An 8-bit RGB TIFF with the ICC profile `icc` in tag 34675.
        fn rgb_tiff_with_icc(width: u32, height: u32, data: &[u8], icc: &[u8]) -> Vec<u8> {
            let mut out = Cursor::new(Vec::new());
            let mut encoder =
                TiffEncoder::new(&mut out).unwrap_or_else(|error| panic!("encoder: {error}"));
            let mut image = encoder
                .new_image::<colortype::RGB8>(width, height)
                .unwrap_or_else(|error| panic!("new image: {error}"));
            image
                .encoder()
                .write_tag(Tag::Unknown(34675), icc)
                .unwrap_or_else(|error| panic!("ICC tag: {error}"));
            image
                .write_data(data)
                .unwrap_or_else(|error| panic!("write data: {error}"));
            out.into_inner()
        }

        #[test]
        fn a_pro_photo_tiff_thumbnail_is_converted_to_srgb() {
            let icc = moxcms::ColorProfile::new_pro_photo_rgb()
                .encode()
                .unwrap_or_else(|error| panic!("encode ProPhoto: {error}"));
            let data = [150u8, 110, 90].repeat(400 * 200);
            let path = temp_file(
                "pro-photo-400x200.tif",
                &rgb_tiff_with_icc(400, 200, &data, &icc),
            );
            let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));
            let image = thumbnail(
                &path,
                100,
                &mut decoder,
                &Profile::srgb(),
                &TransformCache::default(),
            )
            .unwrap_or_else(|error| panic!("{error}"));
            assert_eq!((image.width, image.height), (100, 50));
            // T68's reference for ProPhoto (150,110,90) → sRGB, each channel within 1
            let reference = [200u8, 118, 105];
            for pixel in image.data.as_chunks::<3>().0 {
                assert!(
                    pixel
                        .iter()
                        .zip(reference)
                        .all(|(value, expected)| value.abs_diff(expected) <= 1),
                    "{pixel:?}, reference {reference:?}"
                );
            }
        }

        #[test]
        fn a_tiff_thumbnail_without_icc_in_srgb_equals_todays() {
            let (width, height) = (40u32, 20u32);
            let data: Vec<u8> = (0..height)
                .flat_map(|y| (0..width).flat_map(move |x| [(x * 6) as u8, (y * 12) as u8, 77]))
                .collect();
            let path = temp_file(
                "pattern-40x20.tif",
                &rgb_tiff(width, height, &data, None, None),
            );
            // today's path: decode_rgb8, fit, downscale, orient (1 here)
            let full = pw_tiff::decode_rgb8(&path, None).unwrap_or_else(|error| panic!("{error}"));
            let (w, h) = pw_tiff::fit(full.width, full.height, 10);
            let small = pw_tiff::downscale(&full, w, h);

            let image = thumb(&path, 10).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!((image.width, image.height), (small.width, small.height));
            assert_eq!(image.data, small.data);
        }

        #[test]
        fn a_tiff_is_reduced_to_the_cell_and_never_upscaled() {
            let path = temp_file(
                "rgb-400x200.tif",
                &rgb_tiff(400, 200, &uniform(400, 200), None, None),
            );

            let small = thumb(&path, 100).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!((small.width, small.height), (100, 50));
            assert_eq!(
                small.data,
                uniform(100, 50),
                "a uniform colour stays that colour"
            );

            let same = thumb(&path, 1000).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!((same.width, same.height), (400, 200));
            assert_eq!(same.data, uniform(400, 200));
        }

        #[test]
        fn a_tiff_with_orientation_6_is_turned() {
            let path = temp_file(
                "rgb-400x200-orientation-6.tif",
                &rgb_tiff(400, 200, &uniform(400, 200), Some(6), None),
            );
            let image = thumb(&path, 100).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!((image.width, image.height), (50, 100));
        }

        /// A 2×2 8-bit palette TIFF (PhotometricInterpretation 3 with a ColorMap), built by hand.
        /// Copied from `crates/pw-tiff/src/tests.rs`.
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
        fn a_palette_tiff_is_unreadable_with_the_pw_tiff_text() {
            let path = temp_file("palette.tif", &palette_tiff());
            let expected = pw_tiff::decode_rgb8(&path, None)
                .err()
                .unwrap_or_else(|| panic!("pw-tiff decoded a palette TIFF"));
            assert!(matches!(expected, pw_tiff::TiffError::Unsupported(_)));
            assert_eq!(
                thumb(&path, 100).err(),
                Some(ThumbError::Unreadable(expected.to_string()))
            );
        }

        #[test]
        fn a_tiff_with_an_embedded_jpeg_preview_is_decoded_from_its_image_data() {
            // Encode once to learn where the file ends (the tag values have a fixed size), then
            // again with the JPEG preview's offset there, and append the JPEG.
            let data = uniform(400, 200);
            let length = jpeg_fixture::SMALL.len() as u32;
            let offset = rgb_tiff(400, 200, &data, None, Some((0, length))).len() as u32;
            let mut bytes = rgb_tiff(400, 200, &data, None, Some((offset, length)));
            assert_eq!(bytes.len() as u32, offset);
            bytes.extend_from_slice(jpeg_fixture::SMALL);
            let path = temp_file("rgb-with-jpeg-preview.tif", &bytes);

            // pw-raw sees the 16×8 preview, so a thumbnail from it would be 16×8.
            let info = pw_raw::inspect(&path).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(
                info.previews
                    .iter()
                    .map(|p| (p.offset, p.width, p.height))
                    .collect::<Vec<_>>(),
                vec![(u64::from(offset), 16, 8)]
            );

            let image = thumb(&path, 100).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!((image.width, image.height), (100, 50));
            assert_eq!(image.data, uniform(100, 50));
        }

        /// Every TIFF (`.tif` or `.tiff`, any case) in `PW_TESTDATA`; at least one must exist.
        fn real_tiff_files() -> Vec<PathBuf> {
            let directory = std::env::var_os("PW_TESTDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| panic!("PW_TESTDATA is not set"));
            let mut files: Vec<PathBuf> = std::fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.is_file() && pw_raw::is_tiff_file(path))
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
        fn real_tiff_thumbnails_have_a_long_edge_of_256() {
            for path in real_tiff_files() {
                let image =
                    thumb(&path, 256).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                assert_eq!(
                    image.width.max(image.height),
                    256,
                    "{}: {}×{}",
                    path.display(),
                    image.width,
                    image.height
                );
            }
        }
    }
}
