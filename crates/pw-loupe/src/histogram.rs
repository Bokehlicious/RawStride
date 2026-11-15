//! The Histogram section of the preview's right column and of the inspector (ADR 0017 §2.6, U7,
//! T142): what the panel draws, made from the view core's statistics as plain data. The drawing
//! code in `main.rs` only paints the bars and the texts it is given; the texts themselves are
//! made in `preview_text.rs` (rule 13).

use pw_viewcore::{ImageStatistics, Statistics, StatisticsOrigin};

use crate::preview_text::{HistogramText, clipped_text, deep_shadows_text};

/// The bins of one channel.
pub const HISTOGRAM_BINS: usize = 256;

/// The bar heights of the three channels (R, G, B), each from 0 (empty) to 1 (the panel's full
/// height), one per bin.
pub type HistogramBars = [[f32; HISTOGRAM_BINS]; 3];

/// What the Histogram section shows.
#[derive(Clone, Debug, PartialEq)]
pub enum HistogramPanel {
    /// The statistics have not come yet.
    Reading,
    /// The file has none.
    NoHistogram,
    /// The histogram with the lines under it.
    Ready(Box<HistogramView>),
}

/// A histogram and the lines under it.
#[derive(Clone, Debug, PartialEq)]
pub struct HistogramView {
    pub bars: HistogramBars,
    /// Where the numbers come from (G1): RAW data, Embedded JPEG or TIFF.
    pub source: HistogramText,
    /// "Clipped: R 0.12 %  G 3.40 %  B 0.00 %".
    pub clipped: String,
    /// "Deep shadows: 1.25 %".
    pub deep_shadows: String,
}

/// The panel for the statistics of the shown image: `None` while they are not ready.
pub fn histogram_panel(statistics: Option<&ImageStatistics>) -> HistogramPanel {
    match statistics {
        None => HistogramPanel::Reading,
        Some(ImageStatistics::Unavailable) => HistogramPanel::NoHistogram,
        Some(ImageStatistics::Available(statistics)) => {
            HistogramPanel::Ready(Box::new(histogram_view(statistics)))
        }
    }
}

/// The histogram and the lines under it for `statistics`.
pub fn histogram_view(statistics: &Statistics) -> HistogramView {
    let [r, g, b] = statistics.clipped;
    HistogramView {
        bars: histogram_bars(&statistics.histogram),
        source: histogram_source_text(statistics.origin),
        clipped: clipped_text(r, g, b),
        deep_shadows: deep_shadows_text(statistics.deep_shadows),
    }
}

/// The text that names where the statistics come from (G1).
pub fn histogram_source_text(origin: StatisticsOrigin) -> HistogramText {
    match origin {
        StatisticsOrigin::RawData => HistogramText::RawData,
        StatisticsOrigin::EmbeddedJpeg => HistogramText::EmbeddedJpeg,
        StatisticsOrigin::Tiff => HistogramText::Tiff,
    }
}

/// The bar heights of a histogram (U7-Q7). Each bin is taken as its share of its own channel's
/// pixels, so the green channel of a Bayer sensor (twice the photosites) is not drawn twice as
/// high. The heights are scaled to the highest bin of the three channels with **the two end bins
/// left out of the scale**, so a clipped image (or one with crushed shadows) does not flatten the
/// rest; an end bin higher than that is drawn at the full height. A histogram with pixels only in
/// its end bins is scaled to those.
pub fn histogram_bars(histogram: &[[u64; HISTOGRAM_BINS]; 3]) -> HistogramBars {
    let mut shares = [[0.0f64; HISTOGRAM_BINS]; 3];
    for (channel, shares) in histogram.iter().zip(shares.iter_mut()) {
        let pixels: u64 = channel.iter().sum();
        if pixels == 0 {
            continue;
        }
        for (count, share) in channel.iter().zip(shares.iter_mut()) {
            *share = *count as f64 / pixels as f64;
        }
    }
    let highest = |bins: &[f64]| bins.iter().copied().fold(0.0f64, f64::max);
    let inner = shares
        .iter()
        .map(|channel| highest(channel.get(1..HISTOGRAM_BINS - 1).unwrap_or(&[])))
        .fold(0.0f64, f64::max);
    let scale = if inner > 0.0 {
        inner
    } else {
        shares
            .iter()
            .map(|channel| highest(channel))
            .fold(0.0f64, f64::max)
    };
    let mut bars = [[0.0f32; HISTOGRAM_BINS]; 3];
    if scale <= 0.0 {
        return bars;
    }
    for (shares, bars) in shares.iter().zip(bars.iter_mut()) {
        for (share, bar) in shares.iter().zip(bars.iter_mut()) {
            *bar = (share / scale).min(1.0) as f32;
        }
    }
    bars
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn empty() -> [[u64; HISTOGRAM_BINS]; 3] {
        [[0; HISTOGRAM_BINS]; 3]
    }

    fn statistics(origin: StatisticsOrigin) -> Statistics {
        let mut histogram = empty();
        histogram[0][10] = 50;
        histogram[0][20] = 50;
        histogram[1][10] = 100;
        histogram[2][255] = 100;
        Statistics {
            origin,
            histogram,
            clipped: [0.0012, 0.034, 0.0],
            deep_shadows: 0.0125,
        }
    }

    #[test]
    fn the_bars_are_scaled_to_the_highest_bin_without_the_two_end_bins() {
        let mut histogram = empty();
        // red: a huge clipped end bin and two inner bins
        histogram[0][255] = 800;
        histogram[0][100] = 150;
        histogram[0][101] = 50;
        // green: crushed shadows in the first bin and one inner bin
        histogram[1][0] = 900;
        histogram[1][50] = 100;
        // blue: inner bins only
        histogram[2][30] = 20;
        histogram[2][31] = 80;
        let bars = histogram_bars(&histogram);
        // the highest inner share is blue's 80 % (bin 31): the scale
        assert_eq!(bars[2][31], 1.0);
        assert_eq!(bars[2][30], 0.25);
        // red's inner bins are 15 % and 5 % of its pixels
        assert!((bars[0][100] - 0.1875).abs() < 1e-6, "{}", bars[0][100]);
        assert!((bars[0][101] - 0.0625).abs() < 1e-6, "{}", bars[0][101]);
        // the end bins do not set the scale; higher than it, they are drawn at the full height
        assert_eq!(bars[0][255], 1.0);
        assert_eq!(bars[1][0], 1.0);
        assert!((bars[1][50] - 0.125).abs() < 1e-6, "{}", bars[1][50]);
        // every other bin is empty
        assert_eq!(bars[0][0], 0.0);
        assert_eq!(bars[2][255], 0.0);
        assert!(bars.iter().flatten().all(|h| (0.0..=1.0).contains(h)));
    }

    #[test]
    fn an_end_bin_lower_than_the_scale_keeps_its_own_height() {
        let mut histogram = empty();
        histogram[0][0] = 10;
        histogram[0][128] = 40;
        histogram[0][255] = 50;
        let bars = histogram_bars(&histogram);
        assert_eq!(bars[0][128], 1.0);
        assert_eq!(bars[0][0], 0.25);
        // higher than the scale: the full height, not more
        assert_eq!(bars[0][255], 1.0);
    }

    #[test]
    fn each_channel_is_taken_as_a_share_of_its_own_pixels() {
        // a Bayer sensor: twice as many green photosites, the same distribution
        let mut histogram = empty();
        histogram[0][40] = 100;
        histogram[1][40] = 200;
        histogram[2][40] = 100;
        let bars = histogram_bars(&histogram);
        assert_eq!([bars[0][40], bars[1][40], bars[2][40]], [1.0, 1.0, 1.0]);
    }

    #[test]
    fn a_histogram_with_pixels_only_in_its_end_bins_is_scaled_to_them() {
        let mut histogram = empty();
        histogram[0][255] = 75;
        histogram[0][0] = 25;
        histogram[1][0] = 10;
        let bars = histogram_bars(&histogram);
        // green has all its pixels in bin 0: the highest share
        assert_eq!(bars[1][0], 1.0);
        assert_eq!(bars[0][255], 0.75);
        assert_eq!(bars[0][0], 0.25);
    }

    #[test]
    fn an_empty_histogram_has_no_bars() {
        let bars = histogram_bars(&empty());
        assert!(bars.iter().flatten().all(|h| *h == 0.0));
    }

    #[test]
    fn the_source_text_names_where_the_numbers_come_from() {
        assert_eq!(
            histogram_source_text(StatisticsOrigin::RawData),
            HistogramText::RawData
        );
        assert_eq!(
            histogram_source_text(StatisticsOrigin::EmbeddedJpeg),
            HistogramText::EmbeddedJpeg
        );
        assert_eq!(
            histogram_source_text(StatisticsOrigin::Tiff),
            HistogramText::Tiff
        );
    }

    #[test]
    fn the_view_has_the_bars_the_source_and_the_share_texts() {
        let view = histogram_view(&statistics(StatisticsOrigin::RawData));
        assert_eq!(view.source, HistogramText::RawData);
        assert_eq!(view.clipped, "Clipped: R 0.12 %  G 3.40 %  B 0.00 %");
        assert_eq!(view.deep_shadows, "Deep shadows: 1.25 %");
        // red has half its pixels in each of two bins, green all in one: the scale
        assert_eq!(view.bars[1][10], 1.0);
        assert_eq!(view.bars[0][10], 0.5);
        assert_eq!(view.bars[0][20], 0.5);
        // blue is only in the top bin, drawn at the full height
        assert_eq!(view.bars[2][255], 1.0);

        let view = histogram_view(&statistics(StatisticsOrigin::EmbeddedJpeg));
        assert_eq!(view.source, HistogramText::EmbeddedJpeg);
        let view = histogram_view(&statistics(StatisticsOrigin::Tiff));
        assert_eq!(view.source, HistogramText::Tiff);
    }

    #[test]
    fn the_panel_reads_until_the_statistics_come_and_says_when_there_are_none() {
        assert_eq!(histogram_panel(None), HistogramPanel::Reading);
        assert_eq!(
            histogram_panel(Some(&ImageStatistics::Unavailable)),
            HistogramPanel::NoHistogram
        );
        let statistics = statistics(StatisticsOrigin::Tiff);
        let available = ImageStatistics::Available(Arc::new(statistics.clone()));
        assert_eq!(
            histogram_panel(Some(&available)),
            HistogramPanel::Ready(Box::new(histogram_view(&statistics)))
        );
    }
}
