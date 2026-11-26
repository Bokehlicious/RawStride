//! The real decoders (feature `native`): LibRaw with PPG (ADR 0008 §11) through `pw-libraw` for
//! RAW, libjpeg-turbo through `pw-libjpeg` for the embedded JPEG located by `pw-raw` (G4,
//! ADR 0009 §5.2), and `pw-tiff` for TIFF files, decoded once under the core budget (ADR 0015
//! §4.1–§4.4). Without the feature the core's tests need no native libraries.
//!
//! Every view is converted from its source profile to the backend's destination (the monitor
//! profile) with `pw-colour` before the core gives it out, so the core's caches hold
//! display-ready pixels (ADR 0016 §4.1, §4.2): JPEG from its APP2 profile or Exif marker (ADR 0016
//! §3.1) and RAW from sRGB (LibRaw renders sRGB, ADR 0006). The renders of RAW and JPEG are
//! unconverted; the core reduces a view first and then converts it with
//! [`Backend::to_display`] (ADR 0027 §3.2, W1-Q4). A TIFF is converted from its embedded profile
//! once, when it is opened (W1-Q5), and `to_display` leaves it unchanged. With the default
//! destination sRGB, sRGB sources are identity transforms and the pixels are unchanged (F3). A
//! session remembers whether the file's embedded profile could not be used (ADR 0016 §5), for the
//! loupe's status line.
//!
//! The histogram and the exposure statistics (ADR 0017 §2.6, U7) come from the data **before**
//! that conversion: RAW from the unpacked RAW data (`pw-libraw`), a JPEG from its full decode and
//! a TIFF from its own decoded image. The clipping map (U7b) comes from the same data.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pw_colour::{ColourError, Profile, Transform, TransformCache, jpeg_source_profile_checked};
use pw_libjpeg::{Decoder, Scale};
use pw_libraw::{DAMAGED, Demosaic, Exposure, Session};
use pw_raw::ExifColourSpace;

use crate::backend::{
    Backend, ClippingMap, DecodeError, Rgb, Source, Statistics, StatisticsOrigin, Stop,
};
use crate::exposure::Ev;
use crate::geometry::{ImageSize, Rect};
use crate::scale;

/// Context rendered around a RAW region and cut off again, so a region equals the same area of a
/// whole render (ADR 0006; checked for PPG in `pw-libraw/tests/regions.rs`).
const MARGIN: u32 = 16;

/// The TIFF memory limit of [`NativeBackend::default`]: the default core budget (4 GB).
const DEFAULT_TIFF_LIMIT: usize = 4 << 30;

const MIB: usize = 1024 * 1024;

/// LibRaw with the fixed settings of ADR 0006 and PPG, libjpeg-turbo for embedded JPEGs, and
/// `pw-tiff` for TIFF files; every image converted to the destination profile (ADR 0016).
#[derive(Clone, Debug)]
pub struct NativeBackend {
    /// The most memory in bytes one TIFF decode may use (the core budget, ADR 0015 Q4).
    tiff_limit: usize,
    /// The profile the pixels are converted to: the monitor profile (ADR 0016 §4.1).
    destination: Profile,
    /// The transforms, built once per pair of profiles and shared by the core's workers.
    cache: Arc<TransformCache>,
}

impl Default for NativeBackend {
    /// The default TIFF limit and destination sRGB.
    fn default() -> NativeBackend {
        NativeBackend::with_tiff_limit(DEFAULT_TIFF_LIMIT)
    }
}

impl NativeBackend {
    /// A backend whose TIFF decodes may use at most `bytes` (the core budget, ADR 0015 Q4), with
    /// destination sRGB.
    pub fn with_tiff_limit(bytes: usize) -> NativeBackend {
        NativeBackend {
            tiff_limit: bytes,
            destination: Profile::srgb(),
            cache: Arc::new(TransformCache::default()),
        }
    }

    /// The same backend converting to `profile` (the monitor profile, ADR 0016 §4.1).
    pub fn with_monitor_profile(self, profile: Profile) -> NativeBackend {
        NativeBackend {
            destination: profile,
            ..self
        }
    }

    /// The transform from `source` to the destination, built once per pair. A source profile no
    /// transform can be built from is treated as sRGB, as `pw-tiff` does.
    fn transform(&self, source: &Profile) -> Result<Arc<Transform>, DecodeError> {
        self.transform_checked(source).map(|(t, _)| t)
    }

    /// [`NativeBackend::transform`], plus `true` when it had to fall back to sRGB because no
    /// transform could be built from `source` (ADR 0016 §5).
    fn transform_checked(&self, source: &Profile) -> Result<(Arc<Transform>, bool), DecodeError> {
        match self.cache.get(source, &self.destination) {
            Ok(t) => Ok((t, false)),
            Err(_) => self
                .cache
                .get(&Profile::srgb(), &self.destination)
                .map(|t| (t, true))
                .map_err(colour_error),
        }
    }
}

/// An opened file.
#[derive(Debug)]
pub enum NativeSession {
    /// An unpacked RAW file and the sRGB → destination transform of its renders.
    Raw(Session, Arc<Transform>),
    /// A JPEG: its bytes, a decoder, its transform, and the full decode once a 100 % view needed
    /// it.
    Jpeg(JpegSession),
    /// A TIFF file: the whole image, decoded and converted once when it was opened (ADR 0015
    /// §4.2, ADR 0016 §4.1).
    Tiff(TiffSession),
}

#[derive(Debug)]
pub struct TiffSession {
    /// The whole image, already converted.
    image: Rgb,
    /// The file's embedded ICC profile could not be used (`SourceColour::Unusable`, ADR 0016 §5).
    profile_unusable: bool,
    /// The file, for the statistics of an image that was converted when it was opened.
    path: PathBuf,
    /// `image` holds the file's own values: it was treated as sRGB and the destination is sRGB,
    /// so the conversion changed nothing.
    unconverted: bool,
}

#[derive(Debug)]
pub struct JpegSession {
    data: Vec<u8>,
    decoder: Decoder,
    /// From the JPEG's source profile (ADR 0016 §3.1) to the destination.
    transform: Arc<Transform>,
    /// The full decode, unconverted (a crop of it is converted by `to_display` like any other
    /// view, ADR 0027 W1-Q4).
    full: Option<Rgb>,
    /// The statistics of the full decode before its conversion, once they were computed.
    statistics: Option<Box<Statistics>>,
    /// The JPEG's embedded ICC profile could not be used, or no transform could be built from its
    /// source profile (ADR 0016 §5).
    profile_unusable: bool,
}

/// LibRaw's flip code for an EXIF/TIFF orientation; moved to `geometry` (ADR 0024 §2.3), kept
/// here for its callers.
pub use crate::geometry::flip_of_orientation;

fn raw_error(e: pw_libraw::Error) -> DecodeError {
    if e.is_cancelled() {
        return DecodeError::cancelled();
    }
    let message = e.0;
    if message.contains(DAMAGED) {
        DecodeError::damaged(message)
    } else {
        DecodeError::new(message)
    }
}

fn jpeg_error(e: pw_libjpeg::Error) -> DecodeError {
    DecodeError::new(e.to_string())
}

/// The loupe's text for a TIFF that cannot be shown (ADR 0015 §5); the loupe puts it after
/// "The file cannot be shown: ", so it is lower case without a full stop (T118). Never
/// `damaged`: that is for RAW data (G3).
fn tiff_error(e: pw_tiff::TiffError) -> DecodeError {
    match e {
        pw_tiff::TiffError::Unsupported(m) => {
            DecodeError::new(format!("the TIFF variant is not supported yet ({m})"))
        }
        pw_tiff::TiffError::TooLarge { limit } => DecodeError::new(format!(
            "the TIFF file is too large for the memory budget ({} MB)",
            limit / MIB
        )),
        e @ (pw_tiff::TiffError::Io(_) | pw_tiff::TiffError::Decode(_)) => {
            DecodeError::new(e.to_string())
        }
    }
}

/// A conversion error (cannot happen for whole RGB pixels) or a transform that could not be
/// built, with the `ColourError` text.
fn colour_error(e: ColourError) -> DecodeError {
    DecodeError::new(e.to_string())
}

/// Converts `img` in place with `transform` (nothing happens for an identity).
fn convert(transform: &Transform, mut img: Rgb) -> Result<Rgb, DecodeError> {
    transform
        .convert_rgb8(&mut img.data)
        .map_err(colour_error)?;
    Ok(img)
}

fn rgb_raw(img: pw_libraw::Rgb8) -> Rgb {
    Rgb {
        width: img.width,
        height: img.height,
        data: img.data,
    }
}

fn rgb_jpeg(img: pw_libjpeg::Rgb8) -> Rgb {
    Rgb {
        width: img.width,
        height: img.height,
        data: img.data,
    }
}

/// Opens the largest embedded JPEG of `path` (or a plain JPEG file itself), with the transform
/// from its source profile (APP2 ICC, else the Exif Adobe RGB marker, else sRGB; ADR 0016 §3.1)
/// to the backend's destination.
fn open_jpeg(
    backend: &NativeBackend,
    path: &Path,
) -> Result<(JpegSession, ImageSize), DecodeError> {
    let info =
        pw_raw::inspect(path).map_err(|e| DecodeError::new(format!("{}: {e}", path.display())))?;
    let Some(preview) = info.pick(pw_raw::PreviewSize::Large) else {
        return Err(DecodeError::no_embedded_jpeg(format!(
            "{}: the file has no embedded JPEG",
            path.display()
        )));
    };
    let data = pw_raw::read_preview(path, preview)
        .map_err(|e| DecodeError::new(format!("{}: {e}", path.display())))?;
    let mut decoder = Decoder::new().map_err(jpeg_error)?;
    let icc = decoder.icc_profile(&data).map_err(jpeg_error)?;
    let (w, h) = decoder.header(&data).map_err(jpeg_error)?;
    let (source, icc_unusable) = jpeg_source_profile_checked(
        icc.as_deref(),
        info.colour_space == ExifColourSpace::AdobeRgb,
    );
    let (transform, fell_back) = backend.transform_checked(&source)?;
    // the orientation is the file's tag (ADR 0003 point 4); the JPEG's own tag is not applied
    let flip = flip_of_orientation(info.orientation);
    let session = JpegSession {
        data,
        decoder,
        transform,
        full: None,
        statistics: None,
        profile_unusable: icc_unusable || fell_back,
    };
    Ok((session, ImageSize { w, h, flip }))
}

/// Decodes a TIFF file whole with `pw-tiff` under the backend's limit and converts it from its
/// source profile to the destination (ADR 0016 §4.1); `SourceColour::Unusable` sets the session's
/// `profile_unusable` (ADR 0016 §5). The orientation is the file's tag as `pw-raw` reads it, or 1
/// when `pw-raw` cannot inspect the file (BigTIFF, ADR 0015 §4.3).
fn open_tiff(
    backend: &NativeBackend,
    path: &Path,
) -> Result<(TiffSession, ImageSize), DecodeError> {
    let (img, source) = pw_tiff::decode_rgb8_for_display(
        path,
        Some(backend.tiff_limit),
        &backend.destination,
        &backend.cache,
    )
    .map_err(tiff_error)?;
    let img = Rgb {
        width: img.width,
        height: img.height,
        data: img.data,
    };
    let orientation = pw_raw::inspect(path).map_or(1, |info| info.orientation);
    let size = ImageSize {
        w: img.width,
        h: img.height,
        flip: flip_of_orientation(orientation),
    };
    // Pixels treated as sRGB are unchanged when the destination is sRGB (F3); an embedded
    // profile may have changed them.
    let unconverted = !matches!(source, pw_tiff::SourceColour::Embedded)
        && backend
            .transform(&Profile::srgb())
            .is_ok_and(|transform| transform.is_identity());
    let session = TiffSession {
        image: img,
        profile_unusable: matches!(source, pw_tiff::SourceColour::Unusable(_)),
        path: path.to_path_buf(),
        unconverted,
    };
    Ok((session, size))
}

/// `pw-libraw`'s exposure for `exposure`. An [`Ev`] always lies in LibRaw's range, so the error
/// is never given in practice.
fn raw_exposure(exposure: Ev) -> Result<Exposure, DecodeError> {
    let ev = exposure.milli() as f32 / 1000.0;
    Exposure::from_ev(ev).ok_or_else(|| {
        DecodeError::new(format!(
            "the exposure {} EV is outside the range",
            exposure.ev()
        ))
    })
}

/// The statistics of a RAW file's unpacked data in the core's form.
fn raw_statistics(raw: &pw_libraw::RawStatistics) -> Statistics {
    Statistics {
        origin: StatisticsOrigin::RawData,
        histogram: raw.histogram,
        clipped: [
            raw.clipped_share(0),
            raw.clipped_share(1),
            raw.clipped_share(2),
        ],
        deep_shadows: raw.deep_shadow_share(),
    }
}

/// The statistics of a JPEG from its full decode **before** the conversion to the destination
/// (ADR 0017 U7-Q5), computed once per session. The session's full decode is unconverted, so it
/// is used as it is; a session without one decodes it here and keeps it for the 100 % views, as
/// `render_region` would.
fn jpeg_statistics(j: &mut JpegSession) -> Option<Statistics> {
    if let Some(statistics) = &j.statistics {
        return Some(statistics.as_ref().clone());
    }
    let origin = StatisticsOrigin::EmbeddedJpeg;
    let statistics = match &j.full {
        Some(full) => Statistics::of_rgb8(origin, &full.data),
        None => {
            let full = j.decoder.decode(&j.data, Scale::Full).map(rgb_jpeg).ok()?;
            let statistics = Statistics::of_rgb8(origin, &full.data);
            j.full = Some(full);
            statistics
        }
    };
    j.statistics = Some(Box::new(statistics.clone()));
    Some(statistics)
}

/// The statistics of a TIFF from its own decoded image **before** the conversion to the
/// destination (ADR 0017 U7-Q5). The session holds the converted image (ADR 0015 §4.2), so a
/// file whose pixels the conversion may have changed is decoded once more, without it.
fn tiff_statistics(backend: &NativeBackend, tiff: &TiffSession) -> Option<Statistics> {
    let origin = StatisticsOrigin::Tiff;
    if tiff.unconverted {
        return Some(Statistics::of_rgb8(origin, &tiff.image.data));
    }
    let image = pw_tiff::decode_rgb8(&tiff.path, Some(backend.tiff_limit)).ok()?;
    Some(Statistics::of_rgb8(origin, &image.data))
}

/// The clipping map of a RAW file's unpacked data in the core's form (the same layout).
fn raw_clipping_map(raw: pw_libraw::ClippingMap) -> ClippingMap {
    ClippingMap {
        cell: raw.cell,
        width: raw.width,
        height: raw.height,
        data: raw.data,
    }
}

/// The clipping map of a JPEG from its full decode **before** the conversion to the destination
/// (ADR 0017 U7b, U7-Q5): the session's full decode (unconverted) when it has one, otherwise a
/// new decode of the JPEG, which is not kept. The session's own pixels are not changed.
fn jpeg_clipping_map(j: &mut JpegSession) -> Option<ClippingMap> {
    match &j.full {
        Some(full) => ClippingMap::of_rgb8(full.width, full.height, &full.data),
        None => {
            let full = j.decoder.decode(&j.data, Scale::Full).ok()?;
            ClippingMap::of_rgb8(full.width, full.height, &full.data)
        }
    }
}

/// The clipping map of a TIFF from its own decoded image **before** the conversion to the
/// destination (ADR 0017 U7b, U7-Q5): the session's image when the conversion changed nothing,
/// otherwise a new decode of the file, as [`tiff_statistics`].
fn tiff_clipping_map(backend: &NativeBackend, tiff: &TiffSession) -> Option<ClippingMap> {
    if tiff.unconverted {
        let image = &tiff.image;
        return ClippingMap::of_rgb8(image.width, image.height, &image.data);
    }
    let image = pw_tiff::decode_rgb8(&tiff.path, Some(backend.tiff_limit)).ok()?;
    ClippingMap::of_rgb8(image.width, image.height, &image.data)
}

impl Backend for NativeBackend {
    type Session = NativeSession;

    fn open(&self, path: &Path, source: Source) -> Result<(NativeSession, ImageSize), DecodeError> {
        // the file type wins over the display mode: a TIFF is shown as itself (ADR 0015 Q3)
        if pw_raw::is_tiff_file(path) {
            let (tiff, size) = open_tiff(self, path)?;
            return Ok((NativeSession::Tiff(tiff), size));
        }
        match source {
            Source::Raw => {
                let session = Session::open(path).map_err(raw_error)?;
                // LibRaw renders sRGB (ADR 0006)
                let transform = self.transform(&Profile::srgb())?;
                let size = ImageSize {
                    w: session.width(),
                    h: session.height(),
                    flip: session.flip(),
                };
                Ok((NativeSession::Raw(session, transform), size))
            }
            Source::Jpeg => {
                let (s, size) = open_jpeg(self, path)?;
                Ok((NativeSession::Jpeg(s), size))
            }
        }
    }

    fn render_half(&self, session: &mut NativeSession) -> Result<Rgb, DecodeError> {
        match session {
            NativeSession::Raw(s, _) => s.render_half().map(rgb_raw).map_err(raw_error),
            NativeSession::Jpeg(j) => j
                .decoder
                .decode(&j.data, Scale::Half)
                .map(rgb_jpeg)
                .map_err(jpeg_error),
            NativeSession::Tiff(TiffSession { image: img, .. }) => {
                // With the Lanczos filter (ADR 0027 W1-Q5). A TIFF's view below 50 % is thus
                // reduced twice (whole → half → the view's size); accepted, a TIFF is rare in
                // the owner's work.
                let (w, h) = scale::scaled_size(img.width, img.height, 0.5);
                scale::reduce(img, w, h).ok_or_else(|| {
                    DecodeError::new(format!(
                        "the TIFF could not be reduced to {w}x{h} (it is {}x{})",
                        img.width, img.height
                    ))
                })
            }
        }
    }

    fn render_region(&self, session: &mut NativeSession, r: Rect) -> Result<Rgb, DecodeError> {
        match session {
            NativeSession::Raw(s, _) => {
                let r = pw_libraw::Rect::new(r.x, r.y, r.w, r.h);
                s.render_region_with(r, MARGIN, Demosaic::Ppg)
                    .map(rgb_raw)
                    .map_err(raw_error)
            }
            NativeSession::Jpeg(j) => {
                if j.full.is_none() {
                    let full = j
                        .decoder
                        .decode(&j.data, Scale::Full)
                        .map(rgb_jpeg)
                        .map_err(jpeg_error)?;
                    // decoded once here and kept unconverted, so crops need no decode
                    j.full = Some(full);
                }
                let full = j
                    .full
                    .as_ref()
                    .ok_or_else(|| DecodeError::new("the JPEG was not decoded"))?;
                scale::crop(full, r.x, r.y, r.w, r.h)
                    .ok_or_else(|| DecodeError::new(format!("region {r:?} is outside the JPEG")))
            }
            NativeSession::Tiff(TiffSession { image: img, .. }) => {
                scale::crop(img, r.x, r.y, r.w, r.h)
                    .ok_or_else(|| DecodeError::new(format!("region {r:?} is outside the TIFF")))
            }
        }
    }

    /// A RAW render is made with the exposure applied by LibRaw (ADR 0027 §3.4, W5) and is
    /// stopped between LibRaw's stages; a stop while LibRaw works spends the session (T327). JPEG
    /// and TIFF renders ignore the exposure, are short and are not stopped.
    fn render_half_stoppable(
        &self,
        session: &mut NativeSession,
        exposure: Ev,
        stop: &Stop,
    ) -> Result<Rgb, DecodeError> {
        match session {
            NativeSession::Raw(s, _) => s
                .render_half_exposed(raw_exposure(exposure)?, &|| stop.is_stopped())
                .map(rgb_raw)
                .map_err(raw_error),
            _ => self.render_half(session),
        }
    }

    /// As [`render_half_stoppable`](Self::render_half_stoppable), for a region.
    fn render_region_stoppable(
        &self,
        session: &mut NativeSession,
        r: Rect,
        exposure: Ev,
        stop: &Stop,
    ) -> Result<Rgb, DecodeError> {
        match session {
            NativeSession::Raw(s, _) => {
                let r = pw_libraw::Rect::new(r.x, r.y, r.w, r.h);
                let exposure = raw_exposure(exposure)?;
                s.render_region_exposed(r, MARGIN, Demosaic::Ppg, exposure, &|| stop.is_stopped())
                    .map(rgb_raw)
                    .map_err(raw_error)
            }
            _ => self.render_region(session, r),
        }
    }

    /// The brightness the view adds is the RAW file's value with its sign turned (ADR 0027 §4.4).
    fn camera_exposure(&self, session: &NativeSession) -> Option<f64> {
        match session {
            NativeSession::Raw(s, _) => s.camera_exposure().map(|c| camera_brightness(c.ev)),
            _ => None,
        }
    }

    /// A RAW session is spent after a stopped or failed render (T327).
    fn session_usable(&self, session: &NativeSession) -> bool {
        match session {
            NativeSession::Raw(s, _) => !s.is_spent(),
            _ => true,
        }
    }

    fn to_display(&self, session: &NativeSession, img: Rgb) -> Result<Rgb, DecodeError> {
        match session {
            NativeSession::Raw(_, t) => convert(t, img),
            NativeSession::Jpeg(j) => convert(&j.transform, img),
            // converted once when it was opened (ADR 0027 W1-Q5)
            NativeSession::Tiff(_) => Ok(img),
        }
    }

    fn statistics(&self, session: &mut NativeSession) -> Option<Statistics> {
        match session {
            // an error and a layout without statistics both leave the panel without a histogram
            NativeSession::Raw(s, _) => s.statistics().ok().flatten().as_ref().map(raw_statistics),
            NativeSession::Jpeg(j) => jpeg_statistics(j),
            NativeSession::Tiff(t) => tiff_statistics(self, t),
        }
    }

    fn clipping_map(&self, session: &mut NativeSession) -> Option<ClippingMap> {
        match session {
            // an error and a layout without a map both leave the overlays without one
            NativeSession::Raw(s, _) => s.clipping_map().ok().flatten().map(raw_clipping_map),
            NativeSession::Jpeg(j) => jpeg_clipping_map(j),
            NativeSession::Tiff(t) => tiff_clipping_map(self, t),
        }
    }

    fn profile_unusable(&self, session: &NativeSession) -> bool {
        match session {
            // LibRaw renders sRGB (ADR 0006): there is no embedded profile to use
            NativeSession::Raw(..) => false,
            NativeSession::Jpeg(j) => j.profile_unusable,
            NativeSession::Tiff(t) => t.profile_unusable,
        }
    }
}

/// The view adds the file's value with its sign turned because the value says how far the camera
/// kept the RAW data below its own rendering (ADR 0027 §4.4, W5's hand check, owner 2026-11-26).
fn camera_brightness(file_ev: f32) -> f64 {
    -f64::from(file_ev)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_brightness_turns_the_sign_of_the_file_value() {
        assert!((camera_brightness(-0.72) - 0.72).abs() < 1e-6);
        assert!((camera_brightness(-0.5062) - 0.5062).abs() < 1e-6);
        assert_eq!(camera_brightness(0.0), 0.0);
        assert!((camera_brightness(1.2) + 1.2).abs() < 1e-6);
    }

    #[test]
    fn orientation_tags_map_to_libraw_flips() {
        assert_eq!(flip_of_orientation(1), 0);
        assert_eq!(flip_of_orientation(3), 3);
        assert_eq!(flip_of_orientation(6), 6);
        assert_eq!(flip_of_orientation(8), 5);
        assert_eq!(flip_of_orientation(2), 0);
        assert_eq!(flip_of_orientation(0), 0);
    }

    // ---------- TIFF (ADR 0015 D2) ----------

    use std::io::Cursor;
    use std::path::PathBuf;

    use tiff::encoder::{TiffEncoder, colortype};
    use tiff::tags::Tag;

    /// Writes `bytes` to `name` in a test folder of its own.
    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pw-viewcore-native-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// An 8-bit RGB TIFF, with an Orientation tag when `orientation` is given.
    fn rgb_tiff(w: u32, h: u32, data: &[u8], orientation: Option<u16>) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let mut encoder = TiffEncoder::new(&mut out).unwrap();
        let mut image = encoder.new_image::<colortype::RGB8>(w, h).unwrap();
        if let Some(o) = orientation {
            image.encoder().write_tag(Tag::Orientation, o).unwrap();
        }
        image.write_data(data).unwrap();
        out.into_inner()
    }

    /// Pixel (x, y) of the test image.
    fn pixel(x: u32, y: u32) -> [u8; 3] {
        [(x * 10) as u8, (y * 10) as u8, (x + y * 6) as u8]
    }

    fn pixels(w: u32, h: u32) -> Vec<u8> {
        (0..h)
            .flat_map(|y| (0..w).flat_map(move |x| pixel(x, y)))
            .collect()
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
    fn a_tiff_opens_whole_and_renders_half_and_regions() {
        let path = temp_file("rgb-6x4.tif", &rgb_tiff(6, 4, &pixels(6, 4), None));
        let backend = NativeBackend::default();
        let (mut session, size) = backend.open(&path, Source::Jpeg).unwrap();
        assert!(matches!(session, NativeSession::Tiff(_)), "{session:?}");
        assert_eq!(
            size,
            ImageSize {
                w: 6,
                h: 4,
                flip: 0
            }
        );

        let half = backend.render_half(&mut session).unwrap();
        assert_eq!((half.width, half.height), (3, 2));
        assert!(half.is_consistent());

        let region = backend
            .render_region(&mut session, Rect::new(1, 2, 3, 2))
            .unwrap();
        assert_eq!((region.width, region.height), (3, 2));
        let expected: Vec<u8> = (2..4)
            .flat_map(|y| (1..4).flat_map(move |x| pixel(x, y)))
            .collect();
        assert_eq!(region.data, expected);

        assert!(
            backend
                .render_region(&mut session, Rect::new(4, 0, 3, 1))
                .is_err()
        );
    }

    #[test]
    fn a_tiff_opened_as_raw_is_the_same_tiff_session() {
        let path = temp_file("as-raw.tif", &rgb_tiff(6, 4, &pixels(6, 4), None));
        let backend = NativeBackend::default();
        let (mut session, size) = backend.open(&path, Source::Raw).unwrap();
        assert!(matches!(session, NativeSession::Tiff(_)), "{session:?}");
        assert_eq!(
            size,
            ImageSize {
                w: 6,
                h: 4,
                flip: 0
            }
        );
        let all = backend
            .render_region(&mut session, Rect::new(0, 0, 6, 4))
            .unwrap();
        assert_eq!(all.data, pixels(6, 4));
    }

    #[test]
    fn a_tiff_with_orientation_6_turns() {
        let path = temp_file("orientation-6.tif", &rgb_tiff(6, 4, &pixels(6, 4), Some(6)));
        let (_, size) = NativeBackend::default().open(&path, Source::Jpeg).unwrap();
        assert_eq!(
            size,
            ImageSize {
                w: 6,
                h: 4,
                flip: 6
            }
        );
    }

    #[test]
    fn a_palette_tiff_is_not_supported_yet_and_not_damaged() {
        let path = temp_file("palette.tif", &palette_tiff());
        let err = NativeBackend::default()
            .open(&path, Source::Jpeg)
            .unwrap_err();
        assert!(
            err.message
                .starts_with("the TIFF variant is not supported yet ("),
            "{err:?}"
        );
        assert!(!err.damaged);
    }

    #[test]
    fn a_tiff_larger_than_the_limit_is_too_large_for_the_budget() {
        let (w, h) = (64, 64);
        let path = temp_file("limit.tif", &rgb_tiff(w, h, &pixels(w, h), None));
        let err = NativeBackend::with_tiff_limit(1024)
            .open(&path, Source::Jpeg)
            .unwrap_err();
        assert!(
            err.message.contains("too large for the memory budget"),
            "{err:?}"
        );
        assert!(!err.damaged);
        // the default limit decodes it
        assert!(NativeBackend::default().open(&path, Source::Jpeg).is_ok());
    }

    /// A RAW-like file (TIFF structure, `.dng` name) without an embedded JPEG, opened in JPEG
    /// mode, is marked as such (T118); other errors are not.
    #[test]
    fn a_file_without_an_embedded_jpeg_gets_the_typed_kind() {
        let (w, h) = (8, 8);
        let path = temp_file("no-preview.dng", &rgb_tiff(w, h, &pixels(w, h), None));
        let err = NativeBackend::default()
            .open(&path, Source::Jpeg)
            .unwrap_err();
        assert!(err.no_embedded_jpeg, "{err:?}");
        assert!(!err.damaged);
        assert!(
            err.message.ends_with(": the file has no embedded JPEG"),
            "{err:?}"
        );

        let palette = temp_file("palette-kind.tif", &palette_tiff());
        let err = NativeBackend::default()
            .open(&palette, Source::Jpeg)
            .unwrap_err();
        assert!(!err.no_embedded_jpeg, "{err:?}");
        assert!(!DecodeError::new("x").no_embedded_jpeg);
        assert!(!DecodeError::damaged("x").no_embedded_jpeg);
    }

    #[test]
    fn tiff_errors_get_the_loupe_texts() {
        use pw_tiff::TiffError;
        let e = tiff_error(TiffError::Unsupported("TIFF: palette".into()));
        assert_eq!(
            e,
            DecodeError::new("the TIFF variant is not supported yet (TIFF: palette)")
        );
        let e = tiff_error(TiffError::TooLarge { limit: 512 << 20 });
        assert_eq!(
            e,
            DecodeError::new("the TIFF file is too large for the memory budget (512 MB)")
        );
        let e = tiff_error(TiffError::Io("x.tif: not found".into()));
        assert_eq!(e, DecodeError::new("x.tif: not found"));
        let e = tiff_error(TiffError::Decode("TIFF: bad strip".into()));
        assert_eq!(e, DecodeError::new("TIFF: bad strip"));
    }

    // ---------- colour conversion to the monitor profile (ADR 0016 C4) ----------

    use moxcms::ColorProfile;

    // Inside the inline `tests` module of `native.rs` the path is relative to
    // `src/native/tests/`, hence four `..`.
    #[path = "../../../../pw-thumbcache/src/jpeg_fixture.rs"]
    mod jpeg_fixture;

    fn pro_photo_icc() -> Vec<u8> {
        ColorProfile::new_pro_photo_rgb().encode().unwrap()
    }

    fn adobe_rgb_icc() -> Vec<u8> {
        ColorProfile::new_adobe_rgb().encode().unwrap()
    }

    /// `data` converted from `source` to `destination`, through a transform cache as the backend
    /// gets it.
    fn converted(mut data: Vec<u8>, source: &Profile, destination: &Profile) -> Vec<u8> {
        let cache = TransformCache::default();
        let transform = cache.get(source, destination).unwrap();
        transform.convert_rgb8(&mut data).unwrap();
        data
    }

    /// An 8-bit RGB TIFF with the ICC profile `icc` in tag 34675.
    fn rgb_tiff_with_icc(w: u32, h: u32, data: &[u8], icc: &[u8]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let mut encoder = TiffEncoder::new(&mut out).unwrap();
        let mut image = encoder.new_image::<colortype::RGB8>(w, h).unwrap();
        image.encoder().write_tag(Tag::Unknown(34675), icc).unwrap();
        image.write_data(data).unwrap();
        out.into_inner()
    }

    /// The whole image of a session, through `render_region`.
    fn whole(backend: &NativeBackend, session: &mut NativeSession, size: ImageSize) -> Rgb {
        backend
            .render_region(session, Rect::new(0, 0, size.w, size.h))
            .unwrap()
    }

    #[test]
    fn a_tiff_with_an_icc_profile_is_opened_converted_to_the_destination() {
        let (w, h) = (6, 4);
        let path = temp_file(
            "pro-photo-icc.tif",
            &rgb_tiff_with_icc(w, h, &pixels(w, h), &pro_photo_icc()),
        );
        let srgb = Profile::srgb();
        let expected =
            pw_tiff::decode_rgb8_for_display(&path, None, &srgb, &TransformCache::default())
                .unwrap()
                .0;
        // the conversion changed the pixels, so the comparison below means something
        assert_ne!(
            expected.data,
            pw_tiff::decode_rgb8(&path, None).unwrap().data
        );

        for backend in [
            NativeBackend::default().with_monitor_profile(srgb.clone()),
            NativeBackend::default(),
        ] {
            let (mut session, size) = backend.open(&path, Source::Jpeg).unwrap();
            assert_eq!(whole(&backend, &mut session, size).data, expected.data);
        }
    }

    #[test]
    fn a_tiff_without_icc_is_unchanged_with_the_default_backend() {
        let (w, h) = (6, 4);
        let path = temp_file("no-icc.tif", &rgb_tiff(w, h, &pixels(w, h), None));
        let backend = NativeBackend::default();
        let (mut session, size) = backend.open(&path, Source::Jpeg).unwrap();
        assert_eq!(
            whole(&backend, &mut session, size).data,
            pw_tiff::decode_rgb8(&path, None).unwrap().data
        );
    }

    /// The fixture JPEG's plain decodes: half size, and the region (4, 2, 8×4) of the full decode.
    fn plain_jpeg_decodes() -> (Rgb, Rgb) {
        let mut decoder = Decoder::new().unwrap();
        let half = rgb_jpeg(decoder.decode(jpeg_fixture::SMALL, Scale::Half).unwrap());
        let full = rgb_jpeg(decoder.decode(jpeg_fixture::SMALL, Scale::Full).unwrap());
        let region = scale::crop(&full, 4, 2, 8, 4).unwrap();
        (half, region)
    }

    /// `render_half` and the region (4, 2, 8×4) of `path` opened with `backend`, as shown: each
    /// through `to_display` (ADR 0027 W1-Q4).
    fn jpeg_renders(backend: &NativeBackend, path: &Path) -> (Rgb, Rgb) {
        let (mut session, size) = backend.open(path, Source::Jpeg).unwrap();
        assert!(matches!(session, NativeSession::Jpeg(_)), "{session:?}");
        assert_eq!((size.w, size.h), (16, 8));
        let half = backend.render_half(&mut session).unwrap();
        let half = backend.to_display(&session, half).unwrap();
        let region = shown_region(backend, &mut session, Rect::new(4, 2, 8, 4));
        // a second region comes from the stored full decode and is converted the same way
        let again = shown_region(backend, &mut session, Rect::new(4, 2, 8, 4));
        assert_eq!(again, region);
        (half, region)
    }

    /// The region `r` of a session as shown: `render_region`, then `to_display`.
    fn shown_region(backend: &NativeBackend, session: &mut NativeSession, r: Rect) -> Rgb {
        let region = backend.render_region(session, r).unwrap();
        backend.to_display(session, region).unwrap()
    }

    #[test]
    fn a_plain_jpeg_is_converted_from_srgb_to_the_destination() {
        let path = temp_file("plain.jpg", jpeg_fixture::SMALL);
        let (plain_half, plain_region) = plain_jpeg_decodes();
        let (srgb, adobe) = (Profile::srgb(), Profile::adobe_rgb());

        let backend = NativeBackend::default().with_monitor_profile(adobe.clone());
        let (half, region) = jpeg_renders(&backend, &path);
        let expected_half = converted(plain_half.data.clone(), &srgb, &adobe);
        assert_ne!(
            expected_half, plain_half.data,
            "the conversion changes the fixture"
        );
        assert_eq!(half.data, expected_half);
        assert_eq!(
            region.data,
            converted(plain_region.data.clone(), &srgb, &adobe)
        );

        // destination sRGB: the plain decodes byte for byte (F3)
        let backend = NativeBackend::default().with_monitor_profile(srgb);
        let (half, region) = jpeg_renders(&backend, &path);
        assert_eq!(half, plain_half);
        assert_eq!(region, plain_region);
    }

    /// The fixture JPEG with `segment` (a marker segment, marker included) inserted after SOI.
    fn fixture_with_segment(segment: &[u8]) -> Vec<u8> {
        let jpeg = jpeg_fixture::SMALL;
        let mut out = jpeg[..2].to_vec();
        out.extend_from_slice(segment);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    /// One APP2 `ICC_PROFILE` segment (sequence 1 of 1) holding `icc`.
    fn app2_icc(icc: &[u8]) -> Vec<u8> {
        let len = u16::try_from(2 + 12 + 2 + icc.len()).unwrap();
        let mut s = vec![0xFF, 0xE2];
        s.extend_from_slice(&len.to_be_bytes());
        s.extend_from_slice(b"ICC_PROFILE\0");
        s.extend_from_slice(&[1, 1]);
        s.extend_from_slice(icc);
        s
    }

    /// Writes a little-endian IFD with `entries` (tag, type, count, value) at `at` of `f`.
    fn put_ifd(f: &mut [u8], at: usize, entries: &[(u16, u16, u32, u32)]) {
        f[at..at + 2].copy_from_slice(&(entries.len() as u16).to_le_bytes());
        for (i, &(tag, kind, count, val)) in entries.iter().enumerate() {
            let e = at + 2 + 12 * i;
            f[e..e + 2].copy_from_slice(&tag.to_le_bytes());
            f[e + 2..e + 4].copy_from_slice(&kind.to_le_bytes());
            f[e + 4..e + 8].copy_from_slice(&count.to_le_bytes());
            f[e + 8..e + 12].copy_from_slice(&val.to_le_bytes());
        }
        let n = at + 2 + 12 * entries.len();
        f[n..n + 4].copy_from_slice(&0u32.to_le_bytes());
    }

    /// An APP1 Exif segment: IFD0 → Exif IFD (ColorSpace 0xFFFF, Interop IFD) → Interop IFD
    /// (InteroperabilityIndex "R03"), the DCF marker of Adobe RGB. Built as the `pw-raw` tests
    /// of T71 build it.
    fn app1_exif_adobe_rgb() -> Vec<u8> {
        let (exif_at, interop_at) = (100u32, 200u32);
        let mut tiff = vec![0u8; 256];
        tiff[0..4].copy_from_slice(b"II*\0");
        tiff[4..8].copy_from_slice(&8u32.to_le_bytes());
        put_ifd(&mut tiff, 8, &[(0x8769, 4, 1, exif_at)]);
        put_ifd(
            &mut tiff,
            exif_at as usize,
            &[(0xA001, 3, 1, 0xFFFF), (0xA005, 4, 1, interop_at)],
        );
        let r03 = u32::from_le_bytes([b'R', b'0', b'3', 0]);
        put_ifd(&mut tiff, interop_at as usize, &[(0x0001, 2, 4, r03)]);
        let mut s = vec![0xFF, 0xE1];
        s.extend_from_slice(&((2 + 6 + tiff.len()) as u16).to_be_bytes());
        s.extend_from_slice(b"Exif\0\0");
        s.extend_from_slice(&tiff);
        s
    }

    #[test]
    fn a_jpeg_with_the_destinations_own_icc_profile_is_unchanged() {
        let icc = adobe_rgb_icc();
        let path = temp_file("app2-adobe.jpg", &fixture_with_segment(&app2_icc(&icc)));
        let destination = Profile::from_icc(&icc).unwrap();
        let backend = NativeBackend::default().with_monitor_profile(destination);
        let (plain_half, plain_region) = plain_jpeg_decodes();
        let (half, region) = jpeg_renders(&backend, &path);
        assert_eq!(half, plain_half);
        assert_eq!(region, plain_region);
    }

    #[test]
    fn a_jpeg_with_the_exif_adobe_rgb_marker_is_converted_from_adobe_rgb() {
        let path = temp_file(
            "exif-adobe.jpg",
            &fixture_with_segment(&app1_exif_adobe_rgb()),
        );
        assert_eq!(
            pw_raw::inspect(&path).unwrap().colour_space,
            ExifColourSpace::AdobeRgb
        );
        let backend = NativeBackend::default();
        let (plain_half, plain_region) = plain_jpeg_decodes();
        let (half, region) = jpeg_renders(&backend, &path);
        let (adobe, srgb) = (Profile::adobe_rgb(), Profile::srgb());
        let expected_half = converted(plain_half.data.clone(), &adobe, &srgb);
        assert_ne!(
            expected_half, plain_half.data,
            "the conversion changes the fixture"
        );
        assert_eq!(half.data, expected_half);
        assert_eq!(region.data, converted(plain_region.data, &adobe, &srgb));
    }

    // ---------- the conversion after the reduction (ADR 0027 §3.2, W1-Q4, W1-Q5, T308) ----------

    /// The fixture JPEG's plain full decode.
    fn plain_jpeg_full() -> Rgb {
        let mut decoder = Decoder::new().unwrap();
        rgb_jpeg(decoder.decode(jpeg_fixture::SMALL, Scale::Full).unwrap())
    }

    /// The session's full decode, when it is a JPEG session that has one.
    fn jpeg_full(session: &NativeSession) -> Option<&Rgb> {
        match session {
            NativeSession::Jpeg(j) => j.full.as_ref(),
            _ => None,
        }
    }

    #[test]
    fn the_renders_are_unconverted_and_to_display_converts_them() {
        let (srgb, adobe) = (Profile::srgb(), Profile::adobe_rgb());
        let backend = NativeBackend::default().with_monitor_profile(adobe.clone());

        // a JPEG: the plain decodes, and to_display gives the converted ones
        let path = temp_file("unconverted.jpg", jpeg_fixture::SMALL);
        let (plain_half, plain_region) = plain_jpeg_decodes();
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        let half = backend.render_half(&mut session).unwrap();
        assert_eq!(half, plain_half);
        let region = backend
            .render_region(&mut session, Rect::new(4, 2, 8, 4))
            .unwrap();
        assert_eq!(region, plain_region);
        let expected_half = converted(plain_half.data.clone(), &srgb, &adobe);
        assert_ne!(
            expected_half, plain_half.data,
            "the conversion changes the fixture"
        );
        assert_eq!(
            backend.to_display(&session, half).unwrap().data,
            expected_half
        );
        assert_eq!(
            backend.to_display(&session, region).unwrap().data,
            converted(plain_region.data.clone(), &srgb, &adobe)
        );

        // a TIFF stays as it was converted when it was opened: to_display changes nothing
        let (w, h) = (6, 4);
        let tiff = temp_file("unconverted.tif", &rgb_tiff(w, h, &pixels(w, h), None));
        let (mut session, size) = backend.open(&tiff, Source::Jpeg).unwrap();
        assert!(matches!(&session, NativeSession::Tiff(t) if !t.unconverted));
        let half = backend.render_half(&mut session).unwrap();
        assert_eq!(backend.to_display(&session, half.clone()).unwrap(), half);
        let all = whole(&backend, &mut session, size);
        assert_ne!(all.data, pixels(w, h), "the TIFF was converted when opened");
        assert_eq!(backend.to_display(&session, all.clone()).unwrap(), all);
    }

    #[test]
    fn a_jpeg_session_keeps_its_full_decode_unconverted_for_every_crop() {
        let (srgb, adobe) = (Profile::srgb(), Profile::adobe_rgb());
        let backend = NativeBackend::default().with_monitor_profile(adobe.clone());
        let path = temp_file("full-unconverted.jpg", jpeg_fixture::SMALL);
        let plain = plain_jpeg_full();
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        assert_eq!(jpeg_full(&session), None);

        let first = backend
            .render_region(&mut session, Rect::new(4, 2, 8, 4))
            .unwrap();
        assert_eq!(jpeg_full(&session), Some(&plain));
        let second = backend
            .render_region(&mut session, Rect::new(0, 3, 6, 5))
            .unwrap();
        assert_eq!(jpeg_full(&session), Some(&plain));

        for (crop, r) in [(first, (4, 2, 8, 4)), (second, (0, 3, 6, 5))] {
            let plain_crop = scale::crop(&plain, r.0, r.1, r.2, r.3).unwrap();
            assert_eq!(crop, plain_crop, "{r:?} is unconverted");
            let shown = backend.to_display(&session, crop).unwrap();
            let expected = converted(plain_crop.data.clone(), &srgb, &adobe);
            assert_ne!(expected, plain_crop.data, "the conversion changes {r:?}");
            assert_eq!(shown.data, expected, "{r:?} as shown");
        }
    }

    #[test]
    fn jpeg_statistics_keep_the_full_decode_unconverted() {
        let backend = NativeBackend::default().with_monitor_profile(Profile::adobe_rgb());
        let path = temp_file("statistics-full.jpg", jpeg_fixture::SMALL);
        let plain = plain_jpeg_full();

        // before any render: the statistics leave the plain full decode in the session
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        let before = backend.statistics(&mut session).unwrap();
        assert_eq!(before, plain_jpeg_statistics());
        assert_eq!(jpeg_full(&session), Some(&plain));

        // after a 100 % view: the same statistics, and the full decode is still there
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        backend
            .render_region(&mut session, Rect::new(4, 2, 8, 4))
            .unwrap();
        assert_eq!(backend.statistics(&mut session), Some(before));
        assert_eq!(jpeg_full(&session), Some(&plain));
    }

    // ---------- statistics before the conversion (ADR 0017 §2.6, U7-Q5, T142) ----------

    /// The statistics of the fixture JPEG's plain full decode.
    fn plain_jpeg_statistics() -> Statistics {
        let mut decoder = Decoder::new().unwrap();
        let full = decoder.decode(jpeg_fixture::SMALL, Scale::Full).unwrap();
        Statistics::of_rgb8(StatisticsOrigin::EmbeddedJpeg, &full.data)
    }

    #[test]
    fn jpeg_statistics_are_taken_before_the_conversion_whatever_was_rendered_first() {
        let path = temp_file("statistics.jpg", jpeg_fixture::SMALL);
        let (srgb, adobe) = (Profile::srgb(), Profile::adobe_rgb());
        let expected = plain_jpeg_statistics();
        assert_eq!(expected.origin, StatisticsOrigin::EmbeddedJpeg);
        let mut decoder = Decoder::new().unwrap();
        let plain = rgb_jpeg(decoder.decode(jpeg_fixture::SMALL, Scale::Full).unwrap());
        let converted_data = converted(plain.data.clone(), &srgb, &adobe);
        // the conversion changes the statistics, so the comparisons below mean something
        assert_ne!(
            Statistics::of_rgb8(StatisticsOrigin::EmbeddedJpeg, &converted_data),
            expected
        );
        let converted_region = scale::crop(
            &Rgb {
                data: converted_data,
                ..plain.clone()
            },
            4,
            2,
            8,
            4,
        )
        .unwrap();
        let backend = NativeBackend::default().with_monitor_profile(adobe);

        // 1. before any render: the statistics decode the JPEG and leave the unconverted full
        //    decode for the 100 % views
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        assert_eq!(backend.statistics(&mut session), Some(expected.clone()));
        let region = shown_region(&backend, &mut session, Rect::new(4, 2, 8, 4));
        assert_eq!(region, converted_region);

        // 2. after a half-size render
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        backend.render_half(&mut session).unwrap();
        assert_eq!(backend.statistics(&mut session), Some(expected.clone()));

        // 3. after a 100 % view, when the session already holds the full decode
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        let region = shown_region(&backend, &mut session, Rect::new(4, 2, 8, 4));
        assert_eq!(region, converted_region);
        assert_eq!(backend.statistics(&mut session), Some(expected.clone()));
        // asked again, the same (kept in the session)
        assert_eq!(backend.statistics(&mut session), Some(expected.clone()));
        let again = shown_region(&backend, &mut session, Rect::new(4, 2, 8, 4));
        assert_eq!(again, converted_region);

        // destination sRGB (no conversion): the same statistics
        let backend = NativeBackend::default();
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        backend
            .render_region(&mut session, Rect::new(4, 2, 8, 4))
            .unwrap();
        assert_eq!(backend.statistics(&mut session), Some(expected));
    }

    #[test]
    fn tiff_statistics_are_of_the_files_own_values() {
        let (w, h) = (6, 4);
        // a white, a black and a red-clipped pixel among the test pixels
        let mut data = pixels(w, h);
        data[0..9].copy_from_slice(&[255, 255, 255, 0, 0, 0, 255, 3, 0]);
        let expected = Statistics::of_rgb8(StatisticsOrigin::Tiff, &data);
        assert_eq!(expected.clipped[0], 2.0 / 24.0);
        assert_eq!(expected.deep_shadows, 1.0 / 24.0);

        // without a profile and with destination sRGB: from the session's image
        let plain = temp_file("statistics-plain.tif", &rgb_tiff(w, h, &data, None));
        let backend = NativeBackend::default();
        let (mut session, _) = backend.open(&plain, Source::Jpeg).unwrap();
        assert!(matches!(&session, NativeSession::Tiff(t) if t.unconverted));
        assert_eq!(backend.statistics(&mut session), Some(expected.clone()));

        // with an embedded profile the opened image is converted; the statistics are not
        let profiled = temp_file(
            "statistics-pro-photo.tif",
            &rgb_tiff_with_icc(w, h, &data, &pro_photo_icc()),
        );
        let (mut session, size) = backend.open(&profiled, Source::Jpeg).unwrap();
        assert!(matches!(&session, NativeSession::Tiff(t) if !t.unconverted));
        let shown = whole(&backend, &mut session, size);
        assert_ne!(shown.data, data, "the conversion changes the pixels");
        assert_ne!(
            Statistics::of_rgb8(StatisticsOrigin::Tiff, &shown.data),
            expected
        );
        assert_eq!(backend.statistics(&mut session), Some(expected.clone()));

        // without a profile but with another destination: also the file's own values
        let backend = NativeBackend::default().with_monitor_profile(Profile::adobe_rgb());
        let (mut session, _) = backend.open(&plain, Source::Jpeg).unwrap();
        assert!(matches!(&session, NativeSession::Tiff(t) if !t.unconverted));
        assert_eq!(backend.statistics(&mut session), Some(expected));
    }

    #[test]
    fn raw_statistics_keep_the_histogram_and_become_shares() {
        let mut raw = pw_libraw::RawStatistics {
            histogram: [[0; 256]; 3],
            pixels: [100, 200, 100],
            clipped: [1, 50, 0],
            shadow_pixels: 400,
            deep_shadows: 40,
        };
        raw.histogram[1][255] = 50;
        raw.histogram[2][7] = 100;
        let s = raw_statistics(&raw);
        assert_eq!(s.origin, StatisticsOrigin::RawData);
        assert_eq!(s.histogram, raw.histogram);
        assert_eq!(s.clipped, [0.01, 0.25, 0.0]);
        assert_eq!(s.deep_shadows, 0.1);
    }

    // ---------- the clipping map before the conversion (ADR 0017 §2.6, U7b, T147) ----------

    #[test]
    fn a_jpegs_clipping_map_is_made_before_the_conversion_whatever_was_rendered_first() {
        // the fixture marked as Adobe RGB (Exif): shown on an sRGB screen its red half clips
        let path = temp_file(
            "clipping-exif-adobe.jpg",
            &fixture_with_segment(&app1_exif_adobe_rgb()),
        );
        let (srgb, adobe) = (Profile::srgb(), Profile::adobe_rgb());
        let mut decoder = Decoder::new().unwrap();
        let plain = rgb_jpeg(decoder.decode(jpeg_fixture::SMALL, Scale::Full).unwrap());
        let expected = ClippingMap::of_rgb8(plain.width, plain.height, &plain.data).unwrap();
        assert_eq!(
            (expected.cell, expected.width, expected.height),
            (2, 8, 4),
            "cells of 2 × 2 pixels of the 16 × 8 fixture"
        );
        // the JPEG's own values clip nowhere
        assert!(expected.data.iter().all(|cell| *cell == 0));
        let converted_data = converted(plain.data.clone(), &adobe, &srgb);
        // the conversion changes the map (red clips in the left half), so the comparisons below
        // mean something
        let of_converted =
            ClippingMap::of_rgb8(plain.width, plain.height, &converted_data).unwrap();
        assert_eq!(of_converted.at(0, 0), Some(ClippingMap::CLIPPED_R));
        assert_ne!(of_converted, expected);
        let converted_region = scale::crop(
            &Rgb {
                data: converted_data,
                ..plain.clone()
            },
            4,
            2,
            8,
            4,
        )
        .unwrap();
        let backend = NativeBackend::default();

        // 1. before any render: the map decodes the JPEG; the session's own pixels are not
        //    changed, and a 100 % view after it is the converted one
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        assert_eq!(backend.clipping_map(&mut session), Some(expected.clone()));
        assert!(matches!(&session, NativeSession::Jpeg(j) if j.full.is_none()));
        let region = shown_region(&backend, &mut session, Rect::new(4, 2, 8, 4));
        assert_eq!(region, converted_region);

        // 2. after a half-size render
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        backend.render_half(&mut session).unwrap();
        assert_eq!(backend.clipping_map(&mut session), Some(expected.clone()));

        // 3. after a 100 % view and the statistics, when the session holds the full decode: the
        //    map is still of the JPEG's own values, and the session's pixels stay
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        let region = shown_region(&backend, &mut session, Rect::new(4, 2, 8, 4));
        assert_eq!(region, converted_region);
        assert!(backend.statistics(&mut session).is_some());
        assert_eq!(backend.clipping_map(&mut session), Some(expected.clone()));
        assert_eq!(backend.clipping_map(&mut session), Some(expected.clone()));
        let again = shown_region(&backend, &mut session, Rect::new(4, 2, 8, 4));
        assert_eq!(again, converted_region);

        // a destination that is the JPEG's own profile (no conversion): the same map, from a
        // new decode and from the session's own full decode
        let backend = NativeBackend::default().with_monitor_profile(adobe);
        let (mut session, _) = backend.open(&path, Source::Jpeg).unwrap();
        assert!(matches!(&session, NativeSession::Jpeg(j) if j.transform.is_identity()));
        assert_eq!(backend.clipping_map(&mut session), Some(expected.clone()));
        let region = backend
            .render_region(&mut session, Rect::new(4, 2, 8, 4))
            .unwrap();
        assert_eq!(region, scale::crop(&plain, 4, 2, 8, 4).unwrap());
        assert!(matches!(&session, NativeSession::Jpeg(j) if j.full.is_some()));
        assert_eq!(backend.clipping_map(&mut session), Some(expected));
    }

    #[test]
    fn a_tiffs_clipping_map_is_of_the_files_own_values() {
        let (w, h) = (6, 4);
        // a white, a black and a red-clipped pixel among the test pixels, a black cell, and a
        // strong red below 255 (pixel (0, 2)) that clips only when ProPhoto is converted to sRGB
        let mut data = pixels(w, h);
        data[0..9].copy_from_slice(&[255, 255, 255, 0, 0, 0, 255, 3, 0]);
        data[36..39].copy_from_slice(&[240, 20, 20]);
        for (x, y) in [(4, 2), (5, 2), (4, 3), (5, 3)] {
            let at = ((y * w + x) * 3) as usize;
            data[at..at + 3].copy_from_slice(&[0, 0, 0]);
        }
        let expected = ClippingMap::of_rgb8(w, h, &data).unwrap();
        assert_eq!((expected.cell, expected.width, expected.height), (2, 3, 2));
        assert_eq!(expected.at(0, 0), Some(7));
        assert_eq!(expected.at(1, 0), Some(ClippingMap::CLIPPED_R));
        assert_eq!(expected.at(2, 1), Some(ClippingMap::DEEP_SHADOWS));
        assert_eq!(expected.at(0, 1), Some(0));

        // without a profile and with destination sRGB: from the session's image
        let plain = temp_file("clipping-plain.tif", &rgb_tiff(w, h, &data, None));
        let backend = NativeBackend::default();
        let (mut session, _) = backend.open(&plain, Source::Jpeg).unwrap();
        assert!(matches!(&session, NativeSession::Tiff(t) if t.unconverted));
        assert_eq!(backend.clipping_map(&mut session), Some(expected.clone()));

        // with an embedded profile the opened image is converted; the map is not
        let profiled = temp_file(
            "clipping-pro-photo.tif",
            &rgb_tiff_with_icc(w, h, &data, &pro_photo_icc()),
        );
        let (mut session, size) = backend.open(&profiled, Source::Jpeg).unwrap();
        assert!(matches!(&session, NativeSession::Tiff(t) if !t.unconverted));
        let shown = whole(&backend, &mut session, size);
        let of_shown = ClippingMap::of_rgb8(w, h, &shown.data).unwrap();
        assert_eq!(
            of_shown.at(0, 1),
            Some(ClippingMap::CLIPPED_R),
            "the conversion changes the map"
        );
        assert_ne!(of_shown, expected);
        assert_eq!(backend.clipping_map(&mut session), Some(expected.clone()));
        // the session's own pixels are not changed
        assert_eq!(whole(&backend, &mut session, size), shown);

        // without a profile but with another destination: also the file's own values
        let backend = NativeBackend::default().with_monitor_profile(Profile::adobe_rgb());
        let (mut session, _) = backend.open(&plain, Source::Jpeg).unwrap();
        assert!(matches!(&session, NativeSession::Tiff(t) if !t.unconverted));
        assert_eq!(backend.clipping_map(&mut session), Some(expected));
    }

    #[test]
    fn a_raw_clipping_map_keeps_its_layout() {
        let raw = pw_libraw::ClippingMap {
            cell: 3,
            width: 2,
            height: 2,
            data: vec![1, 2, 4, 8],
        };
        let map = raw_clipping_map(raw.clone());
        assert_eq!(
            (map.cell, map.width, map.height, &map.data),
            (raw.cell, raw.width, raw.height, &raw.data)
        );
        // the bits mean the same in both
        assert_eq!(ClippingMap::CLIPPED, pw_libraw::ClippingMap::CLIPPED);
        assert_eq!(
            ClippingMap::DEEP_SHADOWS,
            pw_libraw::ClippingMap::DEEP_SHADOWS
        );
    }

    // ---------- an embedded profile that cannot be used (ADR 0016 §5, T73) ----------

    /// Bytes that are no ICC profile.
    fn garbage_icc() -> Vec<u8> {
        (0..100u32).map(|i| (i * 37 % 251) as u8).collect()
    }

    /// Whether `path` opened with the default backend reports an unusable profile.
    fn opened_profile_unusable(path: &Path) -> bool {
        let backend = NativeBackend::default();
        let (session, _) = backend.open(path, Source::Jpeg).unwrap();
        backend.profile_unusable(&session)
    }

    #[test]
    fn a_tiff_with_a_garbage_icc_profile_is_flagged() {
        let (w, h) = (6, 4);
        let path = temp_file(
            "garbage-icc.tif",
            &rgb_tiff_with_icc(w, h, &pixels(w, h), &garbage_icc()),
        );
        assert!(opened_profile_unusable(&path));
    }

    #[test]
    fn a_tiff_with_a_pro_photo_icc_profile_or_without_icc_is_not_flagged() {
        let (w, h) = (6, 4);
        let with = temp_file(
            "flag-pro-photo-icc.tif",
            &rgb_tiff_with_icc(w, h, &pixels(w, h), &pro_photo_icc()),
        );
        assert!(!opened_profile_unusable(&with));
        let without = temp_file("flag-no-icc.tif", &rgb_tiff(w, h, &pixels(w, h), None));
        assert!(!opened_profile_unusable(&without));
    }

    #[test]
    fn a_jpeg_with_a_garbage_app2_icc_profile_is_flagged() {
        let path = temp_file(
            "app2-garbage.jpg",
            &fixture_with_segment(&app2_icc(&garbage_icc())),
        );
        assert!(opened_profile_unusable(&path));
    }

    #[test]
    fn a_jpeg_without_app2_or_with_a_usable_icc_profile_is_not_flagged() {
        let plain = temp_file("flag-plain.jpg", jpeg_fixture::SMALL);
        assert!(!opened_profile_unusable(&plain));
        let usable = temp_file(
            "flag-app2-adobe.jpg",
            &fixture_with_segment(&app2_icc(&adobe_rgb_icc())),
        );
        assert!(!opened_profile_unusable(&usable));
    }
}
