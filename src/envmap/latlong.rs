// SPDX-License-Identifier: MIT OR Apache-2.0
//! [`LatLong`]: an equirectangular (latitude-longitude) RGB image of the sphere of directions, and the mapping
//! between directions and image positions. The convention is the starter library's studio environment's:
//! +Y up, row 0 looks straight up, the image centre looks along −Z, u grows towards +X.

use std::f32::consts::{PI, TAU};

pub type V3 = [f32; 3];

/// Largest radiance kept (a sun can be brighter, but the GPU stores half floats, and energy above this would
/// only make fireflies).
pub const MAX_RADIANCE: f32 = 60000.0;

/// The direction of image position (u, v), both 0..1.
pub fn direction(u: f32, v: f32) -> V3 {
    let (phi, theta) = ((u - 0.5) * TAU, v * PI);
    let (st, ct) = theta.sin_cos();
    [st * phi.sin(), ct, -st * phi.cos()]
}

/// The image position (u, v) of a direction (need not be normalised; zero gives the centre).
pub fn uv_of(d: V3) -> (f32, f32) {
    let len = dot(d, d).sqrt();
    if len <= 0.0 || !len.is_finite() {
        return (0.5, 0.5);
    }
    let y = (d[1] / len).clamp(-1.0, 1.0);
    let u = d[0].atan2(-d[2]) / TAU + 0.5;
    (u.rem_euclid(1.0), y.acos() / PI)
}

pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-20 { [a[0] / l, a[1] / l, a[2] / l] } else { [0.0, 1.0, 0.0] }
}

/// Rec. 709 luminance.
pub fn luminance(c: V3) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

#[derive(Debug, Clone, PartialEq)]
pub struct LatLong {
    pub w: usize,
    pub h: usize,
    /// Linear RGB radiance, row by row from the top.
    pub px: Vec<V3>,
}

impl LatLong {
    /// An image from linear RGB pixels; non-finite and negative values become 0, very bright ones are limited
    /// to [`MAX_RADIANCE`]. `None` if the sizes don't match or the image is empty.
    pub fn new(w: usize, h: usize, px: Vec<V3>) -> Option<LatLong> {
        if w == 0 || h == 0 || px.len() != w * h {
            return None;
        }
        let clean = |v: f32| if v.is_finite() { v.clamp(0.0, MAX_RADIANCE) } else { 0.0 };
        Some(LatLong { w, h, px: px.into_iter().map(|p| p.map(clean)).collect() })
    }

    pub fn from_fn(w: usize, h: usize, f: impl Fn(V3) -> V3) -> LatLong {
        let px = (0..w * h).map(|i| f(direction(((i % w) as f32 + 0.5) / w as f32, ((i / w) as f32 + 0.5) / h as f32))).collect();
        LatLong { w, h, px }
    }

    pub fn get(&self, x: usize, y: usize) -> V3 {
        self.px.get(y * self.w + x).copied().unwrap_or([0.0; 3])
    }

    /// Solid angle of one texel of row `y`.
    pub fn texel_solid_angle(&self, y: usize) -> f32 {
        let theta = (y as f32 + 0.5) / self.h as f32 * PI;
        (TAU / self.w as f32) * (PI / self.h as f32) * theta.sin()
    }

    /// The direction of texel (x, y)'s centre.
    pub fn texel_direction(&self, x: usize, y: usize) -> V3 {
        direction((x as f32 + 0.5) / self.w as f32, (y as f32 + 0.5) / self.h as f32)
    }

    /// Bilinear sample in direction `d` (wrapping around horizontally).
    pub fn sample(&self, d: V3) -> V3 {
        let (u, v) = uv_of(d);
        let (x, y) = (u * self.w as f32 - 0.5, (v * self.h as f32 - 0.5).clamp(0.0, (self.h - 1) as f32));
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let xi = |o: f32| ((x0 + o) as i64).rem_euclid(self.w as i64) as usize;
        let yi = |o: f32| ((y0 + o) as usize).min(self.h - 1);
        let mut out = [0.0; 3];
        for (dx, dy, wgt) in [(0.0, 0.0, (1.0 - fx) * (1.0 - fy)), (1.0, 0.0, fx * (1.0 - fy)), (0.0, 1.0, (1.0 - fx) * fy), (1.0, 1.0, fx * fy)] {
            let p = self.get(xi(dx), yi(dy));
            for c in 0..3 {
                out[c] += p[c] * wgt;
            }
        }
        out
    }

    /// The image at `w` × `h`: each axis is averaged over the covered texels when shrinking (keeping the energy
    /// of small bright spots) and interpolated when growing; horizontally the image wraps around.
    pub fn resized(&self, w: usize, h: usize) -> LatLong {
        let (w, h) = (w.max(1), h.max(1));
        let mut rows = Vec::with_capacity(w * self.h);
        for y in 0..self.h {
            let row = self.px.get(y * self.w..(y + 1) * self.w).unwrap_or(&[]);
            rows.extend(resample(row, w, None));
        }
        let mut px = vec![[0.0; 3]; w * h];
        let mut column = vec![[0.0; 3]; self.h];
        // vertically, texels near the poles cover less of the sphere: weigh them by their solid angle
        let weights: Vec<f32> = (0..self.h).map(|y| ((y as f32 + 0.5) / self.h as f32 * std::f32::consts::PI).sin()).collect();
        for x in 0..w {
            for (y, c) in column.iter_mut().enumerate() {
                *c = rows.get(y * w + x).copied().unwrap_or([0.0; 3]);
            }
            for (y, v) in resample(&column, h, Some(&weights)).into_iter().enumerate() {
                if let Some(p) = px.get_mut(y * w + x) {
                    *p = v;
                }
            }
        }
        LatLong { w, h, px }
    }

    /// Halving steps down to `min_h` rows: `[self, self / 2, …]`.
    pub fn chain(&self, min_h: usize) -> Vec<LatLong> {
        let mut out = vec![self.clone()];
        while let Some(last) = out.last() {
            if last.h / 2 < min_h.max(1) || last.w < 2 {
                break;
            }
            let next = last.resized(last.w / 2, last.h / 2);
            out.push(next);
        }
        out
    }
}

/// One line resampled to `n` values: box average when shrinking (weighted by `weights` when given), linear
/// interpolation when growing. Without weights the line wraps around (rows); with them it is clamped (columns).
fn resample(src: &[V3], n: usize, weights: Option<&[f32]>) -> Vec<V3> {
    let m = src.len();
    if m == 0 {
        return vec![[0.0; 3]; n];
    }
    if m == n {
        return src.to_vec();
    }
    let wrap = weights.is_none();
    let index = |i: i64| -> usize { (if wrap { i.rem_euclid(m as i64) } else { i.clamp(0, m as i64 - 1) }) as usize };
    let at = |i: i64| -> V3 { src.get(index(i)).copied().unwrap_or([0.0; 3]) };
    let weight = |i: i64| -> f32 { weights.and_then(|w| w.get(index(i)).copied()).unwrap_or(1.0) };
    let scale = m as f32 / n as f32;
    (0..n)
        .map(|i| {
            if scale > 1.0 {
                let (a, b) = (i as f32 * scale, (i + 1) as f32 * scale);
                let mut sum = [0.0; 3];
                let mut wsum = 0.0;
                let mut j = a.floor();
                while j < b {
                    let cover = (b.min(j + 1.0) - a.max(j)).max(0.0) * weight(j as i64);
                    let p = at(j as i64);
                    for c in 0..3 {
                        sum[c] += p[c] * cover;
                    }
                    wsum += cover;
                    j += 1.0;
                }
                if wsum > 0.0 { sum.map(|v| v / wsum) } else { sum }
            } else {
                let x = (i as f32 + 0.5) * scale - 0.5;
                let x0 = x.floor();
                let f = x - x0;
                let (p, q) = (at(x0 as i64), at(x0 as i64 + 1));
                [0, 1, 2].map(|c| p[c] * (1.0 - f) + q[c] * f)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_and_positions_invert() {
        for &(u, v) in &[(0.5, 0.5), (0.1, 0.3), (0.9, 0.8), (0.26, 0.05)] {
            let (u2, v2) = uv_of(direction(u, v));
            assert!((u - u2).abs() < 1e-4 && (v - v2).abs() < 1e-4, "{u} {v} -> {u2} {v2}");
        }
        let c = direction(0.5, 0.5);
        assert!((c[2] + 1.0).abs() < 1e-5, "the centre looks along -Z");
        assert!((direction(0.75, 0.5)[0] - 1.0).abs() < 1e-5, "u grows towards +X");
        assert!((direction(0.3, 0.0)[1] - 1.0).abs() < 1e-5, "row 0 looks up");
    }

    #[test]
    fn solid_angles_cover_the_sphere_and_shrinking_keeps_energy() {
        let img = LatLong::from_fn(64, 32, |d| if d[1] > 0.95 { [100.0, 0.0, 0.0] } else { [0.1, 0.2, 0.3] });
        let total: f32 = (0..img.h).map(|y| img.texel_solid_angle(y) * img.w as f32).sum();
        assert!((total - 4.0 * PI).abs() < 0.01, "{total}");
        let energy = |l: &LatLong| -> f32 { (0..l.h).flat_map(|y| (0..l.w).map(move |x| (x, y))).map(|(x, y)| l.get(x, y)[0] * l.texel_solid_angle(y)).sum() };
        let small = img.resized(16, 8);
        let (e0, e1) = (energy(&img), energy(&small));
        assert!((e0 - e1).abs() / e0 < 0.05, "{e0} {e1}");
        assert_eq!(img.chain(4).iter().map(|l| l.h).collect::<Vec<_>>(), vec![32, 16, 8, 4]);
        let big = small.resized(64, 32);
        assert_eq!((big.w, big.h), (64, 32));
    }

    #[test]
    fn bad_pixels_are_cleaned() {
        let l = LatLong::new(2, 1, vec![[f32::NAN, -1.0, 1e9], [1.0, 2.0, 3.0]]).unwrap();
        assert_eq!(l.px[0], [0.0, 0.0, MAX_RADIANCE]);
        assert!(LatLong::new(2, 2, vec![[0.0; 3]]).is_none());
        assert_eq!(l.sample([0.0, 0.0, -1.0]).len(), 3);
    }
}
