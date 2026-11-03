//! Minimal JPEG marker walking: frame dimensions and the EXIF (APP1) segment.

use crate::source::{SliceSource, Source};
use crate::{Error, Format, ImageInfo, PreviewRef, Result, tiff};

/// Frame size of the JPEG at `offset`, if it is a baseline/extended/progressive Huffman JPEG
/// (the kinds browsers and ordinary decoders handle). Lossless and arithmetic JPEGs give `None`.
pub(crate) fn dimensions<S: Source>(src: &mut S, offset: u64, length: u64) -> Option<(u32, u32)> {
    let mut found = None;
    walk(src, offset, length, |src, marker, data_pos, _len| {
        if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            if matches!(marker, 0xC0..=0xC2) {
                let mut b = [0u8; 5];
                if src.read_at(data_pos, &mut b).is_ok() {
                    let h = u32::from(u16::from_be_bytes([b[1], b[2]]));
                    let w = u32::from(u16::from_be_bytes([b[3], b[4]]));
                    if w > 0 && h > 0 {
                        found = Some((w, h));
                    }
                }
            }
            return false; // first frame header decides
        }
        true
    });
    found
}

/// Absolute offset and length of the TIFF structure inside an APP1 "Exif" segment.
pub(crate) fn exif_tiff<S: Source>(src: &mut S, offset: u64, length: u64) -> Option<(u64, u64)> {
    let mut found = None;
    walk(src, offset, length, |src, marker, data_pos, len| {
        if marker == 0xE1 && len > 8 {
            let mut id = [0u8; 6];
            if src.read_at(data_pos, &mut id).is_ok() && &id == b"Exif\0\0" {
                found = Some((data_pos + 6, len - 6));
                return false;
            }
        }
        !matches!(marker, 0xC0..=0xCF) || matches!(marker, 0xC4 | 0xC8 | 0xCC)
    });
    found
}

/// Calls `f(src, marker, data_pos, data_len)` for each segment until `f` returns false,
/// start of scan, end of image, or a malformed marker.
fn walk<S: Source>(
    src: &mut S,
    offset: u64,
    length: u64,
    mut f: impl FnMut(&mut S, u8, u64, u64) -> bool,
) {
    let end = offset.saturating_add(length).min(src.len());
    let mut soi = [0u8; 2];
    if src.read_at(offset, &mut soi).is_err() || soi != [0xFF, 0xD8] {
        return;
    }
    let mut pos = offset + 2;
    for _ in 0..512 {
        if pos + 4 > end {
            return;
        }
        let mut m = [0u8; 2];
        if src.read_at(pos, &mut m).is_err() || m[0] != 0xFF {
            return;
        }
        let marker = m[1];
        if marker == 0xFF {
            pos += 1; // fill byte
            continue;
        }
        if matches!(marker, 0x01 | 0xD0..=0xD8) {
            pos += 2;
            continue;
        }
        if matches!(marker, 0xD9 | 0xDA) {
            return;
        }
        let mut l = [0u8; 2];
        if src.read_at(pos + 2, &mut l).is_err() {
            return;
        }
        let seg_len = u64::from(u16::from_be_bytes(l));
        if seg_len < 2 {
            return;
        }
        if !f(src, marker, pos + 4, seg_len - 2) {
            return;
        }
        pos += 2 + seg_len;
    }
}

/// A plain JPEG file: the file itself is the only "preview".
pub(crate) fn inspect_jpeg_file<S: Source>(src: &mut S) -> Result<ImageInfo> {
    let len = src.len();
    let (w, h) = dimensions(src, 0, len)
        .ok_or_else(|| Error::Format("unsupported JPEG (no baseline/progressive frame)".into()))?;
    let mut info = ImageInfo {
        format: Some(Format::Jpeg),
        orientation: 1,
        ..Default::default()
    };
    if let Some((base, tlen)) = exif_tiff(src, 0, len) {
        tiff::fill_from_exif_tiff(src, base, tlen, &mut info);
    }
    info.width = Some(w);
    info.height = Some(h);
    info.previews.push(PreviewRef {
        offset: 0,
        length: len,
        width: w,
        height: h,
        source: "file".into(),
    });
    Ok(info)
}

/// Sets any EXIF Orientation tag (IFD0, 0x0112) inside an in-memory JPEG to 1 (normal).
///
/// Embedded previews may carry their own orientation (the DNG previews in the test set have 6),
/// and WebView2 applies it automatically. The viewer rotates from the RAW file's orientation
/// instead, so the served JPEG must not be rotated a second time. Returns true if a tag was changed.
pub fn neutralize_orientation(jpeg: &mut [u8]) -> bool {
    let Some((base, tlen)) = exif_tiff(&mut SliceSource(jpeg), 0, jpeg.len() as u64) else {
        return false;
    };
    let (base, end) = (base as usize, (base + tlen) as usize);
    let Some(t) = jpeg.get_mut(base..end.min(jpeg.len())) else {
        return false;
    };
    if t.len() < 8 {
        return false;
    }
    let le = match &t[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return false,
    };
    let rd16 = |b: &[u8], i: usize| {
        let v = [b[i], b[i + 1]];
        if le {
            u16::from_le_bytes(v)
        } else {
            u16::from_be_bytes(v)
        }
    };
    let rd32 = |b: &[u8], i: usize| {
        let v = [b[i], b[i + 1], b[i + 2], b[i + 3]];
        if le {
            u32::from_le_bytes(v)
        } else {
            u32::from_be_bytes(v)
        }
    };
    let ifd0 = rd32(t, 4) as usize;
    if ifd0 + 2 > t.len() {
        return false;
    }
    let n = usize::from(rd16(t, ifd0));
    for k in 0..n {
        let e = ifd0 + 2 + 12 * k;
        if e + 12 > t.len() {
            return false;
        }
        if rd16(t, e) == 0x0112 && rd16(t, e + 2) == 3 {
            if rd16(t, e + 8) == 1 {
                return false;
            }
            let one = if le {
                1u16.to_le_bytes()
            } else {
                1u16.to_be_bytes()
            };
            t[e + 8..e + 10].copy_from_slice(&one);
            return true;
        }
    }
    false
}
