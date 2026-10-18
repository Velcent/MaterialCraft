// SPDX-License-Identifier: MIT OR Apache-2.0
//! The specular prefilter: for each roughness level the environment convolved with the GGX lobe around the
//! reflection direction (the usual split-sum assumption that the normal, the view and the reflection coincide).
//! Each output texel sums the source texels inside the lobe's cone, weighted by D(n·h) · (n·l) · solid angle. The
//! source is taken from the halving chain at the coarsest resolution that still resolves the lobe, so the work per
//! level stays small; rough levels are computed small and enlarged afterwards.

use super::latlong::{LatLong, V3, dot};
use std::f32::consts::PI;

/// Rows the lobe's cone should span in the source it is taken from.
const CONE_ROWS: f32 = 24.0;

/// The environment convolved for GGX roughness `rough` (alpha = rough²), at `out_w` × `out_h`.
pub fn level(chain: &[LatLong], rough: f32, out_w: usize, out_h: usize) -> LatLong {
    let Some(base) = chain.first() else { return LatLong { w: out_w, h: out_h, px: vec![[0.0; 3]; out_w * out_h] } };
    let alpha = (rough * rough).clamp(0.0, 1.0);
    if alpha <= 1e-4 {
        return base.resized(out_w, out_h);
    }
    // the cone holding nearly all of the lobe: half-vectors up to tan θh = 3α, so light up to twice that angle
    let cone = (2.0 * (3.0 * alpha).atan()).min(PI);
    let rows_needed = (CONE_ROWS * PI / (2.0 * cone)).ceil() as usize;
    let src = chain.iter().rev().find(|l| l.h >= rows_needed).unwrap_or(base);
    let (w, h) = if src.h < out_h { (src.w, src.h) } else { (out_w, out_h) };
    let px = convolve(src, alpha, cone, w, h);
    let out = LatLong { w, h, px };
    if (w, h) == (out_w, out_h) { out } else { out.resized(out_w, out_h) }
}

/// The source's texel directions and solid angles, computed once per level.
struct Table {
    dirs: Vec<V3>,
    /// Solid angle per row.
    d_omega: Vec<f32>,
    /// Polar angle per row.
    theta: Vec<f32>,
}

fn convolve(src: &LatLong, alpha: f32, cone: f32, w: usize, h: usize) -> Vec<V3> {
    let table = Table {
        dirs: (0..src.h).flat_map(|y| (0..src.w).map(move |x| src.texel_direction(x, y))).collect(),
        d_omega: (0..src.h).map(|y| src.texel_solid_angle(y)).collect(),
        theta: (0..src.h).map(|y| (y as f32 + 0.5) / src.h as f32 * PI).collect(),
    };
    let mut out = vec![[0.0; 3]; w * h];
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 32);
    let rows_per = h.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (chunk_i, chunk) in out.chunks_mut(rows_per * w).enumerate() {
            let table = &table;
            s.spawn(move || {
                for (i, o) in chunk.iter_mut().enumerate() {
                    let (x, y) = (i % w, chunk_i * rows_per + i / w);
                    let r = super::latlong::direction((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                    *o = texel(src, table, r, alpha, cone);
                }
            });
        }
    });
    out
}

/// The convolved radiance in reflection direction `r`.
fn texel(src: &LatLong, t: &Table, r: V3, alpha: f32, cone: f32) -> V3 {
    let a2 = alpha * alpha;
    let theta_r = r[1].clamp(-1.0, 1.0).acos();
    let (sr, cr) = theta_r.sin_cos();
    let cos_cone = cone.cos();
    let row_step = PI / src.h as f32;
    let y0 = (((theta_r - cone) / row_step - 0.5).floor().max(0.0)) as usize;
    let y1 = (((theta_r + cone) / row_step + 0.5).ceil() as usize).min(src.h);
    let phi_r = r[0].atan2(-r[2]);
    let mut sum = [0.0f32; 3];
    let mut wsum = 0.0f32;
    for y in y0..y1 {
        let (Some(&theta), Some(&dw)) = (t.theta.get(y), t.d_omega.get(y)) else { continue };
        let (st, ct) = theta.sin_cos();
        // longitudes of this row inside the cone: cos(angle) = cr·ct + sr·st·cos(Δφ) ≥ cos(cone)
        let denom = sr * st;
        let span = if denom <= 1e-6 {
            PI
        } else {
            let c = (cos_cone - cr * ct) / denom;
            if c >= 1.0 {
                continue;
            }
            if c <= -1.0 { PI } else { c.acos() }
        };
        let (x0, x1) = if span >= PI {
            (0i64, src.w as i64)
        } else {
            let centre = (phi_r / (2.0 * PI) + 0.5) * src.w as f32 - 0.5;
            let half = span / (2.0 * PI) * src.w as f32 + 1.0;
            ((centre - half).floor() as i64, (centre + half).ceil() as i64 + 1)
        };
        let x1 = x1.min(x0 + src.w as i64);
        for xi in x0..x1 {
            let x = xi.rem_euclid(src.w as i64) as usize;
            let idx = y * src.w + x;
            let Some(&l) = t.dirs.get(idx) else { continue };
            let nl = dot(r, l);
            if nl <= 0.0 {
                continue;
            }
            // half vector of r and l; with n = v = r, n·h = sqrt((1 + r·l) / 2)
            let nh2 = (1.0 + nl) * 0.5;
            let d = a2 / (PI * (nh2 * (a2 - 1.0) + 1.0).powi(2));
            let wgt = d * nl * dw;
            let p = src.px.get(idx).copied().unwrap_or([0.0; 3]);
            for c in 0..3 {
                sum[c] += p[c] * wgt;
            }
            wsum += wgt;
        }
    }
    if wsum > 0.0 { sum.map(|v| v / wsum) } else { src.sample(r) }
}

#[cfg(test)]
mod tests {
    use super::super::latlong::{normalize, uv_of};
    use super::*;

    fn spot() -> LatLong {
        let s = normalize([0.3, 0.5, -0.8]);
        LatLong::from_fn(256, 128, |d| if dot(d, s) > 0.995 { [200.0, 100.0, 50.0] } else { [0.2, 0.2, 0.2] })
    }

    #[test]
    fn rougher_levels_spread_a_bright_spot_and_keep_its_colour() {
        let img = spot();
        let chain = img.chain(4);
        let s = normalize([0.3, 0.5, -0.8]);
        let peak = |l: &LatLong| l.sample(s)[0];
        let mirror = level(&chain, 0.0, 128, 64);
        let mid = level(&chain, 0.4, 128, 64);
        let rough = level(&chain, 1.0, 128, 64);
        assert!(peak(&mirror) > peak(&mid) && peak(&mid) > peak(&rough), "{} {} {}", peak(&mirror), peak(&mid), peak(&rough));
        // away from the spot the rough level is brighter than the mirror (the spot's light spread out)
        let side = normalize([-0.2, 0.5, -0.8]);
        assert!(rough.sample(side)[0] > mirror.sample(side)[0]);
        // the spot's hue survives
        let c = mid.sample(s);
        assert!(c[0] > c[1] && c[1] > c[2]);
        // a uniform environment stays uniform at every roughness
        let flat = LatLong::from_fn(64, 32, |_| [1.0, 2.0, 3.0]);
        let fc = flat.chain(4);
        for r in [0.2, 0.6, 1.0] {
            let l = level(&fc, r, 32, 16);
            assert!(l.px.iter().all(|p| (p[0] - 1.0).abs() < 1e-3 && (p[2] - 3.0).abs() < 3e-3), "rough {r}");
        }
        let _ = uv_of(s);
    }
}
