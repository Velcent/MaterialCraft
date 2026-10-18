// SPDX-License-Identifier: MIT OR Apache-2.0
//! Dominant directions: the few bright regions of an environment (a sun, a window, a softbox) that cast
//! visible shadows. Each becomes a [`DominantLight`]: its energy-weighted direction, the irradiance it gives a
//! surface facing it, and how far its light spreads (which softens its shadow). An environment without a clear
//! bright region (an overcast sky) gets one broad light where most of its light comes from, carrying half of the
//! irradiance there, so its shadows are soft and partial.

use super::irradiance::Sh9;
use super::latlong::{LatLong, V3, dot, luminance, normalize};

/// At most this many lights.
pub const MAX_LIGHTS: usize = 4;
/// A region counts when its peak is this many times the environment's mean radiance …
const PEAK_OVER_MEAN: f32 = 4.0;
/// … and it carries at least this share of the environment's energy.
const MIN_SHARE: f32 = 0.03;
/// Texels within this angle (radians) of the peak and at least `REGION_LEVEL` of its radiance join its region.
const REGION_ANGLE: f32 = 0.5;
const REGION_LEVEL: f32 = 0.2;
/// The spread of the light found when nothing stands out, in radians.
const BROAD_SPREAD: f32 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DominantLight {
    /// Unit direction towards the light.
    pub dir: V3,
    /// Irradiance on a surface facing the light (its radiance summed over its solid angle), linear RGB.
    pub irradiance: V3,
    /// Angular radius of the light (radians): how far shadow samples spread around `dir`.
    pub spread: f32,
}

/// The dominant lights of `img` (best a small image, around 64 × 32), brightest first; `sh` is the
/// environment's diffuse projection (used for the broad fallback light).
pub fn find(img: &LatLong, sh: &Sh9) -> Vec<DominantLight> {
    let n = img.w * img.h;
    let dirs: Vec<V3> = (0..n).map(|i| img.texel_direction(i % img.w, i / img.w)).collect();
    let d_omega: Vec<f32> = (0..n).map(|i| img.texel_solid_angle(i / img.w)).collect();
    let lum: Vec<f32> = img.px.iter().map(|p| luminance(*p)).collect();
    let total: f32 = lum.iter().zip(&d_omega).map(|(l, w)| l * w).sum();
    if total.is_nan() || total <= 0.0 {
        return Vec::new();
    }
    let mean = total / (4.0 * std::f32::consts::PI);
    let mut taken = vec![false; n];
    let mut lights = Vec::new();
    while lights.len() < MAX_LIGHTS {
        let Some((peak_i, peak)) = lum.iter().enumerate().filter(|(i, _)| !taken.get(*i).copied().unwrap_or(true)).max_by(|a, b| a.1.total_cmp(b.1)) else {
            break;
        };
        let peak = *peak;
        if peak < PEAK_OVER_MEAN * mean {
            break;
        }
        let centre = dirs.get(peak_i).copied().unwrap_or([0.0, 1.0, 0.0]);
        let cos_region = REGION_ANGLE.cos();
        let mut e = [0.0f32; 3];
        let mut weighted = [0.0f32; 3];
        let mut members = Vec::new();
        for i in 0..n {
            let (Some(&d), Some(&l)) = (dirs.get(i), lum.get(i)) else { continue };
            if taken.get(i).copied().unwrap_or(true) || dot(d, centre) < cos_region || l < REGION_LEVEL * peak {
                continue;
            }
            let dw = d_omega.get(i).copied().unwrap_or(0.0);
            let p = img.px.get(i).copied().unwrap_or([0.0; 3]);
            for c in 0..3 {
                e[c] += p[c] * dw;
                weighted[c] += d[c] * l * dw;
            }
            members.push(i);
        }
        let energy = luminance(e);
        if energy < MIN_SHARE * total {
            break;
        }
        let dir = normalize(weighted);
        let spread2 = members
            .iter()
            .map(|&i| {
                let (d, l, dw) = (dirs.get(i).copied().unwrap_or(dir), lum.get(i).copied().unwrap_or(0.0), d_omega.get(i).copied().unwrap_or(0.0));
                dot(d, dir).clamp(-1.0, 1.0).acos().powi(2) * l * dw
            })
            .sum::<f32>()
            / energy;
        for &i in &members {
            if let Some(t) = taken.get_mut(i) {
                *t = true;
            }
        }
        // a region narrower than a texel is at least half a texel wide
        let texel = std::f32::consts::PI / img.h as f32;
        lights.push(DominantLight { dir, irradiance: e, spread: spread2.sqrt().max(texel * 0.5) });
    }
    if lights.is_empty() {
        let mut mean_dir = [0.0f32; 3];
        for i in 0..n {
            let (d, l, dw) = (dirs.get(i).copied().unwrap_or([0.0; 3]), lum.get(i).copied().unwrap_or(0.0), d_omega.get(i).copied().unwrap_or(0.0));
            for c in 0..3 {
                mean_dir[c] += d[c] * l * dw;
            }
        }
        let dir = if dot(mean_dir, mean_dir) > (total * 0.05).powi(2) { normalize(mean_dir) } else { [0.0, 1.0, 0.0] };
        lights.push(DominantLight { dir, irradiance: sh.irradiance(dir).map(|v| v * 0.5), spread: BROAD_SPREAD });
    }
    lights
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sun_is_found_where_it_is_with_its_energy() {
        let sun = normalize([0.5, 0.6, 0.3]);
        let img = LatLong::from_fn(128, 64, |d| if dot(d, sun) > 0.998 { [5000.0, 4500.0, 4000.0] } else { [0.3, 0.4, 0.6] });
        let small = img.resized(64, 32);
        let lights = find(&small, &Sh9::project(&small));
        assert!(!lights.is_empty());
        let l = lights[0];
        assert!(dot(l.dir, sun) > 0.995, "{:?}", l.dir);
        // the sun's disc: solid angle 2π(1 − 0.998) ≈ 0.01257 sr at 5000
        let expect = 5000.0 * 2.0 * std::f32::consts::PI * 0.002;
        assert!((l.irradiance[0] - expect).abs() / expect < 0.25, "{} vs {expect}", l.irradiance[0]);
        assert!(l.spread < 0.1);
    }

    #[test]
    fn two_windows_give_two_lights_and_an_overcast_sky_one_broad_one() {
        let (a, b) = (normalize([1.0, 0.3, 0.0]), normalize([-0.7, 0.4, 0.6]));
        let img = LatLong::from_fn(64, 32, |d| if dot(d, a) > 0.97 || dot(d, b) > 0.97 { [50.0; 3] } else { [0.5; 3] });
        let lights = find(&img, &Sh9::project(&img));
        assert_eq!(lights.len(), 2);
        let overcast = LatLong::from_fn(64, 32, |d| [1.0 + d[1].max(0.0); 3]);
        let lights = find(&overcast, &Sh9::project(&overcast));
        assert_eq!(lights.len(), 1);
        assert!(lights[0].dir[1] > 0.5 && lights[0].spread == BROAD_SPREAD, "{:?}", lights[0]);
        assert!(find(&LatLong::from_fn(8, 4, |_| [0.0; 3]), &Sh9([[0.0; 3]; 9])).is_empty());
    }
}
