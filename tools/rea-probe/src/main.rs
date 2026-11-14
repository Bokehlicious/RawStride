//! `rea-probe` – a small binary with *known* source code.
//!
//! It exists only so we can check that REA/Ghidra reports what we expect
//! (functions, strings, imports, call graph) before relying on it for real
//! RawStride binaries. It is not part of the application.
//!
//! Build: `cargo build -p rea-probe --profile profiling`
//! Output: `target\profiling\rea-probe.exe` (+ `.pdb`)

use std::hint::black_box;

/// Marker string that must show up in REA's string inventory.
const MARKER: &str = "PW_REA_PROBE_MARKER_v1";

const WIDTH: usize = 64;
const HEIGHT: usize = 48;

/// FNV-1a 64-bit hash – a tight loop with a known constant (0x100000001b3).
#[inline(never)]
fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Deterministic synthetic 8-bit grey image.
#[inline(never)]
fn make_test_image() -> Vec<u8> {
    let mut img = vec![0u8; WIDTH * HEIGHT];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            img[y * WIDTH + x] = ((x * 7 + y * 13) % 256) as u8;
        }
    }
    img
}

/// 3x3 box blur – the kind of per-pixel kernel RawStride will profile later.
#[inline(never)]
fn box_blur_3x3(src: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut dst = vec![0u8; src.len()];
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let mut sum: u32 = 0;
            for dy in 0..3 {
                for dx in 0..3 {
                    sum += u32::from(src[(y + dy - 1) * width + (x + dx - 1)]);
                }
            }
            dst[y * width + x] = (sum / 9) as u8;
        }
    }
    dst
}

fn main() {
    let img = make_test_image();
    let blurred = box_blur_3x3(black_box(&img), WIDTH, HEIGHT);
    let hash = fnv1a64(black_box(&blurred));
    println!("{MARKER} {WIDTH}x{HEIGHT} blur-hash={hash:016x}");
}
