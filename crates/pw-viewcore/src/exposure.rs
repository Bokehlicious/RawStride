//! The exposure of a view (ADR 0027 §3.4, W5): a temporary correction in EV that the decoder
//! applies to the RAW data while it renders, and the auto exposure's rule that proposes one from
//! the RAW statistics (A-Q11). The value is kept in thousandths of an EV, so it can be part of a
//! hashed job key.

use crate::backend::{Statistics, StatisticsOrigin};

/// An exposure in thousandths of an EV, always inside [`Ev::MIN_MILLI`]`..=`[`Ev::MAX_MILLI`]
/// (−2 to +3 EV, W5-Q2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Ev {
    milli: i32,
}

impl Ev {
    /// No correction.
    pub const ZERO: Ev = Ev { milli: 0 };
    /// The darkest correction, −2 EV (W5-Q2).
    pub const MIN_MILLI: i32 = -2000;
    /// The brightest correction, +3 EV (W5-Q2).
    pub const MAX_MILLI: i32 = 3000;

    /// `thirds` thirds of an EV, rounded to the nearest thousandth (halves away from zero) and
    /// limited to the range: 1 gives 333, 2 gives 667, 3 gives 1000.
    pub fn from_thirds(thirds: i32) -> Ev {
        let milli = (f64::from(thirds) * 1000.0 / 3.0).round();
        Self::from_milli_f64(milli)
    }

    /// `ev` EV, rounded to the nearest thousandth (halves away from zero) and limited to the
    /// range; a value that is not finite gives [`Ev::ZERO`].
    pub fn from_ev(ev: f64) -> Ev {
        if !ev.is_finite() {
            return Ev::ZERO;
        }
        Self::from_milli_f64((ev * 1000.0).round())
    }

    /// A whole number of thousandths as `f64`, limited to the range before it is made an `i32`.
    fn from_milli_f64(milli: f64) -> Ev {
        let limited = milli.clamp(f64::from(Self::MIN_MILLI), f64::from(Self::MAX_MILLI));
        // a whole number inside the range, so the conversion is exact
        Ev {
            milli: limited as i32,
        }
    }

    /// The exposure in thousandths of an EV.
    pub fn milli(self) -> i32 {
        self.milli
    }

    /// The exposure in EV.
    pub fn ev(self) -> f64 {
        f64::from(self.milli) / 1000.0
    }

    /// Whether this is no correction.
    pub fn is_zero(self) -> bool {
        self.milli == 0
    }
}

/// What the auto exposure's rule found (A-Q11, W5-Q4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoExposure {
    /// The correction in thirds of an EV, from −6 to 9 (−2 to +3 EV).
    Thirds(i32),
    /// More than 0.1 % of a channel is clipped in the RAW data: the image is not brightened.
    Clipped,
    /// The statistics are not from RAW data (an embedded JPEG, a JPEG or a TIFF file).
    NotRaw,
    /// No channel has any pixels.
    NoData,
}

/// The largest share of a channel's pixels that may be clipped in the RAW data before the rule
/// gives [`AutoExposure::Clipped`] (0.1 %).
const CLIPPED_LIMIT: f64 = 0.001;

/// The auto exposure's rule (A-Q11): the correction that puts the brightest channel's 99.9th
/// percentile of the RAW data ⅓ EV below clipping, rounded down to ⅓ EV and limited to −2..+3 EV
/// (W5-Q2). The rule is tuned in the owner's hand check. Since the percentile is never above
/// clipping, it never darkens by more than ⅓ EV (the result is at least `Thirds(-1)`); the lower
/// limit is kept for safety.
pub fn auto_exposure(stats: &Statistics) -> AutoExposure {
    if stats.origin != StatisticsOrigin::RawData {
        return AutoExposure::NotRaw;
    }
    // a share that is NaN counts as clipped, so broken statistics never brighten
    if stats
        .clipped
        .iter()
        .any(|&share| share.is_nan() || share > CLIPPED_LIMIT)
    {
        return AutoExposure::Clipped;
    }
    let Some(v) = stats
        .histogram
        .iter()
        .filter_map(percentile_999)
        .reduce(f64::max)
    else {
        return AutoExposure::NoData;
    };
    let linear = srgb_to_linear(v);
    let e = -1.0 / 3.0 - linear.log2();
    // the small nudge keeps a value a rounding error below a whole third from dropping a step
    let thirds = (3.0 * e + 1e-9).floor().clamp(-6.0, 9.0);
    // limited to −6..=9 above, so the conversion is exact
    AutoExposure::Thirds(thirds as i32)
}

/// The 99.9th percentile of one channel's histogram on the sRGB-like curve (0 to 1): the upper
/// edge `(b + 1) / 256` of the smallest bin `b` whose running sum reaches 99.9 % of the channel's
/// pixels. `None` for a channel without pixels.
fn percentile_999(bins: &[u64; 256]) -> Option<f64> {
    let total = bins.iter().fold(0u128, |sum, &n| sum + u128::from(n));
    if total == 0 {
        return None;
    }
    let mut sum = 0u128;
    for (b, &n) in bins.iter().enumerate() {
        sum += u128::from(n);
        // in integers, so no float compare decides the bin
        if sum * 1000 >= total * 999 {
            return Some((b as f64 + 1.0) / 256.0);
        }
    }
    // the last bin's running sum is the total, which always reaches it
    Some(1.0)
}

/// A value on the sRGB curve back to linear.
fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_thirds_rounds_to_thousandths_and_is_limited() {
        let cases = [
            (-7, -2000),
            (-6, -2000),
            (-1, -333),
            (0, 0),
            (1, 333),
            (2, 667),
            (3, 1000),
            (9, 3000),
            (10, 3000),
        ];
        for (thirds, milli) in cases {
            assert_eq!(Ev::from_thirds(thirds).milli(), milli, "{thirds}");
        }
        assert_eq!(Ev::from_thirds(0), Ev::ZERO);
    }

    #[test]
    fn from_ev_rounds_is_limited_and_takes_no_broken_value() {
        assert_eq!(Ev::from_ev(-2.5).milli(), -2000);
        assert_eq!(Ev::from_ev(0.4999).milli(), 500);
        assert_eq!(Ev::from_ev(3.0004).milli(), 3000);
        assert_eq!(Ev::from_ev(f64::NAN), Ev::ZERO);
        assert_eq!(Ev::from_ev(f64::INFINITY), Ev::ZERO);
    }

    #[test]
    fn ev_and_is_zero() {
        assert_eq!(Ev::from_thirds(3).ev(), 1.0);
        assert_eq!(Ev::from_ev(-1.25).ev(), -1.25);
        assert!(Ev::ZERO.is_zero());
        assert!(Ev::default().is_zero());
        assert!(!Ev::from_thirds(-1).is_zero());
    }

    /// RAW statistics with the given histogram and nothing clipped.
    fn raw(histogram: [[u64; 256]; 3]) -> Statistics {
        Statistics {
            origin: StatisticsOrigin::RawData,
            histogram,
            clipped: [0.0; 3],
            deep_shadows: 0.0,
        }
    }

    /// Every channel with `n` pixels in bin `b`.
    fn all_in(b: usize, n: u64) -> [[u64; 256]; 3] {
        let mut h = [[0u64; 256]; 3];
        for channel in &mut h {
            channel[b] = n;
        }
        h
    }

    #[test]
    fn everything_at_the_top_darkens_by_a_third() {
        // v = 1, L = 1, e = −⅓, 3e = −1
        assert_eq!(
            auto_exposure(&raw(all_in(255, 1000))),
            AutoExposure::Thirds(-1)
        );
    }

    #[test]
    fn everything_at_the_bottom_brightens_by_the_most() {
        // v = 1/256, L = v / 12.92 ≈ 0.000302, e ≈ 11.36, limited to 9 thirds
        assert_eq!(
            auto_exposure(&raw(all_in(0, 1000))),
            AutoExposure::Thirds(9)
        );
    }

    #[test]
    fn more_than_a_thousandth_clipped_is_clipped() {
        let mut s = raw(all_in(10, 1000));
        s.clipped = [0.0, 0.002, 0.0];
        assert_eq!(auto_exposure(&s), AutoExposure::Clipped);
        s.clipped = [0.001, 0.001, 0.001];
        assert_ne!(auto_exposure(&s), AutoExposure::Clipped);
        s.clipped = [0.0, f64::NAN, 0.0];
        assert_eq!(auto_exposure(&s), AutoExposure::Clipped);
    }

    #[test]
    fn statistics_not_from_raw_data_are_not_raw() {
        let mut s = raw(all_in(10, 1000));
        s.origin = StatisticsOrigin::EmbeddedJpeg;
        assert_eq!(auto_exposure(&s), AutoExposure::NotRaw);
        s.origin = StatisticsOrigin::Tiff;
        assert_eq!(auto_exposure(&s), AutoExposure::NotRaw);
    }

    #[test]
    fn no_pixels_is_no_data() {
        assert_eq!(auto_exposure(&raw([[0; 256]; 3])), AutoExposure::NoData);
    }

    #[test]
    fn a_value_just_above_a_whole_third_is_rounded_down() {
        // bin 187: v = 188/256 = 0.734375, L = ((v + 0.055) / 1.055)^2.4 ≈ 0.498510,
        // log2 L ≈ −1.004305, e = −⅓ + 1.004305 ≈ 0.670971, 3e ≈ 2.0129 → 2 thirds
        assert_eq!(
            auto_exposure(&raw(all_in(187, 1000))),
            AutoExposure::Thirds(2)
        );
        // bin 169: v = 170/256 = 0.6640625, L ≈ 0.398505, log2 L ≈ −1.327329,
        // e ≈ 0.993996, 3e ≈ 2.9820 → 2 thirds (rounded down, not to the nearest 3)
        assert_eq!(
            auto_exposure(&raw(all_in(169, 1000))),
            AutoExposure::Thirds(2)
        );
    }

    #[test]
    fn one_bright_pixel_in_a_thousand_does_not_decide() {
        // 999 of 1000 pixels in bin 187 and 1 in bin 255: the running sum reaches 999 (999 × 1000
        // >= 1000 × 999) in bin 187, so the result is bin 187's (2 thirds), not bin 255's (−1)
        let mut h = all_in(187, 999);
        for channel in &mut h {
            channel[255] = 1;
        }
        assert_eq!(auto_exposure(&raw(h)), AutoExposure::Thirds(2));
    }

    #[test]
    fn the_brightest_channel_decides() {
        // R and B in bin 0 alone would give 9 thirds; G in bin 187 gives 2
        let mut h = all_in(0, 1000);
        h[1] = [0; 256];
        h[1][187] = 1000;
        assert_eq!(auto_exposure(&raw(h)), AutoExposure::Thirds(2));
    }
}
