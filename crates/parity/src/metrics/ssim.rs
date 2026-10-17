//! Structural similarity (SSIM, Wang et al. 2004) of one component over the masked texels, computed on
//! non-overlapping 8×8 windows (only windows mostly inside the islands count). Cheap and good enough to see
//! whether structure matches where absolute values differ.

use materialcraft::tex::Tex;

const WINDOW: usize = 8;
/// Share of a window's texels that must be inside the mask.
const MIN_INSIDE: f64 = 0.75;
const C1: f64 = 0.01 * 0.01;
const C2: f64 = 0.03 * 0.03;

/// Mean SSIM (−1..1, 1 = identical); `None` when no window is inside the mask.
pub fn ssim(ours: &Tex, theirs: &Tex, component: usize, mask: &[bool]) -> Option<f64> {
    let (w, h) = (ours.w, ours.h);
    if theirs.w != w || theirs.h != h || mask.len() != w * h {
        return None;
    }
    let (mut total, mut windows) = (0f64, 0usize);
    for wy in (0..h).step_by(WINDOW) {
        for wx in (0..w).step_by(WINDOW) {
            let mut s = Sums::default();
            for y in wy..(wy + WINDOW).min(h) {
                for x in wx..(wx + WINDOW).min(w) {
                    let i = y * w + x;
                    if mask[i] {
                        s.add(ours.px[i][component] as f64, theirs.px[i][component] as f64);
                    }
                }
            }
            if s.n >= MIN_INSIDE * (WINDOW * WINDOW) as f64 {
                total += s.ssim();
                windows += 1;
            }
        }
    }
    (windows > 0).then(|| total / windows as f64)
}

#[derive(Default)]
struct Sums {
    n: f64,
    a: f64,
    b: f64,
    aa: f64,
    bb: f64,
    ab: f64,
}

impl Sums {
    fn add(&mut self, a: f64, b: f64) {
        let (a, b) = (if a.is_finite() { a } else { 0.0 }, if b.is_finite() { b } else { 0.0 });
        self.n += 1.0;
        self.a += a;
        self.b += b;
        self.aa += a * a;
        self.bb += b * b;
        self.ab += a * b;
    }

    fn ssim(&self) -> f64 {
        let (ma, mb) = (self.a / self.n, self.b / self.n);
        let va = (self.aa / self.n - ma * ma).max(0.0);
        let vb = (self.bb / self.n - mb * mb).max(0.0);
        let cov = self.ab / self.n - ma * mb;
        ((2.0 * ma * mb + C1) * (2.0 * cov + C2)) / ((ma * ma + mb * mb + C1) * (va + vb + C2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(scale: f32) -> Tex {
        Tex::from_fn(16, 16, |u, v| [scale * (u + v) * 0.5, 0.0, 0.0, 1.0])
    }

    #[test]
    fn identical_is_one_and_different_is_less() {
        let mask = vec![true; 256];
        assert!((ssim(&ramp(1.0), &ramp(1.0), 0, &mask).unwrap() - 1.0).abs() < 1e-9);
        assert!(ssim(&ramp(1.0), &ramp(0.2), 0, &mask).unwrap() < 0.9);
        assert_eq!(ssim(&ramp(1.0), &ramp(1.0), 0, &[false; 256]), None);
        assert_eq!(ssim(&ramp(1.0), &Tex::solid(2, 2, [0.0; 4]), 0, &mask), None);
    }
}
