//! The decoder seen by the core. The real one is LibRaw with PPG ([`crate::libraw`], feature
//! `libraw`); the tests use a simulated one, so the core is tested without native libraries.

use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::exposure::Ev;
use crate::geometry::{ImageSize, Rect};

/// An 8-bit RGB image (3 bytes per pixel, rows top to bottom), in sensor orientation.
#[derive(Clone, PartialEq, Eq)]
pub struct Rgb {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl fmt::Debug for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rgb({}x{})", self.width, self.height)
    }
}

impl Rgb {
    /// Whether the buffer holds exactly `width × height` RGB pixels.
    pub fn is_consistent(&self) -> bool {
        u64::from(self.width) * u64::from(self.height) * 3 == self.data.len() as u64
    }
}

/// Why a file or a view could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError {
    pub message: String,
    /// The file's RAW data is missing or corrupt (G3). An intact preview is then never replaced.
    pub damaged: bool,
    /// The file has no embedded JPEG to show in JPEG mode (G4; T118), so a program can say so
    /// without reading `message`.
    pub no_embedded_jpeg: bool,
}

impl DecodeError {
    pub fn new(message: impl Into<String>) -> DecodeError {
        DecodeError {
            message: message.into(),
            damaged: false,
            no_embedded_jpeg: false,
        }
    }

    pub fn damaged(message: impl Into<String>) -> DecodeError {
        DecodeError {
            message: message.into(),
            damaged: true,
            no_embedded_jpeg: false,
        }
    }

    /// A file without an embedded JPEG, opened in JPEG mode (T118).
    pub fn no_embedded_jpeg(message: impl Into<String>) -> DecodeError {
        DecodeError {
            message: message.into(),
            damaged: false,
            no_embedded_jpeg: true,
        }
    }
}

/// The text of the error of a render that a [`Stop`] ended (T328).
const CANCELLED_TEXT: &str = "cancelled: the render was no longer wanted";

impl DecodeError {
    /// A render that its [`Stop`] ended (T328): not a failure of the file. It has no field of its
    /// own (the struct is built as a literal elsewhere); [`is_cancelled`](Self::is_cancelled)
    /// compares the constant text.
    pub fn cancelled() -> DecodeError {
        DecodeError::new(CANCELLED_TEXT)
    }

    /// Whether this is [`cancelled`](Self::cancelled)'s error.
    pub fn is_cancelled(&self) -> bool {
        !self.damaged && !self.no_embedded_jpeg && self.message == CANCELLED_TEXT
    }
}

/// A flag the core sets to end a render that is no longer wanted (T328). Clones share the flag.
#[derive(Clone, Debug, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    pub fn new() -> Stop {
        Stop::default()
    }

    /// Asks the render to end.
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Clears the flag again: the render is wanted again before it ended (T328). A render that
    /// has already ended cancelled stays cancelled.
    pub fn resume(&self) {
        self.0.store(false, Ordering::Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DecodeError {}

/// Where an image's pixels come from: the display mode (G4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// A RAW rendering (LibRaw, PPG).
    Raw,
    /// The camera's largest embedded JPEG, or a plain JPEG file itself.
    Jpeg,
}

impl Source {
    /// The other display mode.
    pub fn other(self) -> Source {
        match self {
            Source::Raw => Source::Jpeg,
            Source::Jpeg => Source::Raw,
        }
    }
}

/// Where an image's statistics were computed from; the panel names it (G1, ADR 0017 U7-Q5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatisticsOrigin {
    /// The unpacked RAW data, before any rendering.
    RawData,
    /// The embedded JPEG (or a plain JPEG file) as decoded, before the conversion to the screen
    /// profile.
    EmbeddedJpeg,
    /// A TIFF file's own decoded image, before the conversion to the screen profile.
    Tiff,
}

/// The histogram and the exposure statistics of an opened image (ADR 0017 §2.6, U7), per colour
/// channel in the order R, G, B.
#[derive(Clone, PartialEq)]
pub struct Statistics {
    pub origin: StatisticsOrigin,
    /// 256 bins per channel. RAW data: the values after the black level is subtracted and the
    /// camera's white balance applied, scaled so the white level is the top bin, on an sRGB-like
    /// curve (U7-Q3). JPEG and TIFF: the 8-bit values themselves.
    pub histogram: [[u64; 256]; 3],
    /// The share (0 to 1) of each channel's pixels at the clipping level (U7-Q4, U7-Q5): RAW data
    /// at or above the white level before white balance; JPEG and TIFF at 255.
    pub clipped: [f64; 3],
    /// The share (0 to 1) of the pixels in the deepest shadows: RAW data with every channel below
    /// 1/256 of the white level above black; JPEG and TIFF with every channel at 0.
    pub deep_shadows: f64,
}

impl fmt::Debug for Statistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Statistics")
            .field("origin", &self.origin)
            .field("clipped", &self.clipped)
            .field("deep_shadows", &self.deep_shadows)
            .finish_non_exhaustive()
    }
}

impl Statistics {
    /// The statistics of 8-bit RGB pixels (3 bytes per pixel) as they are: a bin per value,
    /// clipped at 255 and deepest shadows at 0 in every channel (U7-Q5). Bytes after the last
    /// whole pixel are not counted.
    pub fn of_rgb8(origin: StatisticsOrigin, data: &[u8]) -> Statistics {
        let mut histogram = [[0u64; 256]; 3];
        let mut dark = 0u64;
        let mut pixels = 0u64;
        for &[r, g, b] in data.as_chunks::<3>().0 {
            for (bins, value) in histogram.iter_mut().zip([r, g, b]) {
                if let Some(bin) = bins.get_mut(usize::from(value)) {
                    *bin += 1;
                }
            }
            if r == 0 && g == 0 && b == 0 {
                dark += 1;
            }
            pixels += 1;
        }
        let share = |count: u64| {
            if pixels == 0 {
                0.0
            } else {
                count as f64 / pixels as f64
            }
        };
        Statistics {
            origin,
            clipped: [
                share(histogram[0][255]),
                share(histogram[1][255]),
                share(histogram[2][255]),
            ],
            deep_shadows: share(dark),
            histogram,
        }
    }
}

/// What is known of an opened image's statistics once they were asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageStatistics {
    Available(Arc<Statistics>),
    /// The file has none (for example RAW data that is not from an RGB sensor).
    Unavailable,
}

/// Where the clipped highlights and the deepest shadows of an opened image are (ADR 0017 §2.6,
/// U7b), by the rules of its [`Statistics`]; the layout of `pw_libraw::ClippingMap`.
///
/// The image's pixel grid (sensor orientation, no flip applied) is divided into cells of
/// `cell` × `cell` pixels, counted from its top left corner. The map has `width` × `height`
/// cells (the image's size divided by `cell`, rounded up; a cell at the right or bottom edge
/// holds fewer pixels and is judged on those), and `data` holds one byte per cell, row by row:
/// - [`CLIPPED_R`](Self::CLIPPED_R), [`CLIPPED_G`](Self::CLIPPED_G),
///   [`CLIPPED_B`](Self::CLIPPED_B) (bits 0–2): at least one pixel of the cell has that channel
///   at the clipping level (RAW data: at or above the white level, before white balance; JPEG
///   and TIFF: at 255);
/// - [`DEEP_SHADOWS`](Self::DEEP_SHADOWS) (bit 3): every pixel of the cell is in the deepest
///   shadows in every channel (RAW data: below 1/256 of the white level above black; JPEG and
///   TIFF: at 0);
/// - bits 4–7 are 0.
#[derive(Clone, PartialEq, Eq)]
pub struct ClippingMap {
    pub cell: u32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl fmt::Debug for ClippingMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ClippingMap({}x{} cells of {}x{}, {} bytes)",
            self.width,
            self.height,
            self.cell,
            self.cell,
            self.data.len()
        )
    }
}

impl ClippingMap {
    /// A pixel of the cell has R at the clipping level.
    pub const CLIPPED_R: u8 = 1;
    /// A pixel of the cell has G at the clipping level.
    pub const CLIPPED_G: u8 = 2;
    /// A pixel of the cell has B at the clipping level.
    pub const CLIPPED_B: u8 = 4;
    /// The clipped bit of each channel, in the order R, G, B (as [`Statistics::clipped`]).
    pub const CLIPPED: [u8; 3] = [Self::CLIPPED_R, Self::CLIPPED_G, Self::CLIPPED_B];
    /// Every pixel of the cell is in the deepest shadows.
    pub const DEEP_SHADOWS: u8 = 8;
    /// The side of a cell of a map made from 8-bit pixels (ADR 0017 U7b-Q7).
    pub const RGB8_CELL: u32 = 2;

    /// The map of `width` × `height` 8-bit RGB pixels (3 bytes per pixel, rows top to bottom),
    /// in cells of 2 × 2 pixels: a channel's bit when at least one pixel of the cell has that
    /// channel at 255, the deep-shadow bit when every channel of every pixel of the cell is 0
    /// (U7-Q5). `None` for an image without pixels or a buffer shorter than the image.
    pub fn of_rgb8(width: u32, height: u32, data: &[u8]) -> Option<ClippingMap> {
        if width == 0 || height == 0 {
            return None;
        }
        let cell = Self::RGB8_CELL as usize;
        let (columns, rows) = (
            width.div_ceil(Self::RGB8_CELL),
            height.div_ceil(Self::RGB8_CELL),
        );
        let row_bytes = usize::try_from(width).ok()?.checked_mul(3)?;
        let image_bytes = row_bytes.checked_mul(usize::try_from(height).ok()?)?;
        let pixels = data.get(..image_bytes)?;
        let cells = usize::try_from(columns)
            .ok()?
            .checked_mul(usize::try_from(rows).ok()?)?;
        // every cell holds at least one pixel; a pixel above 0 takes the shadow bit away
        let mut map = vec![Self::DEEP_SHADOWS; cells];
        // the pixel rows of one row of cells (one row fewer at the bottom edge of an odd height)
        let bands = pixels.chunks(row_bytes.checked_mul(cell)?);
        for (band, cell_row) in bands.zip(map.chunks_mut(columns as usize)) {
            for row in band.chunks(row_bytes) {
                // the pixels of one cell in this row (one fewer at the right edge of an odd width)
                for (group, value) in row.chunks(3 * cell).zip(cell_row.iter_mut()) {
                    for &[r, g, b] in group.as_chunks::<3>().0 {
                        for (channel, bit) in [r, g, b].into_iter().zip(Self::CLIPPED) {
                            if channel == u8::MAX {
                                *value |= bit;
                            }
                        }
                        if r != 0 || g != 0 || b != 0 {
                            *value &= !Self::DEEP_SHADOWS;
                        }
                    }
                }
            }
        }
        Some(ClippingMap {
            cell: Self::RGB8_CELL,
            width: columns,
            height: rows,
            data: map,
        })
    }

    /// The byte of the cell in column `x` and row `y` (in cells); `None` outside the map.
    pub fn at(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y as usize)
            .checked_mul(self.width as usize)?
            .checked_add(x as usize)?;
        self.data.get(index).copied()
    }
}

/// Where the edges of high local contrast of an opened image are, for the focus marking (ADR 0017
/// §2.6, U9a): made from the half-size render of the whole image, the pixels the view draws at
/// fit (U9-Q2), not from the RAW data.
///
/// The layout of [`ClippingMap`]: the image's pixel grid (sensor orientation, no flip applied) is
/// divided into cells of `cell` × `cell` pixels, counted from its top left corner; the map has
/// `width` × `height` cells, and `data` holds one byte per cell, row by row:
/// [`EDGE`](Self::EDGE) when the cell is on an edge, 0 when it is not.
#[derive(Clone, PartialEq, Eq)]
pub struct FocusMap {
    pub cell: u32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl fmt::Debug for FocusMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FocusMap({}x{} cells of {}x{}, {} bytes)",
            self.width,
            self.height,
            self.cell,
            self.cell,
            self.data.len()
        )
    }
}

impl FocusMap {
    /// The cell is on an edge of high local contrast.
    pub const EDGE: u8 = 1;
    /// The least Sobel gradient `|gx| + |gy|` of the luminance (0–255) that marks an edge (U9-Q3;
    /// the owner tunes it after the hand check).
    pub const FOCUS_THRESHOLD: u16 = 160;
    /// The side of a cell: a pixel of the half-size render covers 2 × 2 pixels of the image's
    /// grid.
    pub const HALF_CELL: u32 = 2;

    /// The map of a half-size render of `width` × `height` 8-bit RGB pixels (3 bytes per pixel,
    /// rows top to bottom), one cell per pixel of the render: a pixel gets [`EDGE`](Self::EDGE)
    /// when the Sobel gradient `|gx| + |gy|` of the luminance `(54·R + 183·G + 19·B) >> 8` over
    /// its 3 × 3 neighbours is at least [`FOCUS_THRESHOLD`](Self::FOCUS_THRESHOLD). The outermost
    /// rows and columns have no full neighbourhood and are never marked. `None` for an image
    /// narrower or lower than 3 pixels, or a buffer shorter than the image.
    pub fn of_rgb8(width: u32, height: u32, data: &[u8]) -> Option<FocusMap> {
        if width < 3 || height < 3 {
            return None;
        }
        let w = usize::try_from(width).ok()?;
        let h = usize::try_from(height).ok()?;
        let pixels = w.checked_mul(h)?;
        let rgb = data.get(..pixels.checked_mul(3)?)?;
        let luminance: Vec<u8> = rgb
            .as_chunks::<3>()
            .0
            .iter()
            .map(|&[r, g, b]| {
                let y = (54 * u32::from(r) + 183 * u32::from(g) + 19 * u32::from(b)) >> 8;
                u8::try_from(y).unwrap_or(u8::MAX)
            })
            .collect();
        let threshold = i32::from(Self::FOCUS_THRESHOLD);
        let mut map = vec![0u8; pixels];
        let rows: Vec<&[u8]> = luminance.chunks(w).collect();
        // every row but the first and the last, and in it every pixel but the first and the last:
        // those with a full neighbourhood
        for y in 1..h - 1 {
            let (Some(above), Some(row), Some(below)) =
                (rows.get(y - 1), rows.get(y), rows.get(y + 1))
            else {
                continue;
            };
            let start = y * w;
            let Some(out) = map.get_mut(start + 1..start + w - 1) else {
                continue;
            };
            let neighbourhoods = above.windows(3).zip(row.windows(3)).zip(below.windows(3));
            for (((top, middle), bottom), value) in neighbourhoods.zip(out.iter_mut()) {
                let (&[t0, t1, t2], &[m0, _, m2], &[b0, b1, b2]) = (top, middle, bottom) else {
                    continue;
                };
                let [t0, t1, t2, m0, m2, b0, b1, b2] =
                    [t0, t1, t2, m0, m2, b0, b1, b2].map(i32::from);
                let gx = (t2 + 2 * m2 + b2) - (t0 + 2 * m0 + b0);
                let gy = (b0 + 2 * b1 + b2) - (t0 + 2 * t1 + t2);
                if gx.abs() + gy.abs() >= threshold {
                    *value = Self::EDGE;
                }
            }
        }
        Some(FocusMap {
            cell: Self::HALF_CELL,
            width,
            height,
            data: map,
        })
    }

    /// The byte of the cell in column `x` and row `y` (in cells); `None` outside the map.
    pub fn at(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y as usize)
            .checked_mul(self.width as usize)?
            .checked_add(x as usize)?;
        self.data.get(index).copied()
    }
}

/// A decoder: open once, then render any number of views from the session. For RAW, opening
/// unpacks the file; for JPEG it reads the JPEG and its header, and the full decode is made when
/// a view needs it.
pub trait Backend: Send + Sync + 'static {
    /// An opened file. Used by one thread at a time.
    type Session: Send + 'static;

    /// Opens `path` as `source`. Returns the session and the size of its pixel grid (sensor
    /// orientation for RAW, the JPEG's own grid for JPEG) with the orientation to apply.
    fn open(&self, path: &Path, source: Source) -> Result<(Self::Session, ImageSize), DecodeError>;

    /// The whole image at half size, before the conversion to the display
    /// ([`to_display`](Self::to_display)).
    fn render_half(&self, session: &mut Self::Session) -> Result<Rgb, DecodeError>;

    /// Exactly the region `r` at full resolution (RAW: demosaiced with PPG), before the
    /// conversion to the display ([`to_display`](Self::to_display)).
    fn render_region(&self, session: &mut Self::Session, r: Rect) -> Result<Rgb, DecodeError>;

    /// Like [`render_half`](Self::render_half), rendered with the temporary exposure `exposure`
    /// (ADR 0027 §3.4, W5), and ends early with [`DecodeError::cancelled`] when `stop` is set
    /// (T328). The default looks at neither: a backend that has no exposure (and a JPEG or a TIFF
    /// file) renders as without one. A backend that stops a render while its decoder works may
    /// spend the session ([`session_usable`](Self::session_usable)).
    fn render_half_stoppable(
        &self,
        session: &mut Self::Session,
        _exposure: Ev,
        _stop: &Stop,
    ) -> Result<Rgb, DecodeError> {
        self.render_half(session)
    }

    /// Like [`render_region`](Self::render_region), with the exposure and stoppable as
    /// [`render_half_stoppable`](Self::render_half_stoppable) (T328); the default ignores both
    /// as that one's does.
    fn render_region_stoppable(
        &self,
        session: &mut Self::Session,
        r: Rect,
        _exposure: Ev,
        _stop: &Stop,
    ) -> Result<Rgb, DecodeError> {
        self.render_region(session, r)
    }

    /// The camera's own brightness of the opened image in EV, the brightness the view adds (for
    /// `NativeBackend` the file's value with its sign turned; not limited to [`Ev`]'s range;
    /// ADR 0027 W5-Q5, §4.4). Asked once after a successful open. The default has none.
    fn camera_exposure(&self, _session: &Self::Session) -> Option<f64> {
        None
    }

    /// Whether the session can render again. False after a stop (or a failure) made the decoder
    /// free its data. The core asks it only after a cancelled render, and then drops such a
    /// session and opens the file again (T328); a session spent by another failure is kept
    /// (`docs/tasks/known-limits.md`).
    fn session_usable(&self, _session: &Self::Session) -> bool {
        true
    }

    /// Converts a rendered and reduced view of the opened image to the display (the monitor
    /// profile), after the reduction (ADR 0016, ADR 0027 §3.2, W1-Q4). The default leaves it
    /// unchanged.
    fn to_display(&self, _session: &Self::Session, img: Rgb) -> Result<Rgb, DecodeError> {
        Ok(img)
    }

    /// Whether the file's embedded colour profile was present but could not be used, so its
    /// colours rest on a fallback (ADR 0016 §5). Asked once after a successful open.
    fn profile_unusable(&self, _session: &Self::Session) -> bool {
        false
    }

    /// The histogram and the exposure statistics of the opened image (ADR 0017 §2.6, U7): RAW
    /// from the unpacked RAW data, JPEG and TIFF from the decoded image before the conversion to
    /// the screen profile. The core asks once per opened image, after its first view is on the
    /// screen (U7-Q6), so this may take as long as a render. `None` when the file has none.
    fn statistics(&self, _session: &mut Self::Session) -> Option<Statistics> {
        None
    }

    /// Where the clipped highlights and the deepest shadows of the opened image are (ADR 0017
    /// §2.6, U7b), from the same data and by the same rules as [`statistics`](Self::statistics).
    /// The core asks only while an overlay wants it (U7b-Q8), after the image's statistics, so
    /// this may take as long as a render. `None` when the file has none, or on a failure.
    fn clipping_map(&self, _session: &mut Self::Session) -> Option<ClippingMap> {
        None
    }

    /// The memory one open session holds, for the core budget.
    fn session_bytes(&self, size: ImageSize, source: Source) -> u64 {
        let px = u64::from(size.w) * u64::from(size.h);
        match source {
            // unpacked Bayer data, 16 bits per pixel (P5's estimate)
            Source::Raw => px * 2,
            // the full decode, kept for 100 % views
            Source::Jpeg => px * 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb8_statistics_count_every_value_and_the_two_ends() {
        // four pixels: white, black, a red clipped only, and a mid grey; one byte left over
        let data = [255, 255, 255, 0, 0, 0, 255, 10, 0, 128, 128, 128, 77];
        let s = Statistics::of_rgb8(StatisticsOrigin::Tiff, &data);
        assert_eq!(s.origin, StatisticsOrigin::Tiff);
        for channel in &s.histogram {
            assert_eq!(channel.iter().sum::<u64>(), 4);
        }
        assert_eq!(s.histogram[0][255], 2);
        assert_eq!(s.histogram[1][255], 1);
        assert_eq!(s.histogram[1][10], 1);
        assert_eq!(s.histogram[2][0], 2);
        assert_eq!(s.histogram[2][128], 1);
        assert_eq!(s.clipped, [0.5, 0.25, 0.25]);
        // only the black pixel has every channel at 0
        assert_eq!(s.deep_shadows, 0.25);
    }

    #[test]
    fn rgb8_statistics_of_no_pixels_are_zero() {
        let s = Statistics::of_rgb8(StatisticsOrigin::EmbeddedJpeg, &[]);
        assert_eq!(s.clipped, [0.0; 3]);
        assert_eq!(s.deep_shadows, 0.0);
        assert!(s.histogram.iter().flatten().all(|n| *n == 0));
    }

    #[test]
    fn a_backend_without_statistics_has_none() {
        struct Plain;
        impl Backend for Plain {
            type Session = ();
            fn open(&self, _: &Path, _: Source) -> Result<((), ImageSize), DecodeError> {
                Err(DecodeError::new("no files"))
            }
            fn render_half(&self, _: &mut ()) -> Result<Rgb, DecodeError> {
                Err(DecodeError::new("no image"))
            }
            fn render_region(&self, _: &mut (), _: Rect) -> Result<Rgb, DecodeError> {
                Err(DecodeError::new("no image"))
            }
        }
        assert_eq!(Plain.statistics(&mut ()), None);
        assert_eq!(Plain.clipping_map(&mut ()), None);
    }

    // ---------- the clipping map of 8-bit pixels (ADR 0017 U7b, T147) ----------

    const R: u8 = ClippingMap::CLIPPED_R;
    const G: u8 = ClippingMap::CLIPPED_G;
    const B: u8 = ClippingMap::CLIPPED_B;
    const DARK: u8 = ClippingMap::DEEP_SHADOWS;

    /// `width` × `height` mid-grey pixels with the pixels of `set` (x, y, value) put in.
    fn grey_with(width: u32, height: u32, set: &[(u32, u32, [u8; 3])]) -> Vec<u8> {
        let mut data = vec![128u8; (width * height * 3) as usize];
        for &(x, y, value) in set {
            let at = ((y * width + x) * 3) as usize;
            data[at..at + 3].copy_from_slice(&value);
        }
        data
    }

    #[test]
    fn the_clipping_bits_are_those_of_the_raw_map() {
        assert_eq!(ClippingMap::CLIPPED, [1, 2, 4]);
        assert_eq!(ClippingMap::DEEP_SHADOWS, 8);
        assert_eq!(ClippingMap::RGB8_CELL, 2);
    }

    #[test]
    fn a_channel_at_255_sets_only_its_bit_in_its_cell() {
        // 8 × 4 pixels = 4 × 2 cells; one pixel per marked cell, in different corners of the cell
        let data = grey_with(
            8,
            4,
            &[
                (0, 0, [255, 254, 1]),   // cell (0, 0): R
                (3, 1, [0, 255, 200]),   // cell (1, 0): G
                (4, 3, [254, 0, 255]),   // cell (2, 1): B
                (7, 2, [255, 255, 255]), // cell (3, 1): all three
                (6, 0, [255, 0, 0]),     // cell (3, 0): R from one pixel ...
                (7, 1, [0, 0, 255]),     // ... and B from another
            ],
        );
        let map = ClippingMap::of_rgb8(8, 4, &data).unwrap();
        assert_eq!((map.cell, map.width, map.height), (2, 4, 2));
        assert_eq!(map.data, vec![R, G, 0, R | B, 0, 0, B, R | G | B]);
        assert_eq!(map.at(1, 0), Some(G));
        assert_eq!(map.at(3, 1), Some(R | G | B));
        assert_eq!(map.at(4, 0), None);
        assert_eq!(map.at(0, 2), None);
    }

    #[test]
    fn a_cell_of_all_zero_pixels_is_in_the_deep_shadows_and_one_pixel_above_zero_is_not() {
        let black = [0u8; 3];
        let mut set = Vec::new();
        // cells (0, 0) and (1, 0) all black; in cell (1, 0) one channel of one pixel is 1
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1), (2, 0), (3, 0), (2, 1)] {
            set.push((x, y, black));
        }
        set.push((3, 1, [0, 1, 0]));
        // cell (0, 1): three black pixels and a grey one
        for (x, y) in [(0, 2), (1, 2), (0, 3)] {
            set.push((x, y, black));
        }
        let data = grey_with(4, 4, &set);
        let map = ClippingMap::of_rgb8(4, 4, &data).unwrap();
        assert_eq!(map.data, vec![DARK, 0, 0, 0]);
        // a black pixel next to a clipped one: clipped, not dark
        let data = grey_with(
            2,
            2,
            &[
                (0, 0, black),
                (1, 0, black),
                (0, 1, black),
                (1, 1, [0, 0, 255]),
            ],
        );
        assert_eq!(ClippingMap::of_rgb8(2, 2, &data).unwrap().data, vec![B]);
    }

    #[test]
    fn an_odd_width_and_height_give_edge_cells_judged_on_the_pixels_they_hold() {
        // 5 × 3 pixels = 3 × 2 cells: the right column of cells is one pixel wide, the bottom row
        // one pixel high, and the corner cell is the single pixel (4, 2)
        let black = [0u8; 3];
        let data = grey_with(
            5,
            3,
            &[
                (4, 0, black), // cell (2, 0): both of its pixels black
                (4, 1, black),
                (0, 2, [255, 9, 9]), // cell (0, 1): R in its left pixel
                (3, 2, [9, 255, 9]), // cell (1, 1): G in its right pixel
                (4, 2, black),       // cell (2, 1): its only pixel black
            ],
        );
        let map = ClippingMap::of_rgb8(5, 3, &data).unwrap();
        assert_eq!((map.cell, map.width, map.height), (2, 3, 2));
        assert_eq!(map.data, vec![0, 0, DARK, R, G, DARK]);
        // one pixel is one cell
        let map = ClippingMap::of_rgb8(1, 1, &[255, 0, 255]).unwrap();
        assert_eq!((map.width, map.height, map.data), (1, 1, vec![R | B]));
    }

    #[test]
    fn no_pixels_or_too_few_bytes_give_no_map() {
        assert_eq!(ClippingMap::of_rgb8(0, 4, &[]), None);
        assert_eq!(ClippingMap::of_rgb8(4, 0, &[]), None);
        assert_eq!(ClippingMap::of_rgb8(2, 2, &[0; 11]), None);
        // bytes after the image are not read
        let mut data = vec![0u8; 12];
        data.extend_from_slice(&[255; 6]);
        assert_eq!(ClippingMap::of_rgb8(2, 2, &data).unwrap().data, vec![DARK]);
    }

    // ---------- the focus map of the half-size render (ADR 0017 U9a, T167) ----------

    const EDGE: u8 = FocusMap::EDGE;

    /// `width` × `height` grey pixels, `left` up to column `step` (not included) and `right` from
    /// there on: a vertical step whose luminance is the grey value itself.
    fn vertical_step(width: u32, height: u32, step: u32, left: u8, right: u8) -> Vec<u8> {
        let mut data = Vec::new();
        for _ in 0..height {
            for x in 0..width {
                let v = if x < step { left } else { right };
                data.extend_from_slice(&[v, v, v]);
            }
        }
        data
    }

    /// The columns marked in each row.
    fn marked_columns(map: &FocusMap) -> Vec<Vec<u32>> {
        (0..map.height)
            .map(|y| {
                (0..map.width)
                    .filter(|&x| map.at(x, y) == Some(EDGE))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_flat_image_has_no_edges() {
        for value in [0, 128, 255] {
            let data = vec![value; 6 * 5 * 3];
            let map = FocusMap::of_rgb8(6, 5, &data).unwrap();
            assert!(map.data.iter().all(|v| *v == 0), "value {value}");
        }
        // also a colour, whatever its luminance
        let data = [200u8, 30, 90].repeat(4 * 4);
        let map = FocusMap::of_rgb8(4, 4, &data).unwrap();
        assert!(map.data.iter().all(|v| *v == 0));
    }

    #[test]
    fn a_vertical_step_marks_the_cells_on_both_sides_of_it_and_nothing_else() {
        // the step lies between columns 3 and 4: both get 4 · 100 = 400, the rest 0
        let data = vertical_step(8, 5, 4, 20, 120);
        let map = FocusMap::of_rgb8(8, 5, &data).unwrap();
        let none: Vec<u32> = Vec::new();
        assert_eq!(
            marked_columns(&map),
            vec![none.clone(), vec![3, 4], vec![3, 4], vec![3, 4], none]
        );
        // the other way round (dark on the right) is the same edge
        let data = vertical_step(8, 5, 4, 120, 20);
        assert_eq!(FocusMap::of_rgb8(8, 5, &data).unwrap(), map);
        // a horizontal step too, turned
        let mut data = Vec::new();
        for y in 0..8 {
            let v: u8 = if y < 4 { 20 } else { 120 };
            data.extend_from_slice(&[v; 5 * 3]);
        }
        let map = FocusMap::of_rgb8(5, 8, &data).unwrap();
        for y in 0..8 {
            for x in 0..5 {
                let edge = (1..4).contains(&x) && (y == 3 || y == 4);
                assert_eq!(
                    map.at(x, y),
                    Some(if edge { EDGE } else { 0 }),
                    "({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn the_outermost_rows_and_columns_are_never_marked() {
        // stripes two pixels wide, black and white: every inner pixel is next to a step, so every
        // inner pixel is on an edge, and no outer one is marked
        let mut data = Vec::new();
        for _ in 0..6u32 {
            for x in 0..7u32 {
                let v = if x % 4 < 2 { 0 } else { 255 };
                data.extend_from_slice(&[v, v, v]);
            }
        }
        let map = FocusMap::of_rgb8(7, 6, &data).unwrap();
        for y in 0..6 {
            for x in 0..7 {
                let outer = x == 0 || y == 0 || x == 6 || y == 5;
                let want = if outer { 0 } else { EDGE };
                assert_eq!(map.at(x, y), Some(want), "({x}, {y})");
            }
        }
        // steps at the very edges: the pixels next to them are marked, the edge pixels are not
        let data = vertical_step(6, 4, 1, 255, 0);
        assert_eq!(
            marked_columns(&FocusMap::of_rgb8(6, 4, &data).unwrap()),
            vec![vec![], vec![1], vec![1], vec![]]
        );
    }

    #[test]
    fn a_step_just_below_the_threshold_is_not_marked_and_one_at_it_is() {
        let threshold = u32::from(FocusMap::FOCUS_THRESHOLD);
        // a step of luminance d gives 4 · d on both sides of it
        let below = u8::try_from((threshold - 1) / 4).unwrap();
        let at = u8::try_from(threshold.div_ceil(4)).unwrap();
        assert!(4 * u32::from(below) < threshold);
        assert!(4 * u32::from(at) >= threshold);
        let marked = |d: u8| {
            let data = vertical_step(6, 3, 3, 10, 10 + d);
            marked_columns(&FocusMap::of_rgb8(6, 3, &data).unwrap())
        };
        let none: Vec<u32> = Vec::new();
        assert_eq!(
            marked(below),
            vec![none.clone(), none.clone(), none.clone()]
        );
        assert_eq!(marked(at), vec![none.clone(), vec![2, 3], none.clone()]);
        assert_eq!(marked(at + 1), vec![none.clone(), vec![2, 3], none]);
    }

    #[test]
    fn the_luminance_weighs_the_channels_as_rec_709() {
        // pure green of 255 has a luminance of 182 (183 · 255 >> 8), pure blue 18: only the green
        // step is above the threshold
        let step = |colour: [u8; 3]| {
            let mut data = Vec::new();
            for _ in 0..3 {
                for x in 0..4 {
                    data.extend_from_slice(&if x < 2 { [0, 0, 0] } else { colour });
                }
            }
            FocusMap::of_rgb8(4, 3, &data).unwrap().at(1, 1)
        };
        assert_eq!(step([0, 255, 0]), Some(EDGE));
        assert_eq!(step([0, 0, 255]), Some(0));
    }

    #[test]
    fn the_map_has_one_cell_of_two_pixels_per_pixel_of_the_render() {
        let data = vertical_step(9, 7, 4, 0, 255);
        let map = FocusMap::of_rgb8(9, 7, &data).unwrap();
        assert_eq!((map.cell, map.width, map.height), (2, 9, 7));
        assert_eq!(map.data.len(), 9 * 7);
        assert_eq!(map.at(9, 0), None);
        assert_eq!(map.at(0, 7), None);
        // the debug text does not print the data
        assert_eq!(format!("{map:?}"), "FocusMap(9x7 cells of 2x2, 63 bytes)");
    }

    #[test]
    fn too_small_an_image_or_too_few_bytes_give_no_focus_map() {
        assert_eq!(FocusMap::of_rgb8(2, 2, &[0; 12]), None);
        assert_eq!(FocusMap::of_rgb8(2, 5, &[0; 30]), None);
        assert_eq!(FocusMap::of_rgb8(5, 2, &[0; 30]), None);
        assert_eq!(FocusMap::of_rgb8(0, 0, &[]), None);
        assert_eq!(FocusMap::of_rgb8(3, 3, &[0; 26]), None);
        // the smallest: one inner pixel; bytes after the image are not read
        let mut data = vertical_step(3, 3, 1, 0, 255);
        data.extend_from_slice(&[255; 6]);
        let map = FocusMap::of_rgb8(3, 3, &data).unwrap();
        assert_eq!(map.data, vec![0, 0, 0, 0, EDGE, 0, 0, 0, 0]);
    }
}
