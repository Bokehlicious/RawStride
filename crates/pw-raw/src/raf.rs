//! Fujifilm RAF: an own big-endian header with an embedded JPEG (whose APP1 Exif is a normal
//! TIFF) and a metadata directory.
//!
//! Layout used here: bytes 0–15 are the magic `FUJIFILMCCD-RAW `, bytes 28–59 the camera model
//! (ASCII, padded with `\0`), byte 84 the `u32` offset and `u32` length of the embedded JPEG and
//! byte 92 the `u32` offset and `u32` length of the metadata directory. The directory is a `u32`
//! count followed by entries of `u16` tag, `u16` size and `size` bytes of data; tag `0x0111`
//! holds the cropped image size as height then width (`u16` each).

use crate::source::Source;
use crate::{Error, Format, ImageInfo, PreviewRef, Result, jpeg, tiff};

/// The first 16 bytes of every RAF file.
pub(crate) const MAGIC: &[u8; 16] = b"FUJIFILMCCD-RAW ";

/// Bytes of the header read at once (up to and including the directory's offset and length).
const HEADER_LEN: usize = 100;
const MODEL_AT: usize = 28;
const MODEL_LEN: usize = 32;
const JPEG_AT: usize = 84;
const DIRECTORY_AT: usize = 92;
/// Directory tag: cropped image size, height then width.
const TAG_CROPPED_SIZE: u16 = 0x0111;

fn format_error(message: &str) -> Error {
    Error::Format(format!("RAF: {message}"))
}

/// Big-endian `u32` at `at` in the header.
fn header_u32(header: &[u8; HEADER_LEN], at: usize) -> Option<u32> {
    let bytes = header.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

/// The camera model from the header, if it holds any non-blank ASCII.
fn header_model(header: &[u8; HEADER_LEN]) -> Option<String> {
    let bytes = header.get(MODEL_AT..MODEL_AT + MODEL_LEN)?;
    let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
    let text = bytes.get(..end)?;
    if !text.is_ascii() {
        return None;
    }
    let model = String::from_utf8_lossy(text).trim().to_string();
    (!model.is_empty()).then_some(model)
}

/// Width and height from the metadata directory's tag `0x0111`. `None` when the directory is
/// missing, lies outside the file, an entry runs past the directory's length, or the tag is
/// missing.
fn cropped_size<S: Source>(
    src: &mut S,
    header: &[u8; HEADER_LEN],
    file_len: u64,
) -> Option<(u32, u32)> {
    let offset = u64::from(header_u32(header, DIRECTORY_AT)?);
    let length = u64::from(header_u32(header, DIRECTORY_AT + 4)?);
    if offset == 0 {
        return None;
    }
    let end = offset.checked_add(length).filter(|&end| end <= file_len)?;
    let mut count = [0u8; 4];
    if offset.checked_add(4)? > end {
        return None;
    }
    src.read_at(offset, &mut count).ok()?;
    let count = u32::from_be_bytes(count);
    let mut position = offset.checked_add(4)?;
    let mut size = None;
    for _ in 0..count {
        let data_at = position.checked_add(4).filter(|&at| at <= end)?;
        let mut entry = [0u8; 4];
        src.read_at(position, &mut entry).ok()?;
        let tag = u16::from_be_bytes([entry[0], entry[1]]);
        let data_size = u64::from(u16::from_be_bytes([entry[2], entry[3]]));
        let data_end = data_at.checked_add(data_size).filter(|&e| e <= end)?;
        if tag == TAG_CROPPED_SIZE && data_size == 4 && size.is_none() {
            let mut data = [0u8; 4];
            src.read_at(data_at, &mut data).ok()?;
            let height = u32::from(u16::from_be_bytes([data[0], data[1]]));
            let width = u32::from(u16::from_be_bytes([data[2], data[3]]));
            if width > 0 && height > 0 {
                size = Some((width, height));
            }
        }
        position = data_end;
    }
    size
}

/// Inspects a RAF file (the caller has checked the magic).
pub(crate) fn inspect_raf<S: Source>(src: &mut S) -> Result<ImageInfo> {
    let file_len = src.len();
    if file_len < HEADER_LEN as u64 {
        return Err(format_error("file too short for the header"));
    }
    let mut header = [0u8; HEADER_LEN];
    src.read_at(0, &mut header)?;
    let jpeg_offset = header_u32(&header, JPEG_AT).map(u64::from);
    let jpeg_length = header_u32(&header, JPEG_AT + 4).map(u64::from);
    let (jpeg_offset, jpeg_length, jpeg_end) = jpeg_offset
        .zip(jpeg_length)
        .and_then(|(offset, length)| {
            offset
                .checked_add(length)
                .filter(|&end| end <= file_len)
                .map(|end| (offset, length, end))
        })
        .ok_or_else(|| format_error("the embedded JPEG lies past the end of the file"))?;
    let (width, height) = jpeg::dimensions(src, jpeg_offset, jpeg_length)
        .ok_or_else(|| format_error("the embedded JPEG cannot be read"))?;

    let mut info = ImageInfo {
        format: Some(Format::Raf),
        orientation: 1,
        ..Default::default()
    };
    let exif = jpeg::exif_tiff(src, jpeg_offset, jpeg_length);
    if let Some((base, exif_length)) = exif {
        tiff::fill_from_exif_tiff(src, base, exif_length, &mut info);
    }
    if info.model.is_none() {
        info.model = header_model(&header);
    }
    let (raw_width, raw_height) = cropped_size(src, &header, file_len).unzip();
    info.width = raw_width;
    info.height = raw_height;

    info.previews.push(PreviewRef {
        offset: jpeg_offset,
        length: jpeg_length,
        width,
        height,
        source: "RAF/JpegImage".into(),
    });
    let thumbnail = exif
        .and_then(|(base, _)| tiff::exif_thumbnail(src, base))
        .filter(|&(offset, length)| {
            offset >= jpeg_offset
                && offset
                    .checked_add(length)
                    .is_some_and(|end| end <= jpeg_end)
        });
    if let Some((offset, length)) = thumbnail
        && let Some((width, height)) = jpeg::dimensions(src, offset, length)
    {
        info.previews.push(PreviewRef {
            offset,
            length,
            width,
            height,
            source: "RAF/IFD1/JPEGInterchangeFormat".into(),
        });
    }
    info.previews.sort_by_key(|p| (p.width, p.length));
    Ok(info)
}

#[cfg(test)]
mod tests {
    use crate::tests::tiny_jpeg;
    use crate::{Error, Format, ImageInfo, Result, SliceSource, inspect_source};

    /// Where the test RAF's metadata directory starts.
    const DIRECTORY_AT: usize = 100;

    /// A RAF: the magic, model `X-T30`, a metadata directory with `entries` (its count is
    /// `count`, or the number of entries), then a 4416 × 2944 JPEG with Exif Orientation 6.
    fn raf(entries: &[(u16, Vec<u8>)], count: Option<u32>) -> Vec<u8> {
        let mut directory = count.unwrap_or(entries.len() as u32).to_be_bytes().to_vec();
        for (tag, data) in entries {
            directory.extend_from_slice(&tag.to_be_bytes());
            directory.extend_from_slice(&(data.len() as u16).to_be_bytes());
            directory.extend_from_slice(data);
        }
        let jpeg = tiny_jpeg(4416, 2944, Some(6));
        let jpeg_at = DIRECTORY_AT + directory.len();
        let mut f = vec![0u8; DIRECTORY_AT];
        f[0..16].copy_from_slice(super::MAGIC);
        f[28..33].copy_from_slice(b"X-T30");
        f[84..88].copy_from_slice(&(jpeg_at as u32).to_be_bytes());
        f[88..92].copy_from_slice(&(jpeg.len() as u32).to_be_bytes());
        f[92..96].copy_from_slice(&(DIRECTORY_AT as u32).to_be_bytes());
        f[96..100].copy_from_slice(&(directory.len() as u32).to_be_bytes());
        f.extend_from_slice(&directory);
        f.extend_from_slice(&jpeg);
        f
    }

    /// Tag 0x0111: height 4160, width 6240.
    fn cropped_size() -> (u16, Vec<u8>) {
        (0x0111, vec![0x10, 0x40, 0x18, 0x60])
    }

    fn inspect(data: &[u8]) -> Result<ImageInfo> {
        inspect_source(&mut SliceSource(data))
    }

    fn format_message(result: Result<ImageInfo>) -> String {
        match result {
            Err(Error::Format(message)) => message,
            other => panic!("expected a format error, got {other:?}"),
        }
    }

    #[test]
    fn raf_format_model_size_orientation_and_preview() {
        let data = raf(&[(0x0100, vec![1, 2, 3]), cropped_size()], None);
        let info = inspect(&data).unwrap();
        assert_eq!(info.format, Some(Format::Raf));
        assert_eq!(info.model.as_deref(), Some("X-T30"));
        assert_eq!((info.width, info.height), (Some(6240), Some(4160)));
        assert_eq!(info.orientation, 6);
        assert_eq!(info.previews.len(), 1);
        let preview = &info.previews[0];
        let jpeg_at = u64::from(u32::from_be_bytes(data[84..88].try_into().unwrap()));
        assert_eq!(preview.offset, jpeg_at);
        assert_eq!(preview.offset + preview.length, data.len() as u64);
        assert_eq!((preview.width, preview.height), (4416, 2944));
        assert_eq!(preview.source, "RAF/JpegImage");
        assert_eq!(&data[preview.offset as usize..][..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn raf_without_the_size_tag_has_no_size() {
        let info = inspect(&raf(&[(0x0100, vec![1, 2, 3])], None)).unwrap();
        assert_eq!(info.format, Some(Format::Raf));
        assert_eq!((info.width, info.height), (None, None));
        assert_eq!(info.previews.len(), 1);
    }

    #[test]
    fn raf_directory_count_past_its_length_has_no_size() {
        let info = inspect(&raf(&[cropped_size()], Some(5))).unwrap();
        assert_eq!(info.format, Some(Format::Raf));
        assert_eq!((info.width, info.height), (None, None));
        assert_eq!(info.orientation, 6);
        assert_eq!(info.previews.len(), 1);
    }

    #[test]
    fn raf_jpeg_past_the_end_is_an_error() {
        let mut data = raf(&[cropped_size()], None);
        let length = u32::from_be_bytes(data[88..92].try_into().unwrap());
        data[88..92].copy_from_slice(&(length + 1).to_be_bytes());
        assert!(format_message(inspect(&data)).starts_with("RAF: "));
        // an offset near u32::MAX does not overflow either
        data[84..88].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(format_message(inspect(&data)).starts_with("RAF: "));
    }

    #[test]
    fn raf_of_50_bytes_is_an_error() {
        let mut data = vec![0u8; 50];
        data[0..16].copy_from_slice(super::MAGIC);
        assert!(format_message(inspect(&data)).starts_with("RAF: "));
    }

    #[test]
    fn partial_raf_magic_goes_to_the_tiff_check() {
        let mut data = b"FUJIFILMCCD-RAWX".to_vec();
        data.resize(200, 0);
        let message = format_message(inspect(&data));
        assert!(!message.starts_with("RAF: "), "{message}");
    }

    #[test]
    fn a_file_of_10_bytes_gives_the_old_error() {
        assert_eq!(
            format_message(inspect(b"FUJIFILMCC")),
            "not a TIFF/RAW file"
        );
    }
}
