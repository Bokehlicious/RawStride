//! Temporary white balance and the RAW area of a click on grey (ADR 0027 §3.5, W6).

use std::sync::Arc;

use crate::geometry::{ImageSize, Rect, sensor_box};

/// White balance for one render. Red and blue gains are in thousandths relative to green.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Wb {
    #[default]
    Shot,
    Mul {
        r: u32,
        b: u32,
    },
}

impl Wb {
    /// Quantise finite, positive gains, keeping the largest at most 64 times the smallest.
    pub fn from_multipliers([r, g, b]: [f32; 3]) -> Option<Self> {
        if ![r, g, b].into_iter().all(|v| v.is_finite() && v > 0.0)
            || f64::from(r.max(g).max(b)) > 64.0 * f64::from(r.min(g).min(b))
        {
            return None;
        }
        let r = ((r / g) * 1000.0).round() as u32;
        let b = ((b / g) * 1000.0).round() as u32;
        // Rounding can push a balance outside the range accepted by the decoder.
        (r.max(b).max(1000) <= 64 * r.min(b).min(1000)).then_some(Self::Mul { r, b })
    }

    pub fn multipliers(self) -> Option<[f32; 3]> {
        match self {
            Self::Shot => None,
            Self::Mul { r, b } => Some([r as f32 / 1000.0, 1.0, b as f32 / 1000.0]),
        }
    }

    pub fn is_shot(self) -> bool {
        matches!(self, Self::Shot)
    }
}

/// A named white balance from the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WbPreset {
    pub name: String,
    pub wb: Wb,
}

/// The white balance values and measurement support available when a file is opened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WbInfo {
    pub presets: Arc<[WbPreset]>,
    pub daylight: Option<Wb>,
    pub camera: Option<Wb>,
    pub measurable: bool,
}

const GREY_SQUARE_PX: f64 = 15.0;
const GREY_MIN_PHOTOSITES: u32 = 12;

/// The sensor area around a click in upright image edge coordinates, at the shown scale.
pub fn grey_area(size: ImageSize, [x, y]: [f64; 2], scale: f64) -> Option<Rect> {
    let (ow, oh) = size.oriented();
    let (ow, oh) = (f64::from(ow), f64::from(oh));
    if !size.is_valid()
        || !scale.is_finite()
        || scale <= 0.0
        || !(0.0..=ow).contains(&x)
        || !(0.0..=oh).contains(&y)
    {
        return None;
    }

    let radius = (GREY_SQUARE_PX / 2.0) / scale;
    // Bound the corners in upright coordinates so even a very small scale stays finite.
    let (left, top) = ((x - radius).max(0.0), (y - radius).max(0.0));
    let (right, bottom) = ((x + radius).min(ow), (y + radius).min(oh));
    let area = sensor_box(size, [left, top, right - left, bottom - top]);

    let grow = |origin: u32, side: u32, image_side: u32| {
        let grown = side.max(GREY_MIN_PHOTOSITES).min(image_side);
        let centre = f64::from(origin) + f64::from(side) / 2.0;
        let start = (((centre - f64::from(grown) / 2.0) / 2.0).round() * 2.0) as u32;
        (start.min((image_side - grown) & !1), grown)
    };
    let (x, w) = grow(area.x, area.w, size.w);
    let (y, h) = grow(area.y, area.h, size.h);
    Some(Rect::new(x, y, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_balance_gains_are_quantised_relative_to_green() {
        assert_eq!(
            Wb::from_multipliers([2473.0, 1024.0, 1606.0]),
            Some(Wb::Mul { r: 2415, b: 1568 })
        );
        let unity = Wb::from_multipliers([1.0; 3]).unwrap();
        assert_eq!(unity, Wb::Mul { r: 1000, b: 1000 });
        assert!(!unity.is_shot());
        assert_eq!(
            Wb::Mul { r: 2000, b: 500 }.multipliers(),
            Some([2.0, 1.0, 0.5])
        );
        assert_eq!(Wb::Shot.multipliers(), None);
        assert_eq!(Wb::default(), Wb::Shot);
        assert!(Wb::Shot.is_shot());
    }

    #[test]
    fn white_balance_rejects_invalid_gains_before_and_after_rounding() {
        for r in [0.0, -1.0, f32::NAN, f32::INFINITY, 64.01] {
            assert_eq!(Wb::from_multipliers([r, 1.0, 1.0]), None, "{r}");
        }
        assert!(Wb::from_multipliers([64.0, 1.0, 1.0]).is_some());
        assert_eq!(Wb::from_multipliers([64.0, 41.0, 1.0]), None);
    }

    const SIZE: ImageSize = ImageSize {
        w: 6000,
        h: 4000,
        flip: 0,
    };

    #[test]
    fn grey_area_depends_on_the_shown_scale_and_keeps_a_minimum_size() {
        for (scale, rect) in [
            (1.0, Rect::new(2992, 1992, 16, 16)),
            (8.0, Rect::new(2994, 1994, 12, 12)),
            (0.25, Rect::new(2970, 1970, 60, 60)),
        ] {
            assert_eq!(grey_area(SIZE, [3000.0, 2000.0], scale), Some(rect));
        }
    }

    #[test]
    fn grey_area_moves_inside_the_sensor_at_the_edges() {
        assert_eq!(
            grey_area(SIZE, [0.0, 0.0], 1.0),
            Some(Rect::new(0, 0, 12, 12))
        );
        let corner = [6000.0, 4000.0];
        let area = Some(Rect::new(5988, 3988, 12, 12));
        assert_eq!(grey_area(SIZE, corner, 1.0), area);
        let odd = ImageSize { w: 6001, ..SIZE };
        assert_eq!(grey_area(odd, corner, 1.0), area);
        assert_eq!(grey_area(odd, [6001.0, 4000.0], 1.0), area);
        assert_eq!(
            grey_area(ImageSize { w: 8, ..SIZE }, [4.0, 2000.0], 8.0),
            Some(Rect::new(0, 1994, 8, 12))
        );
    }

    #[test]
    fn grey_area_uses_sensor_coordinates_for_rotated_images() {
        for (flip, point) in [
            (6, [2000.0, 3000.0]),
            (5, [2000.0, 3000.0]),
            (3, [3000.0, 2000.0]),
            (6, [1200.0, 3500.0]),
            (5, [1200.0, 3500.0]),
            (3, [3500.0, 1200.0]),
        ] {
            let size = ImageSize { flip, ..SIZE };
            let area = grey_area(size, point, 1.0).unwrap();
            let (x, y) = size.to_sensor(point[0], point[1]);
            assert_eq!((area.w, area.h), (16, 16));
            assert!((f64::from(area.x) + 8.0 - x).abs() <= 1.0);
            assert!((f64::from(area.y) + 8.0 - y).abs() <= 1.0);
        }
    }

    #[test]
    fn grey_area_requires_a_valid_image_point_and_scale() {
        assert_eq!(grey_area(SIZE, [-1.0, 20.0], 1.0), None);
        assert_eq!(grey_area(SIZE, [1.0, f64::NAN], 1.0), None);
        assert_eq!(grey_area(SIZE, [1.0, 20.0], 0.0), None);
        assert_eq!(grey_area(SIZE, [1.0, 20.0], f64::INFINITY), None);
        assert_eq!(
            grey_area(ImageSize { w: 0, ..SIZE }, [0.0, 20.0], 1.0),
            None
        );
    }
}
