//! View geometry (ADR 0008 §2.1, ADR 0009 §3.4, §5.1). Plain arithmetic, no GUI types.
//!
//! - Images are decoded in **sensor orientation**. Orientation is applied only when drawing: every
//!   corner of the visible part of the upright image is mapped to sensor coordinates, and from there
//!   to texture coordinates, so rotation costs nothing.
//! - A **scale** is physical screen pixels per image pixel: 1.0 is 100 %, 0.5 is 50 %.
//! - A **centre** is a relative point of the upright image, `[0, 1]` on each axis (V6). It is clamped
//!   per image so that the view stays inside the image, and an image smaller than the view is
//!   centred.

/// Image size in sensor orientation, and LibRaw's flip code (0, 3, 5 or 6; anything else is
/// treated as 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageSize {
    pub w: u32,
    pub h: u32,
    pub flip: u8,
}

impl ImageSize {
    /// Size of the upright (oriented) image.
    pub fn oriented(self) -> (u32, u32) {
        if self.flip == 5 || self.flip == 6 {
            (self.h, self.w)
        } else {
            (self.w, self.h)
        }
    }

    /// Maps a point of the upright image (edge coordinates, `0..=ow`, `0..=oh`) to sensor
    /// coordinates.
    pub fn to_sensor(self, x: f64, y: f64) -> (f64, f64) {
        let (w, h) = (f64::from(self.w), f64::from(self.h));
        match self.flip {
            3 => (w - x, h - y),
            6 => (y, h - x),
            5 => (w - y, x),
            _ => (x, y),
        }
    }

    /// Whether the size is usable (both sides above zero).
    pub fn is_valid(self) -> bool {
        self.w > 0 && self.h > 0
    }
}

/// LibRaw's flip code for an EXIF/TIFF orientation (mirrored orientations are shown unmirrored,
/// ADR 0024 §2.3).
pub fn flip_of_orientation(orientation: u16) -> u8 {
    match orientation {
        3 => 3,
        6 => 6,
        8 => 5,
        _ => 0,
    }
}

/// The Exif orientation after the image is turned 90° clockwise (`clockwise`) or anticlockwise
/// (ADR 0024 §2.3). A mirrored value stays mirrored. A value outside `1..=8` is taken as 1.
pub fn turned(orientation: u16, clockwise: bool) -> u16 {
    let o = if (1..=8).contains(&orientation) {
        orientation
    } else {
        1
    };
    if clockwise {
        match o {
            1 => 6,
            6 => 3,
            3 => 8,
            8 => 1,
            2 => 7,
            7 => 4,
            4 => 5,
            _ => 2, // 5
        }
    } else {
        match o {
            1 => 8,
            8 => 3,
            3 => 6,
            6 => 1,
            2 => 5,
            5 => 4,
            4 => 7,
            _ => 2, // 7
        }
    }
}

/// A rectangle of whole pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    /// Whether `inner` lies completely inside `self`.
    pub fn contains(self, inner: Rect) -> bool {
        inner.x >= self.x
            && inner.y >= self.y
            && u64::from(inner.x) + u64::from(inner.w) <= u64::from(self.x) + u64::from(self.w)
            && u64::from(inner.y) + u64::from(inner.h) <= u64::from(self.y) + u64::from(self.h)
    }

    /// Number of pixels.
    pub fn area(self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }
}

/// A rectangle with fractional edges: `[x, y, w, h]`.
pub type RectF = [f64; 4];

/// Physical size of the image area in pixels; at least 1×1.
pub fn view_size(view_px: [u32; 2]) -> (f64, f64) {
    (f64::from(view_px[0].max(1)), f64::from(view_px[1].max(1)))
}

/// The scale at which the whole upright image fits the view.
pub fn fit_scale(size: ImageSize, view_px: [u32; 2]) -> f64 {
    let (ow, oh) = size.oriented();
    let (vw, vh) = view_size(view_px);
    if ow == 0 || oh == 0 {
        return 1.0;
    }
    (vw / f64::from(ow)).min(vh / f64::from(oh))
}

fn sane_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn sane_centre(c: f64) -> f64 {
    if c.is_finite() {
        c.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// The visible part of the upright image at `scale`, for a view of `view_px` physical pixels,
/// centred on the relative point `centre` and kept inside the image (V6).
pub fn visible(size: ImageSize, view_px: [u32; 2], scale: f64, centre: [f64; 2]) -> RectF {
    let (ow, oh) = size.oriented();
    let (vw, vh) = view_size(view_px);
    let s = sane_scale(scale);
    let axis = |c: f64, full: f64, view: f64| -> (f64, f64) {
        let part = (view / s).min(full);
        let start = (sane_centre(c) * full - part / 2.0).clamp(0.0, (full - part).max(0.0));
        (start, part)
    };
    let (x, w) = axis(centre[0], f64::from(ow), vw);
    let (y, h) = axis(centre[1], f64::from(oh), vh);
    [x, y, w, h]
}

/// The relative centre of a visible rectangle.
pub fn centre_of(size: ImageSize, vis: RectF) -> [f64; 2] {
    let (ow, oh) = size.oriented();
    let c = |start: f64, part: f64, full: u32| {
        if full == 0 {
            0.5
        } else {
            (start + part / 2.0) / f64::from(full)
        }
    };
    [c(vis[0], vis[2], ow), c(vis[1], vis[3], oh)]
}

/// Where a visible rectangle lands in the view, in physical pixels: `[x0, y0, x1, y1]`. A part
/// smaller than the view is centred.
pub fn dest(vis: RectF, view_px: [u32; 2], scale: f64) -> [f64; 4] {
    let (vw, vh) = view_size(view_px);
    let s = sane_scale(scale);
    let (w, h) = (vis[2] * s, vis[3] * s);
    let (x, y) = ((vw - w) / 2.0, (vh - h) / 2.0);
    [x, y, x + w, y + h]
}

/// The sensor rectangle that covers an upright rectangle, rounded outwards, with an even origin (to
/// keep the Bayer phase, as P4/P5) and inside the image. At least 1×1.
pub fn sensor_box(size: ImageSize, r: RectF) -> Rect {
    let corners = [
        size.to_sensor(r[0], r[1]),
        size.to_sensor(r[0] + r[2], r[1]),
        size.to_sensor(r[0], r[1] + r[3]),
        size.to_sensor(r[0] + r[2], r[1] + r[3]),
    ];
    let min_x = corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
    let min_y = corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|c| c.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = corners
        .iter()
        .map(|c| c.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let clamp = |v: f64, hi: u32| -> u32 {
        if v.is_finite() {
            v.clamp(0.0, f64::from(hi)) as u32
        } else {
            0
        }
    };
    let x0 = clamp(min_x.floor(), size.w) & !1;
    let y0 = clamp(min_y.floor(), size.h) & !1;
    let x1 = clamp(max_x.ceil(), size.w)
        .max(x0.saturating_add(1))
        .min(size.w);
    let y1 = clamp(max_y.ceil(), size.h)
        .max(y0.saturating_add(1))
        .min(size.h);
    let x0 = x0.min(x1.saturating_sub(1));
    let y0 = y0.min(y1.saturating_sub(1));
    Rect::new(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1))
}

/// The whole image as a sensor rectangle.
pub fn whole(size: ImageSize) -> Rect {
    Rect::new(0, 0, size.w, size.h)
}

/// Texture coordinates for the corners (top left, top right, bottom right, bottom left) of the
/// upright rectangle `r`, for a texture that holds the sensor region `tex`.
pub fn corner_uvs(size: ImageSize, tex: Rect, r: RectF) -> [[f32; 2]; 4] {
    let (x0, y0, x1, y1) = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
    let tw = f64::from(tex.w.max(1));
    let th = f64::from(tex.h.max(1));
    let uv = |x: f64, y: f64| {
        let (sx, sy) = size.to_sensor(x, y);
        [
            ((sx - f64::from(tex.x)) / tw) as f32,
            ((sy - f64::from(tex.y)) / th) as f32,
        ]
    };
    [uv(x0, y0), uv(x1, y0), uv(x1, y1), uv(x0, y1)]
}

/// The centre after zooming from `from` to `to`, so that the image point under `anchor` (view
/// pixels; `None` = the view centre) stays under it. The result is clamped for this image.
pub fn zoom_centre(
    size: ImageSize,
    view_px: [u32; 2],
    from: f64,
    to: f64,
    centre: [f64; 2],
    anchor: Option<[f64; 2]>,
) -> [f64; 2] {
    let (vw, vh) = view_size(view_px);
    let (ow, oh) = size.oriented();
    let (s0, s1) = (sane_scale(from), sane_scale(to));
    let vis0 = visible(size, view_px, s0, centre);
    let d0 = dest(vis0, view_px, s0);
    let a = anchor.unwrap_or([vw / 2.0, vh / 2.0]);
    let axis = |a: f64, d0: f64, v0: f64, view: f64, full: u32| -> f64 {
        let full = f64::from(full);
        let p = v0 + (a - d0) / s0; // image point under the anchor
        let part = (view / s1).min(full);
        if part >= full || full <= 0.0 {
            return 0.5;
        }
        let start = (p - a / s1).clamp(0.0, full - part);
        (start + part / 2.0) / full
    };
    [
        axis(a[0], d0[0], vis0[0], vw, ow),
        axis(a[1], d0[1], vis0[1], vh, oh),
    ]
}

/// The centre after dragging the image by `delta` view pixels at `scale` (the image follows the
/// pointer), clamped for this image.
pub fn pan_centre(
    size: ImageSize,
    view_px: [u32; 2],
    scale: f64,
    centre: [f64; 2],
    delta: [f64; 2],
) -> [f64; 2] {
    let s = sane_scale(scale);
    let vis = visible(size, view_px, s, centre);
    let (ow, oh) = size.oriented();
    let (vw, vh) = view_size(view_px);
    let axis = |start: f64, part: f64, d: f64, full: u32, view: f64| -> f64 {
        let full = f64::from(full);
        if part >= full || full <= 0.0 || view / s >= full {
            return 0.5;
        }
        let d = if d.is_finite() { d } else { 0.0 };
        let start = (start - d / s).clamp(0.0, full - part);
        (start + part / 2.0) / full
    };
    [
        axis(vis[0], vis[2], delta[0], ow, vw),
        axis(vis[1], vis[3], delta[1], oh, vh),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const A7R: ImageSize = ImageSize {
        w: 9568,
        h: 6376,
        flip: 5,
    };
    const FLAT: ImageSize = ImageSize {
        w: 8660,
        h: 5784,
        flip: 0,
    };
    const VIEW: [u32; 2] = [1920, 1080];

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn orientation_swaps_sides_for_5_and_6() {
        assert_eq!(A7R.oriented(), (6376, 9568));
        assert_eq!(FLAT.oriented(), (8660, 5784));
        assert_eq!(ImageSize { flip: 3, ..FLAT }.oriented(), (8660, 5784));
        assert_eq!(ImageSize { flip: 6, ..A7R }.oriented(), (6376, 9568));
    }

    #[test]
    fn corners_map_onto_sensor_corners() {
        // flip 5 (Orientation 8): the upright top-left corner is the sensor's top-right corner
        assert_eq!(A7R.to_sensor(0.0, 0.0), (9568.0, 0.0));
        assert_eq!(A7R.to_sensor(6376.0, 9568.0), (0.0, 6376.0));
        assert_eq!(
            ImageSize { flip: 6, ..A7R }.to_sensor(0.0, 0.0),
            (0.0, 6376.0)
        );
        assert_eq!(
            ImageSize { flip: 3, ..FLAT }.to_sensor(0.0, 0.0),
            (8660.0, 5784.0)
        );
        assert_eq!(
            ImageSize { flip: 9, ..FLAT }.to_sensor(1.0, 2.0),
            (1.0, 2.0)
        );
    }

    #[test]
    fn visible_region_at_100_percent_is_centred_and_clamped() {
        assert_eq!(
            visible(FLAT, VIEW, 1.0, [0.5, 0.5]),
            [3370.0, 2352.0, 1920.0, 1080.0]
        );
        assert_eq!(
            visible(FLAT, VIEW, 1.0, [0.0, 1.0]),
            [0.0, 4704.0, 1920.0, 1080.0]
        );
        let small = ImageSize {
            w: 800,
            h: 600,
            flip: 0,
        };
        assert_eq!(
            visible(small, VIEW, 1.0, [0.3, 0.3]),
            [0.0, 0.0, 800.0, 600.0]
        );
        assert_eq!(visible(FLAT, VIEW, 1.0, [f64::NAN, 0.5])[0], 3370.0);
    }

    #[test]
    fn visible_region_follows_the_scale() {
        let v = visible(FLAT, VIEW, 0.5, [0.5, 0.5]);
        assert_eq!((v[2], v[3]), (3840.0, 2160.0));
        let v = visible(FLAT, VIEW, 4.0, [0.5, 0.5]);
        assert_eq!((v[2], v[3]), (480.0, 270.0));
        // at fit the whole image is visible and centred in the view
        let s = fit_scale(FLAT, VIEW);
        let v = visible(FLAT, VIEW, s, [0.1, 0.9]);
        assert!(close(v[0], 0.0) && close(v[2], 8660.0));
        let d = dest(v, VIEW, s);
        assert!(close(d[1], 0.0) && close(d[3], 1080.0));
        assert!(close(d[0] + d[2], 1920.0));
        // nonsense scales are treated as 100 %
        assert_eq!(visible(FLAT, VIEW, 0.0, [0.5, 0.5])[2], 1920.0);
    }

    #[test]
    fn sensor_box_of_a_vertical_image_swaps_and_keeps_an_even_origin() {
        let v = visible(A7R, VIEW, 1.0, [0.5, 0.5]);
        let b = sensor_box(A7R, v);
        assert_eq!((b.w, b.h), (1080, 1920));
        assert_eq!(b.x % 2, 0);
        assert_eq!(b.y % 2, 0);
        assert!(b.x + b.w <= A7R.w && b.y + b.h <= A7R.h);
        // a fractional region is covered completely
        let b = sensor_box(FLAT, [10.5, 20.25, 100.5, 50.5]);
        assert!(b.x <= 10 && b.y <= 20 && b.x + b.w >= 111 && b.y + b.h >= 71);
        assert_eq!(sensor_box(FLAT, [0.0, 0.0, 8660.0, 5784.0]), whole(FLAT));
    }

    #[test]
    fn rect_contains() {
        let outer = Rect::new(10, 10, 100, 100);
        assert!(outer.contains(Rect::new(10, 10, 100, 100)));
        assert!(outer.contains(Rect::new(20, 20, 10, 10)));
        assert!(!outer.contains(Rect::new(5, 20, 10, 10)));
        assert!(!outer.contains(Rect::new(100, 100, 11, 10)));
        assert!(!Rect::new(u32::MAX - 1, 0, 1, 1).contains(Rect::new(u32::MAX - 1, 0, 2, 1)));
    }

    #[test]
    fn uvs_of_a_whole_flat_image_are_the_unit_square() {
        let uv = corner_uvs(FLAT, whole(FLAT), [0.0, 0.0, 8660.0, 5784.0]);
        assert_eq!(uv, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    }

    #[test]
    fn uvs_of_a_vertical_image_are_rotated() {
        // flip 5: upright top-left = texture top-right; upright top-right = texture bottom-right
        let uv = corner_uvs(A7R, whole(A7R), [0.0, 0.0, 6376.0, 9568.0]);
        assert_eq!(uv, [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]);
    }

    #[test]
    fn uvs_of_a_region_stay_inside_its_sensor_box() {
        for flip in [0u8, 3, 5, 6] {
            let size = ImageSize { flip, ..A7R };
            for (scale, c) in [(1.0, [0.3, 0.7]), (0.7, [0.0, 0.0]), (3.0, [1.0, 0.2])] {
                let v = visible(size, VIEW, scale, c);
                let b = sensor_box(size, v);
                for uv in corner_uvs(size, b, v) {
                    assert!(
                        (-1e-6..=1.0 + 1e-6).contains(&uv[0])
                            && (-1e-6..=1.0 + 1e-6).contains(&uv[1]),
                        "flip {flip} scale {scale}: {uv:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn zooming_keeps_the_anchor_point_still() {
        let size = FLAT;
        let anchor = [400.0, 300.0];
        let s0 = 1.0;
        let c0 = [0.4, 0.6];
        let v0 = visible(size, VIEW, s0, c0);
        let p = v0[0] + anchor[0] / s0; // image x under the anchor
        let c1 = zoom_centre(size, VIEW, s0, 2.0, c0, Some(anchor));
        let v1 = visible(size, VIEW, 2.0, c1);
        assert!(close(v1[0] + anchor[0] / 2.0, p));
        // zooming out to a scale where the image is smaller than the view centres it
        assert_eq!(
            zoom_centre(size, VIEW, 1.0, 0.1, c0, Some(anchor)),
            [0.5, 0.5]
        );
        // without an anchor the view centre stays
        let c2 = zoom_centre(size, VIEW, 1.0, 2.0, [0.5, 0.5], None);
        assert!(close(c2[0], 0.5) && close(c2[1], 0.5));
    }

    #[test]
    fn zooming_in_from_fit_at_a_point_lands_on_that_point() {
        let s0 = fit_scale(FLAT, VIEW);
        let d = dest(visible(FLAT, VIEW, s0, [0.5, 0.5]), VIEW, s0);
        // the pointer at the image's top-left quarter point on screen
        let anchor = [d[0] + (d[2] - d[0]) * 0.25, d[1] + (d[3] - d[1]) * 0.25];
        let c = zoom_centre(FLAT, VIEW, s0, 1.0, [0.5, 0.5], Some(anchor));
        let v = visible(FLAT, VIEW, 1.0, c);
        assert!(close(v[0] + anchor[0], 8660.0 * 0.25));
        assert!(close(v[1] + anchor[1], 5784.0 * 0.25));
    }

    #[test]
    fn panning_moves_with_the_pointer_and_stops_at_the_edges() {
        let c = pan_centre(FLAT, VIEW, 1.0, [0.5, 0.5], [100.0, 0.0]);
        let before = visible(FLAT, VIEW, 1.0, [0.5, 0.5]);
        let after = visible(FLAT, VIEW, 1.0, c);
        assert!(close(after[0], before[0] - 100.0));
        // far past the edge: clamped so the view stays inside the image
        let c = pan_centre(FLAT, VIEW, 1.0, [0.5, 0.5], [1.0e9, -1.0e9]);
        let v = visible(FLAT, VIEW, 1.0, c);
        assert!(close(v[0], 0.0) && close(v[1] + v[3], 5784.0));
        // at fit there is nothing to pan
        assert_eq!(
            pan_centre(FLAT, VIEW, fit_scale(FLAT, VIEW), [0.3, 0.3], [50.0, 50.0]),
            [0.5, 0.5]
        );
    }

    #[test]
    fn turned_follows_the_exif_table_both_ways() {
        let cw = [
            (1, 6),
            (6, 3),
            (3, 8),
            (8, 1),
            (2, 7),
            (7, 4),
            (4, 5),
            (5, 2),
        ];
        for (from, to) in cw {
            assert_eq!(turned(from, true), to, "clockwise from {from}");
            assert_eq!(turned(to, false), from, "anticlockwise from {to}");
        }
        let acw = [
            (1, 8),
            (8, 3),
            (3, 6),
            (6, 1),
            (2, 5),
            (5, 4),
            (4, 7),
            (7, 2),
        ];
        for (from, to) in acw {
            assert_eq!(turned(from, false), to, "anticlockwise from {from}");
        }
    }

    #[test]
    fn turned_four_times_or_back_gives_the_start() {
        for o in 1..=8u16 {
            let mut t = o;
            for _ in 0..4 {
                t = turned(t, true);
            }
            assert_eq!(t, o, "four clockwise turns from {o}");
            assert_eq!(turned(turned(o, true), false), o, "there and back from {o}");
            assert_eq!(turned(turned(o, false), true), o, "back and there from {o}");
        }
    }

    #[test]
    fn turned_takes_values_outside_the_range_as_1() {
        assert_eq!(turned(0, true), 6);
        assert_eq!(turned(9, true), 6);
        assert_eq!(turned(0, false), 8);
        assert_eq!(turned(9, false), 8);
    }
}
