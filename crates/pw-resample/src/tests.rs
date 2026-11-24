use super::*;

/// A deterministic, non-flat test pattern: the sample at pixel (x, y), channel c.
fn pattern(x: u32, y: u32, c: u32) -> u32 {
    let mut v = x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(2_246_822_519) ^ c.wrapping_mul(97);
    v ^= v >> 13;
    v = v.wrapping_mul(1_274_126_177);
    v ^ (v >> 16)
}

fn image8(width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::new();
    for y in 0..height {
        for x in 0..width {
            for c in 0..3 {
                out.push((pattern(x, y, c) & 0xFF) as u8);
            }
        }
    }
    out
}

fn image16(width: u32, height: u32) -> Vec<u16> {
    let mut out = Vec::new();
    for y in 0..height {
        for x in 0..width {
            for c in 0..3 {
                out.push((pattern(x, y, c) & 0xFFFF) as u16);
            }
        }
    }
    out
}

/// The sample index of pixel (x, y), channel c.
fn at(width: u32, x: u32, y: u32, c: u32) -> usize {
    ((y * width + x) * 3 + c) as usize
}

fn mirror_left_right<T: Copy>(src: &[T], width: u32, height: u32) -> Vec<T> {
    let mut out = src.to_vec();
    for y in 0..height {
        for x in 0..width {
            for c in 0..3 {
                out[at(width, width - 1 - x, y, c)] = src[at(width, x, y, c)];
            }
        }
    }
    out
}

fn mirror_top_bottom<T: Copy>(src: &[T], width: u32, height: u32) -> Vec<T> {
    let mut out = src.to_vec();
    for y in 0..height {
        for x in 0..width {
            for c in 0..3 {
                out[at(width, x, height - 1 - y, c)] = src[at(width, x, y, c)];
            }
        }
    }
    out
}

fn assert_within<T: Copy + Into<i64> + std::fmt::Debug>(a: &[T], b: &[T], tolerance: i64) {
    assert_eq!(a.len(), b.len());
    for (index, (&x, &y)) in a.iter().zip(b).enumerate() {
        let difference = (x.into() - y.into()).abs();
        assert!(difference <= tolerance, "sample {index}: {x:?} and {y:?}");
    }
}

/// The inverse sRGB curve in `f64`, as a reference.
fn encode_reference(linear: f64, max: f64) -> f64 {
    let v = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    v * max
}

#[test]
fn refused_inputs_give_none_for_8_bit() {
    let src = vec![0_u8; 4 * 3 * 3];
    assert!(reduce_rgb8(&src, 0, 3, 1, 1).is_none());
    assert!(reduce_rgb8(&src, 4, 0, 1, 1).is_none());
    assert!(reduce_rgb8(&src, 4, 3, 0, 1).is_none());
    assert!(reduce_rgb8(&src, 4, 3, 1, 0).is_none());
    assert!(reduce_rgb8(&src, 4, 3, 5, 3).is_none());
    assert!(reduce_rgb8(&src, 4, 3, 4, 4).is_none());
    assert!(reduce_rgb8(&src[1..], 4, 3, 2, 2).is_none());
    let mut long = src.clone();
    long.push(0);
    assert!(reduce_rgb8(&long, 4, 3, 2, 2).is_none());
    assert!(reduce_rgb8(&src, u32::MAX, u32::MAX, 1, 1).is_none());
}

#[test]
fn refused_inputs_give_none_for_16_bit() {
    let src = vec![0_u16; 4 * 3 * 3];
    assert!(reduce_rgb16(&src, 0, 3, 1, 1).is_none());
    assert!(reduce_rgb16(&src, 4, 0, 1, 1).is_none());
    assert!(reduce_rgb16(&src, 4, 3, 0, 1).is_none());
    assert!(reduce_rgb16(&src, 4, 3, 1, 0).is_none());
    assert!(reduce_rgb16(&src, 4, 3, 5, 3).is_none());
    assert!(reduce_rgb16(&src, 4, 3, 4, 4).is_none());
    assert!(reduce_rgb16(&src[1..], 4, 3, 2, 2).is_none());
    let mut long = src.clone();
    long.push(0);
    assert!(reduce_rgb16(&long, 4, 3, 2, 2).is_none());
    assert!(reduce_rgb16(&src, u32::MAX, u32::MAX, 1, 1).is_none());
}

#[test]
fn the_same_size_gives_back_the_input() {
    let src8 = image8(7, 5);
    assert_eq!(reduce_rgb8(&src8, 7, 5, 7, 5), Some(src8));
    let src16 = image16(7, 5);
    assert_eq!(reduce_rgb16(&src16, 7, 5, 7, 5), Some(src16));
}

#[test]
fn a_flat_8_bit_image_keeps_every_value_exactly() {
    for value in 0..=255_u8 {
        let src = vec![value; 9 * 7 * 3];
        let out = reduce_rgb8(&src, 9, 7, 4, 3);
        assert_eq!(out, Some(vec![value; 4 * 3 * 3]), "value {value}");
    }
}

#[test]
fn a_flat_16_bit_image_keeps_its_value_within_one() {
    for value in [0_u16, 1, 1000, 32768, 65534, 65535] {
        let src = vec![value; 9 * 7 * 3];
        let out = reduce_rgb16(&src, 9, 7, 4, 3);
        assert_within(&out.unwrap_or_default(), &[value; 4 * 3 * 3], 1);
    }
}

#[test]
fn a_checkerboard_is_averaged_in_linear_light() {
    // Black and white in a checkerboard: the linear mean is 0.5, which encodes to 187.51 in
    // 8 bit; a mean of the encoded values would give 128.
    let black_white8 = [0, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 0];
    assert_eq!(
        reduce_rgb8(&black_white8, 2, 2, 1, 1),
        Some(vec![188, 188, 188])
    );

    let black_white16 = [0, 0, 0, 65535, 65535, 65535, 65535, 65535, 65535, 0, 0, 0];
    let expected = encode_reference(0.5, 65535.0).round() as u16;
    let out = reduce_rgb16(&black_white16, 2, 2, 1, 1);
    assert_within(&out.unwrap_or_default(), &[expected; 3], 2);
}

/// 12 × 1, columns 0–5 black and 6–11 white.
fn edge_image<T: Copy>(black: T, white: T) -> Vec<T> {
    (0..12)
        .flat_map(|x| [if x < 6 { black } else { white }; 3])
        .collect()
}

#[test]
fn the_overshoot_at_an_edge_is_clamped_not_wrapped() {
    // Reduced to 8 × 1 (f = 2/3), output column 5 has its centre at source 7.75 and column 2 at
    // 3.25. Lanczos' kernel has negative lobes at 1 < |x| < 2: for column 5 the black pixels 4
    // and 5 fall mostly under a negative lobe, so the white pixels' weights add up to more than
    // 1 and the value rises above white (ringing); for column 2 the white pixels 6 and 7 do, and
    // the value falls below black.
    let src8 = edge_image(0_u8, 255);
    let linear8 = reduce_linear_rgb8(&src8, 12, 1, 8, 1).unwrap_or_default();
    let src16 = edge_image(0_u16, 65535);
    let linear16 = reduce_linear_rgb16(&src16, 12, 1, 8, 1).unwrap_or_default();
    for linear in [&linear8, &linear16] {
        assert_eq!(linear.len(), 8 * 3);
        for c in 0..3 {
            assert!(
                linear[at(8, 5, 0, c)] > 1.0,
                "column 5: {}",
                linear[at(8, 5, 0, c)]
            );
            assert!(
                linear[at(8, 2, 0, c)] < 0.0,
                "column 2: {}",
                linear[at(8, 2, 0, c)]
            );
        }
    }

    let out8 = reduce_rgb8(&src8, 12, 1, 8, 1).unwrap_or_default();
    let out16 = reduce_rgb16(&src16, 12, 1, 8, 1).unwrap_or_default();
    assert_eq!(out8.len(), 8 * 3);
    assert_eq!(out16.len(), 8 * 3);
    for c in 0..3 {
        assert_eq!(out8[at(8, 5, 0, c)], 255);
        assert_eq!(out8[at(8, 2, 0, c)], 0);
        assert_eq!(out16[at(8, 5, 0, c)], 65535);
        assert_eq!(out16[at(8, 2, 0, c)], 0);
    }
}

#[test]
fn a_mirrored_input_gives_the_mirrored_output() {
    let (w, h, ow, oh) = (23, 17, 9, 6);
    let src8 = image8(w, h);
    let out8 = reduce_rgb8(&src8, w, h, ow, oh).unwrap_or_default();
    let lr8 = reduce_rgb8(&mirror_left_right(&src8, w, h), w, h, ow, oh).unwrap_or_default();
    assert_within(&lr8, &mirror_left_right(&out8, ow, oh), 1);
    let tb8 = reduce_rgb8(&mirror_top_bottom(&src8, w, h), w, h, ow, oh).unwrap_or_default();
    assert_within(&tb8, &mirror_top_bottom(&out8, ow, oh), 1);

    let src16 = image16(w, h);
    let out16 = reduce_rgb16(&src16, w, h, ow, oh).unwrap_or_default();
    let lr16 = reduce_rgb16(&mirror_left_right(&src16, w, h), w, h, ow, oh).unwrap_or_default();
    assert_within(&lr16, &mirror_left_right(&out16, ow, oh), 1);
    let tb16 = reduce_rgb16(&mirror_top_bottom(&src16, w, h), w, h, ow, oh).unwrap_or_default();
    assert_within(&tb16, &mirror_top_bottom(&out16, ow, oh), 1);
    assert_eq!(out8.len(), (ow * oh * 3) as usize);
    assert_eq!(out16.len(), (ow * oh * 3) as usize);
}

/// `src` with every channel but `channel` set to 0.
fn only_channel<T: Copy + Default>(src: &[T], channel: usize) -> Vec<T> {
    src.iter()
        .enumerate()
        .map(|(index, &v)| {
            if index % 3 == channel {
                v
            } else {
                T::default()
            }
        })
        .collect()
}

#[test]
fn the_channels_are_filtered_independently() {
    let (w, h, ow, oh) = (13, 11, 5, 4);
    let src8 = image8(w, h);
    let out8 = reduce_rgb8(&src8, w, h, ow, oh).unwrap_or_default();
    let src16 = image16(w, h);
    let out16 = reduce_rgb16(&src16, w, h, ow, oh).unwrap_or_default();
    assert_eq!(out8.len(), (ow * oh * 3) as usize);
    assert_eq!(out16.len(), (ow * oh * 3) as usize);
    for channel in 0..3 {
        let alone8 = reduce_rgb8(&only_channel(&src8, channel), w, h, ow, oh).unwrap_or_default();
        assert_eq!(only_channel(&out8, channel), alone8, "channel {channel}");
        let alone16 =
            reduce_rgb16(&only_channel(&src16, channel), w, h, ow, oh).unwrap_or_default();
        assert_eq!(only_channel(&out16, channel), alone16, "channel {channel}");
    }
}

#[test]
fn edges_and_small_sizes_work() {
    for (w, h, ow, oh) in [(7, 5, 3, 2), (1, 9, 1, 4), (9, 1, 4, 1), (5, 5, 1, 1)] {
        let count = (ow * oh * 3) as usize;
        let out8 = reduce_rgb8(&image8(w, h), w, h, ow, oh);
        assert_eq!(out8.map(|o| o.len()), Some(count), "{w}×{h} → {ow}×{oh}");
        let out16 = reduce_rgb16(&image16(w, h), w, h, ow, oh);
        assert_eq!(out16.map(|o| o.len()), Some(count), "{w}×{h} → {ow}×{oh}");

        let flat8 = vec![77_u8; (w * h * 3) as usize];
        assert_eq!(reduce_rgb8(&flat8, w, h, ow, oh), Some(vec![77; count]));
        let flat16 = vec![12345_u16; (w * h * 3) as usize];
        let out = reduce_rgb16(&flat16, w, h, ow, oh).unwrap_or_default();
        assert_within(&out, &vec![12345; count], 1);
    }
    assert_eq!(reduce_rgb8(&[1, 2, 3], 1, 1, 1, 1), Some(vec![1, 2, 3]));
    assert_eq!(reduce_rgb16(&[1, 2, 3], 1, 1, 1, 1), Some(vec![1, 2, 3]));
}

#[test]
fn the_8_and_16_bit_versions_agree() {
    for (w, h, ow, oh) in [(23, 17, 9, 6), (12, 1, 8, 1), (9, 7, 4, 3), (5, 5, 1, 1)] {
        let src8 = image8(w, h);
        let src16: Vec<u16> = src8.iter().map(|&v| u16::from(v) * 257).collect();
        let out8 = reduce_rgb8(&src8, w, h, ow, oh).unwrap_or_default();
        let out16 = reduce_rgb16(&src16, w, h, ow, oh).unwrap_or_default();
        let back: Vec<u8> = out16
            .iter()
            .map(|&v| (f64::from(v) / 257.0).round() as u8)
            .collect();
        assert_eq!(out8.len(), (ow * oh * 3) as usize);
        assert_within(&out8, &back, 1);
    }
}

/// `reduce_with_threads` for 8 bit, with the decode and encode of `reduce_rgb8`.
fn with_threads8(src: &[u8], (w, h, ow, oh): (u32, u32, u32, u32), threads: usize) -> Vec<u8> {
    let table: &[f32] = &DECODE_8;
    reduce_with_threads(
        src,
        Size::new(w, h, ow, oh),
        threads,
        |v| table[usize::from(v)],
        |l| encode(l, 255.0) as u8,
    )
    .unwrap_or_default()
}

/// `reduce_with_threads` for 16 bit, with the decode and encode of `reduce_rgb16`.
fn with_threads16(src: &[u16], (w, h, ow, oh): (u32, u32, u32, u32), threads: usize) -> Vec<u16> {
    let table: &[f32] = &DECODE_16;
    reduce_with_threads(
        src,
        Size::new(w, h, ow, oh),
        threads,
        |v| table[usize::from(v)],
        |l| encode(l, 65535.0) as u16,
    )
    .unwrap_or_default()
}

/// The sizes of the thread tests: few and many rows, fewer rows than threads, and a factor of 1
/// in each direction.
const THREAD_SIZES: [(u32, u32, u32, u32); 6] = [
    (23, 17, 9, 6),
    (4784, 40, 3241, 27),
    (97, 300, 40, 200),
    (9, 7, 4, 3),
    (50, 40, 20, 40),
    (40, 50, 40, 20),
];

#[test]
fn any_number_of_threads_gives_the_same_8_bit_result() {
    for size in THREAD_SIZES {
        let (w, h, ow, oh) = size;
        let src = image8(w, h);
        let one = with_threads8(&src, size, 1);
        assert_eq!(one.len(), (ow * oh * 3) as usize, "{size:?}");
        for threads in [2, 3, 7, 8] {
            assert_eq!(
                with_threads8(&src, size, threads),
                one,
                "{size:?}, {threads} threads"
            );
        }
    }
}

#[test]
fn any_number_of_threads_gives_the_same_16_bit_result() {
    for size in THREAD_SIZES {
        let (w, h, ow, oh) = size;
        let src = image16(w, h);
        let one = with_threads16(&src, size, 1);
        assert_eq!(one.len(), (ow * oh * 3) as usize, "{size:?}");
        for threads in [2, 3, 7, 8] {
            assert_eq!(
                with_threads16(&src, size, threads),
                one,
                "{size:?}, {threads} threads"
            );
        }
    }
}

#[test]
fn the_public_functions_equal_one_thread() {
    let size = (97, 300, 40, 200);
    let (w, h, ow, oh) = size;
    let src8 = image8(w, h);
    let one8 = with_threads8(&src8, size, 1);
    assert_eq!(one8.len(), (ow * oh * 3) as usize);
    assert_eq!(reduce_rgb8(&src8, w, h, ow, oh), Some(one8));
    let src16 = image16(w, h);
    let one16 = with_threads16(&src16, size, 1);
    assert_eq!(one16.len(), (ow * oh * 3) as usize);
    assert_eq!(reduce_rgb16(&src16, w, h, ow, oh), Some(one16));
}

#[test]
fn the_number_of_threads_follows_the_cores_and_the_rows() {
    for (cores, out_h, expected) in [
        (8, 1, 1),
        (8, 63, 1),
        (8, 64, 2),
        (8, 10_000, 8),
        (16, 10_000, 8),
        (1, 10_000, 1),
        (0, 10_000, 1),
    ] {
        assert_eq!(
            thread_count(cores, out_h),
            expected,
            "{cores} cores, {out_h} rows"
        );
    }
}

#[test]
#[ignore = "a time measurement; run with --release (ADR 0027 W1-Q2)"]
fn timing_of_a_61_mpix_half_size_render_reduced_to_fit() {
    let (w, h, ow, oh) = (4784, 3188, 3241, 2160);
    let src = image8(w, h);
    let started = std::time::Instant::now();
    let out = reduce_rgb8(&src, w, h, ow, oh);
    let elapsed = started.elapsed();
    println!(
        "{w}×{h} → {ow}×{oh} (8 bit): {:.1} ms on {} threads",
        elapsed.as_secs_f64() * 1000.0,
        thread_count(available_cores(), oh)
    );
    assert_eq!(out.map(|o| o.len()), Some((ow * oh * 3) as usize));
}
