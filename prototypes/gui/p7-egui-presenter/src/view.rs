//! View geometry (GUI_REQUIREMENTS GR3, GR11; ADR 0008 §2.1, §14.3). Plain Rust, no egui types.
//!
//! LibRaw renders in sensor orientation (`user_flip = 0`). Orientation is applied only when drawing:
//! every corner of the visible part of the upright image is mapped to sensor coordinates, and from
//! there to texture coordinates. Rotation is therefore free (a different UV per corner).

/// Image size in sensor orientation, and LibRaw's flip (0, 3, 5 or 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageSize {
    pub w: u32,
    pub h: u32,
    pub flip: i32,
}

impl ImageSize {
    /// Size of the upright (oriented) image.
    pub fn oriented(self) -> (u32, u32) {
        if self.flip == 5 || self.flip == 6 { (self.h, self.w) } else { (self.w, self.h) }
    }

    /// Maps a point of the upright image (edge coordinates, 0..=ow, 0..=oh) to sensor coordinates.
    pub fn to_sensor(self, x: f32, y: f32) -> (f32, f32) {
        let (w, h) = (self.w as f32, self.h as f32);
        match self.flip {
            3 => (w - x, h - y),
            6 => (y, h - x),
            5 => (w - y, x),
            _ => (x, y),
        }
    }
}

/// A rectangle `[x, y, w, h]` in pixels.
pub type Rect = [u32; 4];

/// The visible part of the upright image at 100 % for a view of `view_px` physical pixels, centred on
/// the relative point `center` and kept inside the image (V6).
pub fn visible_oriented(size: ImageSize, view_px: [u32; 2], center: [f32; 2]) -> Rect {
    let (ow, oh) = size.oriented();
    let (w, h) = (view_px[0].min(ow).max(1), view_px[1].min(oh).max(1));
    let axis = |c: f32, full: u32, part: u32| -> u32 {
        let c = if c.is_finite() { c.clamp(0.0, 1.0) } else { 0.5 };
        let start = (c * full as f32 - part as f32 / 2.0).round();
        start.clamp(0.0, (full - part) as f32) as u32
    };
    [axis(center[0], ow, w), axis(center[1], oh, h), w, h]
}

/// The sensor rectangle that covers an oriented rectangle, with an even origin (keeps the Bayer phase,
/// as P4/P5) and inside the image.
pub fn sensor_box(size: ImageSize, r: Rect) -> Rect {
    let corners = [
        size.to_sensor(r[0] as f32, r[1] as f32),
        size.to_sensor((r[0] + r[2]) as f32, r[1] as f32),
        size.to_sensor(r[0] as f32, (r[1] + r[3]) as f32),
        size.to_sensor((r[0] + r[2]) as f32, (r[1] + r[3]) as f32),
    ];
    let min_x = corners.iter().map(|c| c.0).fold(f32::INFINITY, f32::min).max(0.0) as u32;
    let min_y = corners.iter().map(|c| c.1).fold(f32::INFINITY, f32::min).max(0.0) as u32;
    let max_x = (corners.iter().map(|c| c.0).fold(0.0, f32::max) as u32).min(size.w);
    let max_y = (corners.iter().map(|c| c.1).fold(0.0, f32::max) as u32).min(size.h);
    let x0 = min_x & !1;
    let y0 = min_y & !1;
    [x0, y0, max_x.saturating_sub(x0).max(1), max_y.saturating_sub(y0).max(1)]
}

/// The 100 % view region (exact) and the ring around it (twice the size, for panning), in sensor
/// coordinates, as P5's Zoom and Ring views.
pub fn zoom_regions(size: ImageSize, view_px: [u32; 2], center: [f32; 2]) -> (Rect, Rect) {
    let exact = sensor_box(size, visible_oriented(size, view_px, center));
    let ring = sensor_box(size, visible_oriented(size, [view_px[0].saturating_mul(2), view_px[1].saturating_mul(2)], center));
    (exact, ring)
}

/// Whether `outer` contains `inner`.
pub fn contains(outer: Rect, inner: Rect) -> bool {
    inner[0] >= outer[0]
        && inner[1] >= outer[1]
        && inner[0] + inner[2] <= outer[0] + outer[2]
        && inner[1] + inner[3] <= outer[1] + outer[3]
}

/// Texture coordinates for the corners (top left, top right, bottom right, bottom left) of the
/// oriented rectangle `r`, for a texture that holds the sensor region `tex`.
pub fn corner_uvs(size: ImageSize, tex: Rect, r: Rect) -> [[f32; 2]; 4] {
    let (x0, y0, x1, y1) = (r[0] as f32, r[1] as f32, (r[0] + r[2]) as f32, (r[1] + r[3]) as f32);
    let uv = |x: f32, y: f32| {
        let (sx, sy) = size.to_sensor(x, y);
        [(sx - tex[0] as f32) / tex[2].max(1) as f32, (sy - tex[1] as f32) / tex[3].max(1) as f32]
    };
    [uv(x0, y0), uv(x1, y0), uv(x1, y1), uv(x0, y1)]
}

/// Destination of the fitted upright image in a view of `view` points: `[x0, y0, x1, y1]`.
pub fn fit_dest(size: ImageSize, view: [f32; 2]) -> Option<[f32; 4]> {
    let (ow, oh) = size.oriented();
    if !(view[0] > 0.0 && view[1] > 0.0 && ow > 0 && oh > 0) {
        return None;
    }
    let s = (view[0] / ow as f32).min(view[1] / oh as f32);
    let (w, h) = (ow as f32 * s, oh as f32 * s);
    let (x, y) = ((view[0] - w) / 2.0, (view[1] - h) / 2.0);
    Some([x, y, x + w, y + h])
}

/// Destination of a 100 % region of `r` pixels, centred in a view of `view` points, at `ppp` physical
/// pixels per point.
pub fn actual_dest(r: Rect, view: [f32; 2], ppp: f32) -> [f32; 4] {
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 };
    let (w, h) = (r[2] as f32 / ppp, r[3] as f32 / ppp);
    let (x, y) = ((view[0] - w) / 2.0, (view[1] - h) / 2.0);
    [x, y, x + w, y + h]
}

#[cfg(test)]
mod tests {
    use super::*;

    const A7R: ImageSize = ImageSize { w: 9568, h: 6376, flip: 5 };
    const FLAT: ImageSize = ImageSize { w: 8660, h: 5784, flip: 0 };

    #[test]
    fn orientation_swaps_sides_for_5_and_6() {
        assert_eq!(A7R.oriented(), (6376, 9568));
        assert_eq!(FLAT.oriented(), (8660, 5784));
        assert_eq!(ImageSize { flip: 3, ..FLAT }.oriented(), (8660, 5784));
    }

    #[test]
    fn corners_map_onto_sensor_corners() {
        // flip 5 (Orientation 8): the upright top-left corner is the sensor's top-right corner
        assert_eq!(A7R.to_sensor(0.0, 0.0), (9568.0, 0.0));
        assert_eq!(A7R.to_sensor(6376.0, 9568.0), (0.0, 6376.0));
        let s6 = ImageSize { flip: 6, ..A7R };
        assert_eq!(s6.to_sensor(0.0, 0.0), (0.0, 6376.0));
        let s3 = ImageSize { flip: 3, ..FLAT };
        assert_eq!(s3.to_sensor(0.0, 0.0), (8660.0, 5784.0));
    }

    #[test]
    fn visible_region_is_centred_and_clamped() {
        assert_eq!(visible_oriented(FLAT, [1920, 1080], [0.5, 0.5]), [3370, 2352, 1920, 1080]);
        assert_eq!(visible_oriented(FLAT, [1920, 1080], [0.0, 1.0]), [0, 4704, 1920, 1080]);
        // a view larger than the image shows the whole image
        let small = ImageSize { w: 800, h: 600, flip: 0 };
        assert_eq!(visible_oriented(small, [1920, 1080], [0.3, 0.3]), [0, 0, 800, 600]);
        assert_eq!(visible_oriented(FLAT, [1920, 1080], [f32::NAN, 0.5])[0], 3370);
    }

    #[test]
    fn sensor_box_of_a_vertical_image_swaps_and_keeps_an_even_origin() {
        let v = visible_oriented(A7R, [1920, 1080], [0.5, 0.5]);
        let b = sensor_box(A7R, v);
        assert_eq!((b[2], b[3]), (1080, 1920));
        assert_eq!(b[0] % 2, 0);
        assert_eq!(b[1] % 2, 0);
        assert!(b[0] + b[2] <= A7R.w && b[1] + b[3] <= A7R.h);
    }

    #[test]
    fn the_ring_contains_the_exact_region() {
        for c in [[0.5, 0.5], [0.0, 0.0], [1.0, 1.0], [0.2, 0.9]] {
            let (exact, ring) = zoom_regions(A7R, [1920, 1080], c);
            assert!(contains(ring, exact), "{c:?}: {exact:?} not in {ring:?}");
        }
    }

    #[test]
    fn uvs_of_a_whole_flat_image_are_the_unit_square() {
        let uv = corner_uvs(FLAT, [0, 0, FLAT.w, FLAT.h], [0, 0, FLAT.w, FLAT.h]);
        assert_eq!(uv, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    }

    #[test]
    fn uvs_of_a_vertical_image_are_rotated() {
        // flip 5: upright top-left = texture top-right; upright top-right = texture bottom-right
        let (ow, oh) = A7R.oriented();
        let uv = corner_uvs(A7R, [0, 0, A7R.w, A7R.h], [0, 0, ow, oh]);
        assert_eq!(uv, [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]);
    }

    #[test]
    fn uvs_of_a_region_stay_inside_the_texture() {
        let v = visible_oriented(A7R, [1920, 1080], [0.3, 0.7]);
        let (_, ring) = zoom_regions(A7R, [1920, 1080], [0.3, 0.7]);
        for c in corner_uvs(A7R, ring, v) {
            assert!((0.0..=1.0).contains(&c[0]) && (0.0..=1.0).contains(&c[1]), "{c:?}");
        }
    }

    #[test]
    fn fit_keeps_the_aspect_ratio() {
        let d = fit_dest(A7R, [1920.0, 1080.0]).unwrap_or([0.0; 4]);
        let (w, h) = (d[2] - d[0], d[3] - d[1]);
        assert!((h - 1080.0).abs() < 0.01);
        assert!((w / h - 6376.0 / 9568.0).abs() < 1e-4);
        assert_eq!(fit_dest(A7R, [0.0, 10.0]), None);
    }

    #[test]
    fn actual_size_is_one_pixel_per_physical_pixel() {
        assert_eq!(actual_dest([0, 0, 1920, 1080], [960.0, 540.0], 2.0), [0.0, 0.0, 960.0, 540.0]);
        assert_eq!(actual_dest([0, 0, 800, 600], [1920.0, 1080.0], 1.0), [560.0, 240.0, 1360.0, 840.0]);
    }
}
