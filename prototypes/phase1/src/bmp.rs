//! Minimal 24-bit BMP writer/reader (no dependency), plus selection of high-detail crops for the
//! blind side-by-side review. Images are top-down RGB in memory; BMP rows are stored bottom-up BGR.

use std::path::Path;

pub struct BmpImage {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
}

/// Rotates by LibRaw `sizes.flip` (3 = 180°, 5 = 90° CCW, 6 = 90° CW; anything else unchanged).
fn orient(w: usize, h: usize, rgb: &[u8], flip: i32) -> (usize, usize, Vec<u8>) {
    let px = |x: usize, y: usize| &rgb[(y * w + x) * 3..(y * w + x) * 3 + 3];
    match flip {
        3 => {
            let mut o = Vec::with_capacity(rgb.len());
            for y in (0..h).rev() {
                for x in (0..w).rev() {
                    o.extend_from_slice(px(x, y));
                }
            }
            (w, h, o)
        }
        5 | 6 => {
            // output is h wide, w tall
            let mut o = Vec::with_capacity(rgb.len());
            for oy in 0..w {
                for ox in 0..h {
                    let (x, y) = if flip == 6 { (oy, h - 1 - ox) } else { (w - 1 - oy, ox) };
                    o.extend_from_slice(px(x, y));
                }
            }
            (h, w, o)
        }
        _ => (w, h, rgb.to_vec()),
    }
}

fn encode(w: usize, h: usize, rgb: &[u8]) -> Vec<u8> {
    let row = (w * 3 + 3) & !3;
    let size = 54 + row * h;
    let mut b = Vec::with_capacity(size);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&(size as u32).to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(w as i32).to_le_bytes());
    b.extend_from_slice(&(h as i32).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&((row * h) as u32).to_le_bytes());
    b.extend_from_slice(&2835i32.to_le_bytes());
    b.extend_from_slice(&2835i32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    for y in (0..h).rev() {
        for x in 0..w {
            let i = (y * w + x) * 3;
            b.extend_from_slice(&[rgb[i + 2], rgb[i + 1], rgb[i]]);
        }
        b.resize(b.len() + (row - w * 3), 0);
    }
    b
}

fn save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn write(path: &Path, w: usize, h: usize, rgb: &[u8], flip: i32) -> Result<(), String> {
    if rgb.len() < w * h * 3 {
        return Err(format!("{}: buffer too small", path.display()));
    }
    let (ow, oh, o) = orient(w, h, &rgb[..w * h * 3], flip);
    save(path, &encode(ow, oh, &o))
}

pub fn read(path: &Path) -> Result<BmpImage, String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    if b.len() < 54 || &b[0..2] != b"BM" || u16::from_le_bytes([b[28], b[29]]) != 24 || u32_at(30) != 0 {
        return Err(format!("{}: not an uncompressed 24-bit BMP", path.display()));
    }
    let off = u32_at(10) as usize;
    let w = i32::from_le_bytes(b[18..22].try_into().unwrap());
    let hs = i32::from_le_bytes(b[22..26].try_into().unwrap());
    if w <= 0 || hs == 0 {
        return Err(format!("{}: bad size", path.display()));
    }
    let (w, h, bottom_up) = (w as usize, hs.unsigned_abs() as usize, hs > 0);
    let row = (w * 3 + 3) & !3;
    if b.len() < off + row * h {
        return Err(format!("{}: truncated", path.display()));
    }
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        let src = off + if bottom_up { (h - 1 - y) * row } else { y * row };
        for x in 0..w {
            let s = src + x * 3;
            let d = (y * w + x) * 3;
            rgb[d] = b[s + 2];
            rgb[d + 1] = b[s + 1];
            rgb[d + 2] = b[s];
        }
    }
    Ok(BmpImage { w, h, rgb })
}

/// Up to `n` non-overlapping `cw`×`ch` crops with the highest luma gradient energy, greedily,
/// searched on an 8-px grid. Returns `[x, y, w, h]`.
pub fn detail_crops(img: &BmpImage, cw: usize, ch: usize, n: usize) -> Vec<[usize; 4]> {
    let (w, h) = (img.w, img.h);
    if w < cw || h < ch {
        return vec![[0, 0, w.min(cw), h.min(ch)]];
    }
    let luma: Vec<f64> = img
        .rgb
        .chunks_exact(3)
        .map(|p| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64)
        .collect();
    // integral image of |gx| + |gy|
    let mut ii = vec![0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut acc = 0.0;
        for x in 0..w {
            let l = luma[y * w + x];
            let gx = if x + 1 < w { (luma[y * w + x + 1] - l).abs() } else { 0.0 };
            let gy = if y + 1 < h { (luma[(y + 1) * w + x] - l).abs() } else { 0.0 };
            acc += gx + gy;
            ii[(y + 1) * (w + 1) + x + 1] = ii[y * (w + 1) + x + 1] + acc;
        }
    }
    let sum = |x: usize, y: usize| {
        let (x1, y1) = (x + cw, y + ch);
        ii[y1 * (w + 1) + x1] - ii[y * (w + 1) + x1] - ii[y1 * (w + 1) + x] + ii[y * (w + 1) + x]
    };
    let mut cands = Vec::new();
    for y in (0..=h - ch).step_by(8) {
        for x in (0..=w - cw).step_by(8) {
            cands.push((sum(x, y), x, y));
        }
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out: Vec<[usize; 4]> = Vec::new();
    for (_, x, y) in cands {
        if out.len() >= n {
            break;
        }
        let overlaps = out.iter().any(|c| x < c[0] + cw && c[0] < x + cw && y < c[1] + ch && c[1] < y + ch);
        if !overlaps {
            out.push([x, y, cw, ch]);
        }
    }
    out
}

pub fn write_crop(path: &Path, img: &BmpImage, c: [usize; 4]) -> Result<(), String> {
    let [x, y, cw, ch] = c;
    if x + cw > img.w || y + ch > img.h {
        return Err(format!("{}: crop outside image", path.display()));
    }
    let mut o = Vec::with_capacity(cw * ch * 3);
    for yy in y..y + ch {
        let s = (yy * img.w + x) * 3;
        o.extend_from_slice(&img.rgb[s..s + cw * 3]);
    }
    save(path, &encode(cw, ch, &o))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("pwproto-bmp-{}-{name}", std::process::id()))
    }

    fn sample(w: usize, h: usize) -> Vec<u8> {
        (0..w * h).flat_map(|i| [(i % 251) as u8, (i / 7 % 253) as u8, (i * 3 % 255) as u8]).collect()
    }

    #[test]
    fn roundtrip_odd_width() {
        let (w, h) = (5, 3);
        let rgb = sample(w, h);
        let p = tmp("rt.bmp");
        write(&p, w, h, &rgb, 0).unwrap();
        let r = read(&p).unwrap();
        assert_eq!((r.w, r.h), (w, h));
        assert_eq!(r.rgb, rgb);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn rotations() {
        // 2×1 image: [A, B]
        let rgb = vec![1, 1, 1, 2, 2, 2];
        let (w, h, o) = orient(2, 1, &rgb, 6); // 90° CW: A on top
        assert_eq!((w, h), (1, 2));
        assert_eq!(o, vec![1, 1, 1, 2, 2, 2]);
        let (_, _, o) = orient(2, 1, &rgb, 5); // 90° CCW: B on top
        assert_eq!(o, vec![2, 2, 2, 1, 1, 1]);
        let (_, _, o) = orient(2, 1, &rgb, 3);
        assert_eq!(o, vec![2, 2, 2, 1, 1, 1]);
        // 3×2 CW: top-left of output is bottom-left of input
        let rgb: Vec<u8> = (0..6u8).flat_map(|i| [i, i, i]).collect();
        let (w, h, o) = orient(3, 2, &rgb, 6);
        assert_eq!((w, h), (2, 3));
        let first: Vec<u8> = o.chunks(3).map(|p| p[0]).collect();
        assert_eq!(first, vec![3, 0, 4, 1, 5, 2]);
    }

    #[test]
    fn crops_pick_detail_and_do_not_overlap() {
        let (w, h) = (200, 100);
        let mut rgb = vec![128u8; w * h * 3];
        // checkerboard detail in x 120..180, y 20..80
        for y in 20..80 {
            for x in 120..180 {
                let v = if (x + y) % 2 == 0 { 0 } else { 255 };
                rgb[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&[v, v, v]);
            }
        }
        let img = BmpImage { w, h, rgb };
        let c = detail_crops(&img, 40, 40, 2);
        assert_eq!(c.len(), 2);
        assert!(c[0][0] >= 112 && c[0][0] <= 144 && c[0][1] >= 16 && c[0][1] <= 44, "{:?}", c[0]);
        let (a, b) = (c[0], c[1]);
        assert!(!(a[0] < b[0] + 40 && b[0] < a[0] + 40 && a[1] < b[1] + 40 && b[1] < a[1] + 40));
        let p = tmp("crop.bmp");
        write_crop(&p, &img, c[0]).unwrap();
        let r = read(&p).unwrap();
        assert_eq!((r.w, r.h), (40, 40));
        let _ = std::fs::remove_file(p);
    }
}
