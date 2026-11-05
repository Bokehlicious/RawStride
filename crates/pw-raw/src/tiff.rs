//! TIFF/EXIF structure walking (ARW, RW2, DNG and other TIFF-based RAW files).

use std::collections::{HashSet, VecDeque};

use crate::source::Source;
use crate::{Error, ExposureTime, Format, ImageInfo, PreviewRef, Result, jpeg};

const MAX_IFDS: usize = 64;
const MAX_ENTRIES: u16 = 1000;

// Tags
const NEW_SUBFILE_TYPE: u16 = 0x00FE;
const IMAGE_WIDTH: u16 = 0x0100;
const IMAGE_LENGTH: u16 = 0x0101;
const COMPRESSION: u16 = 0x0103;
const PHOTOMETRIC: u16 = 0x0106;
const MAKE: u16 = 0x010F;
const MODEL: u16 = 0x0110;
const STRIP_OFFSETS: u16 = 0x0111;
const ORIENTATION: u16 = 0x0112;
const STRIP_BYTE_COUNTS: u16 = 0x0117;
const SUB_IFDS: u16 = 0x014A;
const JPEG_IF_OFFSET: u16 = 0x0201;
const JPEG_IF_LENGTH: u16 = 0x0202;
const EXIF_IFD: u16 = 0x8769;
const EXPOSURE_TIME: u16 = 0x829A;
const F_NUMBER: u16 = 0x829D;
const ISO: u16 = 0x8827;
const DATE_TIME_ORIGINAL: u16 = 0x9003;
const EXPOSURE_BIAS: u16 = 0x9204;
const FOCAL_LENGTH: u16 = 0x920A;
const LENS_MODEL: u16 = 0xA434;
// Panasonic RW2 (IFD0)
const PANA_SENSOR_WIDTH: u16 = 0x0002;
const PANA_SENSOR_HEIGHT: u16 = 0x0003;
const PANA_TOP: u16 = 0x0004;
const PANA_LEFT: u16 = 0x0005;
const PANA_BOTTOM: u16 = 0x0006;
const PANA_RIGHT: u16 = 0x0007;
const PANA_ISO: u16 = 0x0017;
const PANA_JPG_FROM_RAW: u16 = 0x002E;

const PHOTOMETRIC_YCBCR: u32 = 6;
const PHOTOMETRIC_CFA: u32 = 32803;
const PHOTOMETRIC_LINEAR_RAW: u32 = 34892;

#[derive(Debug, Clone, Copy)]
struct Entry {
    tag: u16,
    typ: u16,
    count: u32,
    /// Absolute position of the value data (inline or pointed to).
    pos: u64,
}

struct Parser<'a, S> {
    src: &'a mut S,
    /// Absolute offset of the TIFF header; IFD offsets are relative to it.
    base: u64,
    le: bool,
}

fn type_size(typ: u16) -> u64 {
    match typ {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => 0,
    }
}

impl<'a, S: Source> Parser<'a, S> {
    /// Reads the TIFF header at `base`; returns the parser, the magic number and IFD0 offset.
    fn new(src: &'a mut S, base: u64) -> Result<(Self, u16, u32)> {
        let mut h = [0u8; 8];
        src.read_at(base, &mut h)?;
        let le = match &h[0..2] {
            b"II" => true,
            b"MM" => false,
            _ => return Err(Error::Format("not a TIFF/RAW file".into())),
        };
        let p = Self { src, base, le };
        let magic = p.u16_of([h[2], h[3]]);
        let ifd0 = p.u32_of([h[4], h[5], h[6], h[7]]);
        Ok((p, magic, ifd0))
    }

    fn u16_of(&self, b: [u8; 2]) -> u16 {
        if self.le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        }
    }
    fn u32_of(&self, b: [u8; 4]) -> u32 {
        if self.le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        }
    }
    fn read_u16(&mut self, pos: u64) -> Option<u16> {
        let mut b = [0u8; 2];
        self.src.read_at(pos, &mut b).ok()?;
        Some(self.u16_of(b))
    }
    fn read_u32(&mut self, pos: u64) -> Option<u32> {
        let mut b = [0u8; 4];
        self.src.read_at(pos, &mut b).ok()?;
        Some(self.u32_of(b))
    }

    fn read_ifd(&mut self, off: u32) -> Result<(Vec<Entry>, u32)> {
        let start = self.base + u64::from(off);
        let n = self
            .read_u16(start)
            .ok_or_else(|| Error::Format("IFD outside file".into()))?;
        if n == 0 || n > MAX_ENTRIES {
            return Err(Error::Format(format!("implausible IFD entry count {n}")));
        }
        let mut raw = vec![0u8; usize::from(n) * 12 + 4];
        self.src.read_at(start + 2, &mut raw)?;
        let mut entries = Vec::with_capacity(usize::from(n));
        for e in raw[..usize::from(n) * 12].chunks_exact(12) {
            let tag = self.u16_of([e[0], e[1]]);
            let typ = self.u16_of([e[2], e[3]]);
            let count = self.u32_of([e[4], e[5], e[6], e[7]]);
            let size = type_size(typ).saturating_mul(u64::from(count));
            let entry_pos = start + 2 + (entries.len() as u64) * 12;
            let pos = if size <= 4 {
                entry_pos + 8
            } else {
                self.base + u64::from(self.u32_of([e[8], e[9], e[10], e[11]]))
            };
            entries.push(Entry {
                tag,
                typ,
                count,
                pos,
            });
        }
        let t = &raw[usize::from(n) * 12..];
        let next = self.u32_of([t[0], t[1], t[2], t[3]]);
        Ok((entries, next))
    }

    /// Integer value `i` of a BYTE/SHORT/LONG/IFD entry.
    fn uint(&mut self, e: &Entry, i: u32) -> Option<u32> {
        if i >= e.count {
            return None;
        }
        let i = u64::from(i);
        match e.typ {
            1 | 7 => {
                let mut b = [0u8; 1];
                self.src.read_at(e.pos + i, &mut b).ok()?;
                Some(u32::from(b[0]))
            }
            3 => self.read_u16(e.pos + 2 * i).map(u32::from),
            4 | 13 => self.read_u32(e.pos + 4 * i),
            _ => None,
        }
    }

    fn rational(&mut self, e: &Entry) -> Option<(u32, u32)> {
        if e.typ != 5 || e.count == 0 {
            return None;
        }
        Some((self.read_u32(e.pos)?, self.read_u32(e.pos + 4)?))
    }

    fn srational(&mut self, e: &Entry) -> Option<(i32, i32)> {
        if e.typ != 10 || e.count == 0 {
            return None;
        }
        Some((
            self.read_u32(e.pos)? as i32,
            self.read_u32(e.pos + 4)? as i32,
        ))
    }

    fn ascii(&mut self, e: &Entry) -> Option<String> {
        if e.typ != 2 || e.count == 0 {
            return None;
        }
        let mut b = vec![0u8; e.count.min(256) as usize];
        self.src.read_at(e.pos, &mut b).ok()?;
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        let s = String::from_utf8_lossy(&b[..end]).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

fn ratio(n: u32, d: u32) -> Option<f64> {
    (d != 0).then(|| f64::from(n) / f64::from(d))
}

/// Fills empty EXIF fields of `info` from an EXIF IFD.
fn read_exif_ifd<S: Source>(p: &mut Parser<'_, S>, off: u32, info: &mut ImageInfo) {
    let Ok((entries, _)) = p.read_ifd(off) else {
        return;
    };
    for e in &entries {
        match e.tag {
            EXPOSURE_TIME if info.exposure_time.is_none() => {
                info.exposure_time = p.rational(e).map(|(num, den)| ExposureTime { num, den });
            }
            F_NUMBER if info.f_number.is_none() => {
                info.f_number = p.rational(e).and_then(|(n, d)| ratio(n, d));
            }
            ISO if info.iso.is_none() => info.iso = p.uint(e, 0).filter(|&v| v > 0),
            DATE_TIME_ORIGINAL if info.date_time_original.is_none() => {
                info.date_time_original = p.ascii(e);
            }
            EXPOSURE_BIAS if info.exposure_bias.is_none() => {
                info.exposure_bias = p
                    .srational(e)
                    .and_then(|(n, d)| (d != 0).then(|| f64::from(n) / f64::from(d)));
            }
            FOCAL_LENGTH if info.focal_length.is_none() => {
                info.focal_length = p.rational(e).and_then(|(n, d)| ratio(n, d));
            }
            LENS_MODEL if info.lens.is_none() => info.lens = p.ascii(e),
            _ => {}
        }
    }
}

/// Fills empty fields of `info` from a TIFF structure embedded in a JPEG APP1 segment.
pub(crate) fn fill_from_exif_tiff<S: Source>(
    src: &mut S,
    base: u64,
    _len: u64,
    info: &mut ImageInfo,
) {
    let Ok((mut p, _magic, ifd0)) = Parser::new(src, base) else {
        return;
    };
    let Ok((entries, _)) = p.read_ifd(ifd0) else {
        return;
    };
    let had_orientation = info.orientation != 1;
    for e in &entries {
        match e.tag {
            MAKE if info.make.is_none() => info.make = p.ascii(e),
            MODEL if info.model.is_none() => info.model = p.ascii(e),
            ORIENTATION if !had_orientation => {
                if let Some(o) = p.uint(e, 0) {
                    info.orientation = o as u16;
                }
            }
            EXIF_IFD => {
                if let Some(off) = p.uint(e, 0) {
                    read_exif_ifd(&mut p, off, info);
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct IfdVals {
    subfile: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    compression: Option<u32>,
    photometric: Option<u32>,
    strip: Option<(u32, u32)>,
    jif: (Option<u32>, Option<u32>),
}

pub(crate) fn inspect_tiff<S: Source>(src: &mut S) -> Result<ImageInfo> {
    let file_len = src.len();
    let (mut p, magic, ifd0) = Parser::new(src, 0)?;
    let format = match magic {
        42 => Format::Tiff,
        0x55 => Format::Panasonic,
        m => return Err(Error::Format(format!("unsupported TIFF magic {m:#x}"))),
    };
    let mut info = ImageInfo {
        format: Some(format),
        orientation: 1,
        ..Default::default()
    };
    let mut orientation_found = false;
    let mut candidates: Vec<(u64, u64, String)> = Vec::new();
    let mut raw_dims: Option<(u32, u32)> = None;
    let mut exif_offsets = Vec::new();
    let mut pana = [None::<u32>; 8];
    // Size of IFD0's image; for plain TIFF files this is the image itself.
    let mut ifd0_dims: Option<(u32, u32)> = None;

    let mut queue = VecDeque::from([(ifd0, "IFD0".to_string(), Some(0usize))]);
    let mut visited = HashSet::new();
    while let Some((off, label, chain)) = queue.pop_front() {
        if off == 0 || visited.len() >= MAX_IFDS || !visited.insert(off) {
            continue;
        }
        let (entries, next) = match p.read_ifd(off) {
            Ok(r) => r,
            Err(e) if label == "IFD0" => return Err(e),
            Err(_) => continue, // tolerate broken secondary IFDs
        };
        if let Some(n) = chain
            && next != 0
        {
            queue.push_back((next, format!("IFD{}", n + 1), Some(n + 1)));
        }
        let is_ifd0 = label == "IFD0";
        let mut v = IfdVals::default();
        let mut strip_offsets = None;
        let mut strip_counts = None;
        for e in &entries {
            match e.tag {
                NEW_SUBFILE_TYPE => v.subfile = p.uint(e, 0),
                IMAGE_WIDTH => v.width = p.uint(e, 0),
                IMAGE_LENGTH => v.height = p.uint(e, 0),
                COMPRESSION => v.compression = p.uint(e, 0),
                PHOTOMETRIC => v.photometric = p.uint(e, 0),
                STRIP_OFFSETS if e.count == 1 => strip_offsets = p.uint(e, 0),
                STRIP_BYTE_COUNTS if e.count == 1 => strip_counts = p.uint(e, 0),
                JPEG_IF_OFFSET => v.jif.0 = p.uint(e, 0),
                JPEG_IF_LENGTH => v.jif.1 = p.uint(e, 0),
                SUB_IFDS => {
                    for i in 0..e.count.min(16) {
                        if let Some(o) = p.uint(e, i) {
                            queue.push_back((o, format!("{label}/SubIFD{i}"), None));
                        }
                    }
                }
                EXIF_IFD => {
                    if let Some(o) = p.uint(e, 0) {
                        exif_offsets.push(o);
                    }
                }
                MAKE if is_ifd0 => info.make = p.ascii(e),
                MODEL if is_ifd0 => info.model = p.ascii(e),
                ORIENTATION if is_ifd0 => {
                    if let Some(o) = p.uint(e, 0) {
                        info.orientation = o as u16;
                        orientation_found = true;
                    }
                }
                PANA_SENSOR_WIDTH..=PANA_RIGHT if is_ifd0 && format == Format::Panasonic => {
                    pana[usize::from(e.tag)] = p.uint(e, 0);
                }
                PANA_ISO if is_ifd0 && format == Format::Panasonic => {
                    info.iso = p.uint(e, 0).filter(|&v| v > 0);
                }
                PANA_JPG_FROM_RAW if is_ifd0 && format == Format::Panasonic => {
                    candidates.push((e.pos, u64::from(e.count), format!("{label}/JpgFromRaw")));
                }
                _ => {}
            }
        }
        if let (Some(o), Some(c)) = (strip_offsets, strip_counts) {
            v.strip = Some((o, c));
        }
        if is_ifd0
            && !matches!(
                v.photometric,
                Some(PHOTOMETRIC_CFA | PHOTOMETRIC_LINEAR_RAW)
            )
        {
            ifd0_dims = v.width.zip(v.height);
        }
        if let (Some(o), Some(l)) = v.jif
            && l > 0
        {
            candidates.push((
                u64::from(o),
                u64::from(l),
                format!("{label}/JPEGInterchangeFormat"),
            ));
        }
        if let Some((o, c)) = v.strip
            && matches!(v.compression, Some(6 | 7))
            && v.photometric == Some(PHOTOMETRIC_YCBCR)
        {
            candidates.push((u64::from(o), u64::from(c), format!("{label}/StripOffsets")));
        }
        if matches!(
            v.photometric,
            Some(PHOTOMETRIC_CFA | PHOTOMETRIC_LINEAR_RAW)
        ) && v.subfile.unwrap_or(0) & 1 == 0
            && let (Some(w), Some(h)) = (v.width, v.height)
        {
            let bigger = raw_dims
                .is_none_or(|(rw, rh)| u64::from(w) * u64::from(h) > u64::from(rw) * u64::from(rh));
            if bigger {
                raw_dims = Some((w, h));
            }
        }
    }

    for off in exif_offsets {
        read_exif_ifd(&mut p, off, &mut info);
    }

    if format == Format::Panasonic {
        let g = |t: u16| pana[usize::from(t)];
        raw_dims = match (g(PANA_TOP), g(PANA_LEFT), g(PANA_BOTTOM), g(PANA_RIGHT)) {
            (Some(t), Some(l), Some(b), Some(r)) if r > l && b > t => Some((r - l, b - t)),
            _ => g(PANA_SENSOR_WIDTH).zip(g(PANA_SENSOR_HEIGHT)),
        };
    }

    // Validate candidates: inside the file, a JPEG we can decode, not duplicated.
    let src = p.src;
    let mut seen = HashSet::new();
    for (offset, length, source) in candidates {
        if length < 4 || offset.saturating_add(length) > file_len || !seen.insert(offset) {
            continue;
        }
        if let Some((width, height)) = jpeg::dimensions(src, offset, length) {
            info.previews.push(PreviewRef {
                offset,
                length,
                width,
                height,
                source,
            });
        }
    }
    info.previews.sort_by_key(|p| (p.width, p.length));

    // Some formats (e.g. RW2) keep the EXIF only inside an embedded JPEG.
    if info.exposure_time.is_none() || info.make.is_none() {
        if !orientation_found {
            info.orientation = 1;
        }
        let previews: Vec<_> = info
            .previews
            .iter()
            .rev()
            .map(|p| (p.offset, p.length))
            .collect();
        for (offset, length) in previews {
            if let Some((base, tlen)) = jpeg::exif_tiff(src, offset, length) {
                fill_from_exif_tiff(src, base, tlen, &mut info);
                break;
            }
        }
    }

    // Plain TIFF (no RAW data): the image size is IFD0's size.
    let dims = raw_dims.or(ifd0_dims);
    let (w, h) = dims.unzip();
    info.width = w;
    info.height = h;
    if info.previews.is_empty() && dims.is_none() {
        return Err(Error::Format("no image data found".into()));
    }
    Ok(info)
}
