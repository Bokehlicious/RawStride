//! Channel view on encoded RGB values (ADR 0027 §3.6, W4).

use std::thread;

use crate::backend::Rgb;

const MAX_THREADS: usize = 8;
const MIN_ROWS_PER_BAND: usize = 32;

/// Which component of the view to show in gray, or full color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Channel {
    #[default]
    Color,
    Red,
    Green,
    Blue,
    Luminance,
}

impl Channel {
    pub fn next(self) -> Self {
        match self {
            Self::Color => Self::Red,
            Self::Red => Self::Green,
            Self::Green => Self::Blue,
            Self::Blue => Self::Luminance,
            Self::Luminance => Self::Color,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Color => Self::Luminance,
            Self::Red => Self::Color,
            Self::Green => Self::Red,
            Self::Blue => Self::Green,
            Self::Luminance => Self::Blue,
        }
    }
}

/// Copies a color view or shows the selected component in gray. Returns `None` for an
/// inconsistent image, and copies images without pixels.
pub fn apply(img: &Rgb, channel: Channel) -> Option<Rgb> {
    let width = usize::try_from(img.width).ok()?;
    let height = usize::try_from(img.height).ok()?;
    if width.checked_mul(height)?.checked_mul(3)? != img.data.len() {
        return None;
    }
    let mut out = img.clone();
    if channel == Channel::Color || out.data.is_empty() {
        return Some(out);
    }
    let threads = thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(MAX_THREADS)
        .min(height / MIN_ROWS_PER_BAND)
        .max(1);
    let band_pixels = height.div_ceil(threads) * width;
    if !apply_channel(&mut out, channel, band_pixels) {
        // A worker could not be started or joined. Retry from the original pixels.
        out.data.copy_from_slice(&img.data);
        apply_channel(&mut out, channel, width * height);
    }
    Some(out)
}

fn apply_channel(out: &mut Rgb, channel: Channel, band_pixels: usize) -> bool {
    let pixels = out.data.as_chunks_mut::<3>().0;
    match channel {
        Channel::Color => true,
        Channel::Red => gray_bands(pixels, band_pixels, |[r, _, _]| r),
        Channel::Green => gray_bands(pixels, band_pixels, |[_, g, _]| g),
        Channel::Blue => gray_bands(pixels, band_pixels, |[_, _, b]| b),
        Channel::Luminance => gray_bands(pixels, band_pixels, |[r, g, b]| {
            ((2126 * u32::from(r) + 7152 * u32::from(g) + 722 * u32::from(b) + 5000) / 10000) as u8
        }),
    }
}

fn gray_bands(
    pixels: &mut [[u8; 3]],
    band_pixels: usize,
    gray: impl Fn([u8; 3]) -> u8 + Sync,
) -> bool {
    if pixels.len() <= band_pixels {
        repeat_gray(pixels, &gray);
        return true;
    }
    thread::scope(|scope| {
        let mut bands = pixels.chunks_mut(band_pixels);
        let Some(first) = bands.next() else {
            return true;
        };
        let gray = &gray;
        let mut workers = Vec::with_capacity(bands.len());
        let mut complete = true;
        for band in bands {
            match thread::Builder::new().spawn_scoped(scope, move || repeat_gray(band, gray)) {
                Ok(worker) => workers.push(worker),
                Err(_) => {
                    complete = false;
                    break;
                }
            }
        }
        repeat_gray(first, gray);
        for worker in workers {
            complete &= worker.join().is_ok();
        }
        complete
    })
}

fn repeat_gray(pixels: &mut [[u8; 3]], gray: &impl Fn([u8; 3]) -> u8) {
    for pixel in pixels {
        *pixel = [gray(*pixel); 3];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANNELS: [Channel; 5] = [
        Channel::Color,
        Channel::Red,
        Channel::Green,
        Channel::Blue,
        Channel::Luminance,
    ];

    #[test]
    fn channel_cycle_wraps_in_both_directions() {
        assert_eq!(Channel::default(), Channel::Color);
        for (i, channel) in CHANNELS.into_iter().enumerate() {
            assert_eq!(channel.next(), CHANNELS[(i + 1) % CHANNELS.len()]);
            assert_eq!(channel.previous(), CHANNELS[(i + 4) % CHANNELS.len()]);
        }
    }

    #[test]
    fn color_copies_and_rgb_channels_repeat_the_selected_value() {
        let img = Rgb {
            width: 2,
            height: 1,
            data: vec![255, 0, 0, 10, 200, 30],
        };
        assert_eq!(apply(&img, Channel::Color), Some(img.clone()));
        for (channel, expected) in [
            (Channel::Red, [255, 255, 255, 10, 10, 10]),
            (Channel::Green, [0, 0, 0, 200, 200, 200]),
            (Channel::Blue, [0, 0, 0, 30, 30, 30]),
        ] {
            let out = apply(&img, channel).unwrap();
            assert_eq!((out.width, out.height), (2, 1));
            assert_eq!(out.data, expected);
        }
    }

    #[test]
    fn luminance_rounds_encoded_values_to_the_nearest_level() {
        let img = Rgb {
            width: 3,
            height: 2,
            data: vec![
                255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 10, 200, 30,
            ],
        };
        let out = apply(&img, Channel::Luminance).unwrap();
        assert_eq!((out.width, out.height), (3, 2));
        assert_eq!(
            out.data,
            [
                54, 54, 54, 182, 182, 182, 18, 18, 18, 255, 255, 255, 0, 0, 0, 147, 147, 147
            ]
        );
    }

    #[test]
    fn all_channels_refuse_malformed_images_and_copy_empty_ones() {
        let empty = Rgb {
            width: 0,
            height: 0,
            data: vec![],
        };
        let short = Rgb {
            width: 2,
            height: 1,
            data: vec![0; 5],
        };
        let oversized = Rgb {
            width: u32::MAX,
            height: u32::MAX,
            data: vec![],
        };
        for channel in CHANNELS {
            assert_eq!(apply(&short, channel), None);
            assert_eq!(apply(&oversized, channel), None);
            assert_eq!(apply(&empty, channel), Some(empty.clone()));
        }
    }

    #[test]
    fn channel_bands_match_a_single_pass_byte_for_byte() {
        let img = Rgb {
            width: 53,
            height: 107,
            data: (0..53 * 107 * 3).map(|i| (i % 251) as u8).collect(),
        };
        for channel in CHANNELS {
            let mut single = img.clone();
            let mut bands = img.clone();
            assert!(apply_channel(&mut single, channel, 53 * 107));
            assert!(apply_channel(&mut bands, channel, 53 * 27));
            assert_eq!(single, bands, "{channel:?}");
        }
    }

    #[test]
    #[ignore = "a time measurement; run with --release (ADR 0027 W4-Q7)"]
    fn timing_of_channel_view() {
        use std::hint::black_box;
        use std::time::Instant;

        for (width, height) in [(3840, 2160), (1920, 1200)] {
            let img = Rgb {
                width,
                height,
                data: (0..width * height * 3).map(|i| (i % 251) as u8).collect(),
            };
            for channel in [Channel::Red, Channel::Luminance] {
                let mut runs = [0.0; 11];
                for run in &mut runs {
                    let start = Instant::now();
                    let out = black_box(apply(black_box(&img), channel)).unwrap();
                    *run = start.elapsed().as_secs_f64() * 1000.0;
                    assert_eq!(out.data.len(), img.data.len());
                }
                runs.sort_by(f64::total_cmp);
                println!(
                    "{width}×{height} {channel:?}: median {:.3} ms, max {:.3} ms",
                    runs[5], runs[10]
                );
            }
        }
    }
}
