//! Image-quality metrics for P4b/P6 (ADR 0008 §8.3). Pure Rust, no dependencies, so they can be
//! unit-tested anywhere. All functions take 8-bit sRGB, RGB-interleaved images of equal size and
//! compare a test image against a reference (AHD).
//!
//! What is measured, and how (approximations are named as such in the report):
//! - colour: CIEDE2000 (ΔE00) per pixel in CIELAB (D65), mean and 99th percentile;
//! - structure: SSIM (Gaussian 11×11, σ = 1.5) and MS-SSIM (5 scales, standard weights) on luma;
//! - PSNR on luma (supplement only);
//! - sharpness, directly: (a) the 10–90 % rise distance across strong straight edges (pixels);
//!   (b) high-frequency energy ratio (variance of the Laplacian on detail areas, test / reference);
//! - artefacts: false colour (mean chroma C*ab at edges that are neutral in the reference) and a
//!   zipper index (second difference along the edge tangent, test / reference).

#[derive(Debug, Clone, Copy)]
pub struct Img<'a> {
    pub w: usize,
    pub h: usize,
    pub rgb: &'a [u8],
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub de00_mean: f64,
    pub de00_p99: f64,
    pub ssim: f64,
    pub ms_ssim: f64,
    pub psnr_y: f64,
    pub rise_ref_px: f64,
    pub rise_test_px: f64,
    pub hf_ratio: f64,
    pub false_colour_ref: f64,
    pub false_colour_test: f64,
    pub zipper_ratio: f64,
    pub edge_points: usize,
}

// ---------- colour ----------

fn srgb_to_linear(c: u8) -> f64 {
    let v = f64::from(c) / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// sRGB (8-bit) → CIELAB, D65 white.
pub fn lab(r: u8, g: u8, b: u8) -> [f64; 3] {
    let (r, g, b) = (srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b));
    let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;
    let (xn, yn, zn) = (0.95047, 1.0, 1.08883);
    let f = |t: f64| if t > 216.0 / 24389.0 { t.cbrt() } else { (24389.0 / 27.0 * t + 16.0) / 116.0 };
    let (fx, fy, fz) = (f(x / xn), f(y / yn), f(z / zn));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 colour difference (kL = kC = kH = 1).
pub fn de2000(l1: [f64; 3], l2: [f64; 3]) -> f64 {
    use std::f64::consts::PI;
    let [l1_, a1, b1] = l1;
    let [l2_, a2, b2] = l2;
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let cm = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cm.powi(7) / (cm.powi(7) + 25f64.powi(7))).sqrt());
    let a1p = (1.0 + g) * a1;
    let a2p = (1.0 + g) * a2;
    let c1p = (a1p * a1p + b1 * b1).sqrt();
    let c2p = (a2p * a2p + b2 * b2).sqrt();
    let hp = |b: f64, a: f64| {
        if a == 0.0 && b == 0.0 { 0.0 } else {
            let h = b.atan2(a).to_degrees();
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let h1p = hp(b1, a1p);
    let h2p = hp(b2, a2p);
    let dlp = l2_ - l1_;
    let dcp = c2p - c1p;
    let dhp = if c1p * c2p == 0.0 {
        0.0
    } else {
        let d = h2p - h1p;
        if d.abs() <= 180.0 { d } else if d > 180.0 { d - 360.0 } else { d + 360.0 }
    };
    let dhp_big = 2.0 * (c1p * c2p).sqrt() * (dhp.to_radians() / 2.0).sin();
    let lpm = (l1_ + l2_) / 2.0;
    let cpm = (c1p + c2p) / 2.0;
    let hpm = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hpm - 30.0).to_radians().cos() + 0.24 * (2.0 * hpm).to_radians().cos()
        + 0.32 * (3.0 * hpm + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hpm - 63.0).to_radians().cos();
    let dtheta = 30.0 * (-((hpm - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cpm.powi(7) / (cpm.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lpm - 50.0).powi(2) / (20.0 + (lpm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cpm;
    let sh = 1.0 + 0.015 * cpm * t;
    let rt = -(2.0 * dtheta * PI / 180.0).sin() * rc;
    ((dlp / sl).powi(2) + (dcp / sc).powi(2) + (dhp_big / sh).powi(2)
        + rt * (dcp / sc) * (dhp_big / sh))
        .sqrt()
}

fn luma(img: &Img) -> Vec<f64> {
    img.rgb
        .chunks_exact(3)
        .map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]))
        .collect()
}

fn percentile(mut v: Vec<f64>, p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((v.len() - 1) as f64 * p).round() as usize;
    v[i]
}

// ---------- SSIM / MS-SSIM ----------

fn gaussian_kernel() -> [f64; 11] {
    let mut k = [0.0; 11];
    let s = 1.5f64;
    let mut sum = 0.0;
    for (i, v) in k.iter_mut().enumerate() {
        let x = i as f64 - 5.0;
        *v = (-(x * x) / (2.0 * s * s)).exp();
        sum += *v;
    }
    for v in &mut k {
        *v /= sum;
    }
    k
}

/// Separable Gaussian blur, "valid" region only (output (w-10)×(h-10)).
fn blur_valid(a: &[f64], w: usize, h: usize) -> (Vec<f64>, usize, usize) {
    let k = gaussian_kernel();
    let w1 = w - 10;
    let mut tmp = vec![0.0; w1 * h];
    for y in 0..h {
        for x in 0..w1 {
            let mut s = 0.0;
            for (i, kv) in k.iter().enumerate() {
                s += kv * a[y * w + x + i];
            }
            tmp[y * w1 + x] = s;
        }
    }
    let h1 = h - 10;
    let mut out = vec![0.0; w1 * h1];
    for y in 0..h1 {
        for x in 0..w1 {
            let mut s = 0.0;
            for (i, kv) in k.iter().enumerate() {
                s += kv * tmp[(y + i) * w1 + x];
            }
            out[y * w1 + x] = s;
        }
    }
    (out, w1, h1)
}

/// Returns (mean SSIM, mean contrast-structure term) for one scale.
fn ssim_terms(x: &[f64], y: &[f64], w: usize, h: usize) -> (f64, f64) {
    let c1 = (0.01f64 * 255.0).powi(2);
    let c2 = (0.03f64 * 255.0).powi(2);
    let xx: Vec<f64> = x.iter().map(|v| v * v).collect();
    let yy: Vec<f64> = y.iter().map(|v| v * v).collect();
    let xy: Vec<f64> = x.iter().zip(y).map(|(a, b)| a * b).collect();
    let (mx, w1, h1) = blur_valid(x, w, h);
    let (my, ..) = blur_valid(y, w, h);
    let (sxx, ..) = blur_valid(&xx, w, h);
    let (syy, ..) = blur_valid(&yy, w, h);
    let (sxy, ..) = blur_valid(&xy, w, h);
    let n = (w1 * h1) as f64;
    let (mut ssim, mut cs) = (0.0, 0.0);
    for i in 0..w1 * h1 {
        let (ux, uy) = (mx[i], my[i]);
        let vx = sxx[i] - ux * ux;
        let vy = syy[i] - uy * uy;
        let cov = sxy[i] - ux * uy;
        let l = (2.0 * ux * uy + c1) / (ux * ux + uy * uy + c1);
        let c = (2.0 * cov + c2) / (vx + vy + c2);
        ssim += l * c;
        cs += c;
    }
    (ssim / n, cs / n)
}

fn downsample2(a: &[f64], w: usize, h: usize) -> (Vec<f64>, usize, usize) {
    let (w2, h2) = (w / 2, h / 2);
    let mut o = vec![0.0; w2 * h2];
    for y in 0..h2 {
        for x in 0..w2 {
            o[y * w2 + x] = (a[2 * y * w + 2 * x]
                + a[2 * y * w + 2 * x + 1]
                + a[(2 * y + 1) * w + 2 * x]
                + a[(2 * y + 1) * w + 2 * x + 1])
                / 4.0;
        }
    }
    (o, w2, h2)
}

pub fn ssim_ms(r: &[f64], t: &[f64], w: usize, h: usize) -> (f64, f64) {
    const WEIGHTS: [f64; 5] = [0.0448, 0.2856, 0.3001, 0.2363, 0.1333];
    let (ssim0, _) = ssim_terms(r, t, w, h);
    let (mut a, mut b, mut cw, mut ch) = (r.to_vec(), t.to_vec(), w, h);
    let mut ms = 1.0;
    for (s, wgt) in WEIGHTS.iter().enumerate() {
        if cw < 22 || ch < 22 {
            break;
        }
        let (ss, cs) = ssim_terms(&a, &b, cw, ch);
        let term = if s == WEIGHTS.len() - 1 { ss } else { cs };
        ms *= term.max(1e-6).powf(*wgt);
        let (a2, w2, h2) = downsample2(&a, cw, ch);
        let (b2, ..) = downsample2(&b, cw, ch);
        a = a2;
        b = b2;
        cw = w2;
        ch = h2;
    }
    (ssim0, ms)
}

// ---------- sharpness and artefacts ----------

struct EdgePoint {
    x: usize,
    y: usize,
    horizontal_profile: bool, // true: profile along x (vertical edge)
}

fn grad(l: &[f64], w: usize, x: usize, y: usize) -> (f64, f64) {
    let gx = (l[y * w + x + 1] - l[y * w + x - 1]) / 2.0;
    let gy = (l[(y + 1) * w + x] - l[(y - 1) * w + x]) / 2.0;
    (gx, gy)
}

/// Strong, straight, axis-aligned edges in the reference (top 2 % gradient, direction within
/// ~20° of an axis), at least 8 px from the border, subsampled to at most ~20 000 points.
fn edge_points(lr: &[f64], w: usize, h: usize) -> Vec<EdgePoint> {
    let m = 8;
    let mut mags = Vec::new();
    for y in m..h - m {
        for x in m..w - m {
            let (gx, gy) = grad(lr, w, x, y);
            mags.push((gx * gx + gy * gy).sqrt());
        }
    }
    let thr = percentile(mags, 0.98).max(8.0);
    let mut pts = Vec::new();
    for y in (m..h - m).step_by(2) {
        for x in (m..w - m).step_by(2) {
            let (gx, gy) = grad(lr, w, x, y);
            let mag = (gx * gx + gy * gy).sqrt();
            if mag < thr {
                continue;
            }
            if gx.abs() > 2.75 * gy.abs() {
                pts.push(EdgePoint { x, y, horizontal_profile: true });
            } else if gy.abs() > 2.75 * gx.abs() {
                pts.push(EdgePoint { x, y, horizontal_profile: false });
            }
        }
    }
    if pts.len() > 20_000 {
        let step = pts.len() / 20_000 + 1;
        pts = pts.into_iter().step_by(step).collect();
    }
    pts
}

/// 10–90 % rise distance of a 13-sample profile through the edge (linear interpolation).
fn rise(profile: &[f64]) -> Option<f64> {
    let (lo, hi) = profile.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    if hi - lo < 10.0 {
        return None;
    }
    let increasing = profile[profile.len() - 1] > profile[0];
    let p: Vec<f64> = if increasing { profile.to_vec() } else { profile.iter().rev().copied().collect() };
    let at = |level: f64| -> Option<f64> {
        for i in 0..p.len() - 1 {
            if p[i] <= level && p[i + 1] >= level {
                let d = p[i + 1] - p[i];
                return Some(i as f64 + if d == 0.0 { 0.0 } else { (level - p[i]) / d });
            }
        }
        None
    };
    let (a, b) = (at(lo + 0.1 * (hi - lo))?, at(lo + 0.9 * (hi - lo))?);
    Some((b - a).abs())
}

fn profile(l: &[f64], w: usize, p: &EdgePoint) -> Vec<f64> {
    (-6i64..=6)
        .map(|d| {
            let (x, y) = if p.horizontal_profile {
                ((p.x as i64 + d) as usize, p.y)
            } else {
                (p.x, (p.y as i64 + d) as usize)
            };
            l[y * w + x]
        })
        .collect()
}

/// Second difference along the edge tangent (zipper/maze along the edge).
fn tangent_alternation(l: &[f64], w: usize, p: &EdgePoint) -> f64 {
    let v = |dx: i64, dy: i64| l[(p.y as i64 + dy) as usize * w + (p.x as i64 + dx) as usize];
    if p.horizontal_profile {
        (v(0, -1) - 2.0 * v(0, 0) + v(0, 1)).abs()
    } else {
        (v(-1, 0) - 2.0 * v(0, 0) + v(1, 0)).abs()
    }
}

fn laplacian_var(l: &[f64], w: usize, h: usize, mask: &[bool]) -> f64 {
    let mut vals = Vec::new();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if !mask[y * w + x] {
                continue;
            }
            let c = l[y * w + x];
            vals.push(l[y * w + x - 1] + l[y * w + x + 1] + l[(y - 1) * w + x] + l[(y + 1) * w + x]
                - 4.0 * c);
        }
    }
    if vals.is_empty() {
        return f64::NAN;
    }
    let n = vals.len() as f64;
    let mean = vals.iter().sum::<f64>() / n;
    vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n
}

// ---------- all together ----------

pub fn compare(reference: Img, test: Img) -> Report {
    assert_eq!((reference.w, reference.h), (test.w, test.h), "images must have the same size");
    assert_eq!(reference.rgb.len(), test.rgb.len());
    let (w, h) = (reference.w, reference.h);

    // colour
    let mut de = Vec::with_capacity(w * h);
    let mut chroma_ref = Vec::with_capacity(w * h);
    let mut chroma_test = Vec::with_capacity(w * h);
    for (a, b) in reference.rgb.chunks_exact(3).zip(test.rgb.chunks_exact(3)) {
        let la = lab(a[0], a[1], a[2]);
        let lb = lab(b[0], b[1], b[2]);
        de.push(de2000(la, lb));
        chroma_ref.push((la[1] * la[1] + la[2] * la[2]).sqrt());
        chroma_test.push((lb[1] * lb[1] + lb[2] * lb[2]).sqrt());
    }
    let de00_mean = de.iter().sum::<f64>() / de.len() as f64;
    let de00_p99 = percentile(de, 0.99);

    // luma-based measures
    let lr = luma(&reference);
    let lt = luma(&test);
    let mse = lr.iter().zip(&lt).map(|(a, b)| (a - b).powi(2)).sum::<f64>() / lr.len() as f64;
    let psnr_y = if mse == 0.0 { f64::INFINITY } else { 10.0 * (255.0f64 * 255.0 / mse).log10() };
    let (ssim, ms_ssim) = ssim_ms(&lr, &lt, w, h);

    // detail mask: top 20 % gradient magnitude of the reference
    let mut mags = vec![0.0; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let (gx, gy) = grad(&lr, w, x, y);
            mags[y * w + x] = (gx * gx + gy * gy).sqrt();
        }
    }
    let thr = percentile(mags.clone(), 0.80);
    let mask: Vec<bool> = mags.iter().map(|m| *m >= thr && *m > 0.0).collect();
    let hf_ratio = laplacian_var(&lt, w, h, &mask) / laplacian_var(&lr, w, h, &mask);

    // edges: rise distance, false colour, zipper
    let pts = edge_points(&lr, w, h);
    let (mut rr, mut rt) = (Vec::new(), Vec::new());
    let (mut fc_ref, mut fc_test, mut fc_n) = (0.0, 0.0, 0usize);
    let (mut z_ref, mut z_test) = (0.0, 0.0);
    for p in &pts {
        if let (Some(a), Some(b)) = (rise(&profile(&lr, w, p)), rise(&profile(&lt, w, p))) {
            rr.push(a);
            rt.push(b);
        }
        let i = p.y * w + p.x;
        if chroma_ref[i] < 5.0 {
            fc_ref += chroma_ref[i];
            fc_test += chroma_test[i];
            fc_n += 1;
        }
        z_ref += tangent_alternation(&lr, w, p);
        z_test += tangent_alternation(&lt, w, p);
    }
    let mean = |v: &[f64]| if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 };
    Report {
        de00_mean,
        de00_p99,
        ssim,
        ms_ssim,
        psnr_y,
        rise_ref_px: mean(&rr),
        rise_test_px: mean(&rt),
        hf_ratio,
        false_colour_ref: if fc_n > 0 { fc_ref / fc_n as f64 } else { f64::NAN },
        false_colour_test: if fc_n > 0 { fc_test / fc_n as f64 } else { f64::NAN },
        zipper_ratio: if z_ref > 0.0 { z_test / z_ref } else { f64::NAN },
        edge_points: pts.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn de2000_reference_pairs() {
        // Sharma, Wu, Dalal (2005) test data, pairs 1, 7 and 17.
        let d = de2000([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485]);
        assert!((d - 2.0425).abs() < 1e-3, "{d}");
        let d = de2000([50.0, 0.0, 0.0], [50.0, -1.0, 2.0]);
        assert!((d - 2.3669).abs() < 1e-3, "{d}");
        let d = de2000([50.0, 2.5, 0.0], [73.0, 25.0, -18.0]);
        assert!((d - 27.1492).abs() < 1e-3, "{d}");
    }

    fn test_image(w: usize, h: usize, blur: bool) -> Vec<u8> {
        // vertical step edges every 16 px plus a gentle gradient, optionally blurred
        let mut l = vec![0.0f64; w * h];
        for y in 0..h {
            for x in 0..w {
                l[y * w + x] = if (x / 16) % 2 == 0 { 40.0 } else { 200.0 } + (y as f64) * 0.05;
            }
        }
        if blur {
            let src = l.clone();
            for y in 0..h {
                for x in 2..w - 2 {
                    l[y * w + x] = (src[y * w + x - 2] + src[y * w + x - 1] + src[y * w + x]
                        + src[y * w + x + 1] + src[y * w + x + 2]) / 5.0;
                }
            }
        }
        l.iter().flat_map(|v| { let c = v.round().clamp(0.0, 255.0) as u8; [c, c, c] }).collect()
    }

    #[test]
    fn identical_images() {
        let a = test_image(128, 96, false);
        let r = compare(Img { w: 128, h: 96, rgb: &a }, Img { w: 128, h: 96, rgb: &a });
        assert!(r.de00_mean < 1e-9);
        assert!((r.ssim - 1.0).abs() < 1e-9);
        assert!((r.ms_ssim - 1.0).abs() < 1e-6);
        assert!((r.hf_ratio - 1.0).abs() < 1e-9);
        assert!((r.rise_ref_px - r.rise_test_px).abs() < 1e-9);
        assert!(r.edge_points > 0);
    }

    #[test]
    fn blur_is_detected_as_less_sharp() {
        let a = test_image(128, 96, false);
        let b = test_image(128, 96, true);
        let r = compare(Img { w: 128, h: 96, rgb: &a }, Img { w: 128, h: 96, rgb: &b });
        assert!(r.ssim < 0.99, "ssim {}", r.ssim);
        assert!(r.hf_ratio < 0.8, "hf {}", r.hf_ratio);
        assert!(r.rise_test_px > r.rise_ref_px + 1.0, "rise {} vs {}", r.rise_test_px, r.rise_ref_px);
    }

    #[test]
    fn false_colour_is_detected() {
        let a = test_image(128, 96, false);
        let mut b = a.clone();
        for px in b.chunks_exact_mut(3) {
            px[0] = px[0].saturating_add(30); // red cast everywhere
        }
        let r = compare(Img { w: 128, h: 96, rgb: &a }, Img { w: 128, h: 96, rgb: &b });
        assert!(r.false_colour_test > r.false_colour_ref + 5.0);
        assert!(r.de00_mean > 3.0);
    }
}
