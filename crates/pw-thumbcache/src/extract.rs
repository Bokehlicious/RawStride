use std::fmt;
use std::fs;
use std::path::Path;

use pw_libjpeg::{Decoder, Rgb8, Scale};
use pw_raw::{Format, ImageInfo, PreviewRef};

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
            ThumbError::NoPreview => f.write_str("ingen förhandsvisning"),
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

/// Extracts and orients a thumbnail without decoding RAW image data.
pub fn thumbnail(path: &Path, cell: u32, decoder: &mut Decoder) -> Result<Rgb8, ThumbError> {
    let info = pw_raw::inspect(path).map_err(|error| ThumbError::Unreadable(error.to_string()))?;
    let bytes = if info.format == Some(Format::Jpeg) {
        fs::read(path).map_err(|error| ThumbError::Unreadable(error.to_string()))?
    } else {
        let preview = choose_preview(&info, cell).ok_or(ThumbError::NoPreview)?;
        pw_raw::read_preview(path, preview)
            .map_err(|error| ThumbError::Unreadable(error.to_string()))?
    };
    let (width, height) = decoder
        .header(&bytes)
        .map_err(|error| ThumbError::Unreadable(error.to_string()))?;
    let scale = choose_scale(width, height, cell);
    let image = decoder
        .decode(&bytes, scale)
        .map_err(|error| ThumbError::Unreadable(error.to_string()))?;
    Ok(orient(image, info.orientation))
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
            thumbnail(&missing, 200, &mut decoder),
            Err(ThumbError::Unreadable(_))
        ));

        let invalid = TempFile::new("img", b"not an image");
        assert!(matches!(
            thumbnail(invalid.path(), 200, &mut decoder),
            Err(ThumbError::Unreadable(_))
        ));
    }

    #[test]
    fn plain_jpeg_file_decodes_to_expected_size() {
        let file = TempFile::new("jpg", jpeg_fixture::SMALL);
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));

        let image = thumbnail(file.path(), 200, &mut decoder)
            .unwrap_or_else(|error| panic!("thumbnail failed: {error}"));
        assert_eq!((image.width, image.height), (16, 8));
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

        let image = thumbnail(&path, 200, &mut decoder)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(image.width > 0 && image.height > 0);
    }

    #[test]
    #[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
    fn real_panasonic_rw2_thumbnail() {
        let path = test_file("P1240231.RW2");
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));

        let image = thumbnail(&path, 200, &mut decoder)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(image.width > 0 && image.height > 0);
    }

    #[test]
    #[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
    fn real_dng_thumbnail() {
        let path = test_file("IMG_20261026_062851.dng");
        let mut decoder = Decoder::new().unwrap_or_else(|error| panic!("{error}"));

        let image = thumbnail(&path, 200, &mut decoder)
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

        let image = thumbnail(&path, 200, &mut decoder)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!((image.width, image.height), (height, width));
    }
}
