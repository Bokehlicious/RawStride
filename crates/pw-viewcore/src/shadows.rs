//! The shadows of a view (ADR 0027 §3.4, W5-Q6, A-Q10): a tone curve that lifts the dark values of
//! a view after the render and any reduction, and before the sharpening and the conversion to the
//! display (W5-Q3). The same curve is applied to each channel's 8-bit values as they are.

use std::ops::RangeInclusive;

use crate::backend::Rgb;

/// The amounts the user can choose, in percent.
pub const AMOUNT_RANGE: RangeInclusive<u8> = 0..=100;
/// The amount when nothing else is set.
pub const DEFAULT_AMOUNT: u8 = 50;

/// The curve for `amount` as a table from each 8-bit value to its lifted value (an amount above
/// 100 counts as 100). With `x = v / 255` and `s = amount / 100`, the curve is
/// `y = x + 1.7 · s · x · (1 − x)³`: it keeps 0 and 255, never lowers a value, is monotonic (its
/// slope is at least `1 − 0.425 s > 0`), and lifts most at `x = ¼`, the top of the darkest
/// quarter.
pub fn table(amount: u8) -> [u8; 256] {
    let s = f64::from(amount.min(*AMOUNT_RANGE.end())) / 100.0;
    let mut out = [0u8; 256];
    for (v, entry) in (0u8..=255).zip(out.iter_mut()) {
        let x = f64::from(v) / 255.0;
        let y = x + 1.7 * s * x * (1.0 - x).powi(3);
        // limited to 0..=255 first, so the conversion is exact
        *entry = (255.0 * y).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// `img` with every byte (each channel) put through [`table`]`(amount)`. `None` when the image's
/// data is not `width × height × 3` bytes.
pub fn lift(img: &Rgb, amount: u8) -> Option<Rgb> {
    if !img.is_consistent() {
        return None;
    }
    let table = table(amount);
    // the table has an entry for every byte, so the fallback is never taken
    let data = img
        .data
        .iter()
        .map(|&v| table.get(usize::from(v)).copied().unwrap_or(v))
        .collect();
    Some(Rgb {
        width: img.width,
        height: img.height,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_amount_is_the_identity() {
        let t = table(0);
        for (v, &y) in t.iter().enumerate() {
            assert_eq!(usize::from(y), v);
        }
    }

    #[test]
    fn the_curve_keeps_the_ends_never_lowers_is_monotonic_and_lifts_the_darkest_quarter() {
        for amount in [50, 100] {
            let t = table(amount);
            assert_eq!(t[0], 0, "{amount}");
            assert_eq!(t[255], 255, "{amount}");
            for v in 0..256 {
                assert!(usize::from(t[v]) >= v, "{amount} {v}");
                if v > 0 {
                    assert!(t[v] >= t[v - 1], "{amount} {v}");
                }
            }
            let lifts: Vec<usize> = (0..256).map(|v| usize::from(t[v]) - v).collect();
            let most = lifts.iter().copied().max().unwrap_or(0);
            assert!(most > 0, "{amount}");
            // every entry with the largest lift lies around x = ¼
            for (v, &lift) in lifts.iter().enumerate() {
                if lift == most {
                    assert!((48..=80).contains(&v), "{amount} {v}");
                }
            }
        }
    }

    #[test]
    fn an_amount_above_100_counts_as_100() {
        assert_eq!(table(101), table(100));
        assert_eq!(table(255), table(100));
    }

    #[test]
    fn lift_puts_each_channel_through_the_table() {
        let img = Rgb {
            width: 2,
            height: 1,
            data: vec![0, 40, 128, 200, 255, 10],
        };
        let t = table(70);
        let lifted = lift(&img, 70).unwrap();
        assert_eq!((lifted.width, lifted.height), (2, 1));
        let want: Vec<u8> = img.data.iter().map(|&v| t[usize::from(v)]).collect();
        assert_eq!(lifted.data, want);
        assert_ne!(lifted.data, img.data);
    }

    #[test]
    fn lift_refuses_data_of_the_wrong_length() {
        let img = Rgb {
            width: 2,
            height: 1,
            data: vec![0; 5],
        };
        assert_eq!(lift(&img, 50), None);
    }
}
