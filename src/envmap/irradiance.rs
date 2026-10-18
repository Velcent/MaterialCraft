// SPDX-License-Identifier: MIT OR Apache-2.0
//! Diffuse light: the environment projected onto the nine spherical harmonics of bands 0–2, convolved with the
//! clamped cosine (the irradiance E(n) is smooth enough that nine coefficients carry nearly all of it).

use super::latlong::{LatLong, V3};
use std::f32::consts::PI;

/// Nine RGB coefficients.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sh9(pub [V3; 9]);

/// The real SH basis of bands 0–2 at unit direction `d`.
fn basis(d: V3) -> [f32; 9] {
    let [x, y, z] = d;
    [
        0.282_095,
        0.488_603 * y,
        0.488_603 * z,
        0.488_603 * x,
        1.092_548 * x * y,
        1.092_548 * y * z,
        0.315_392 * (3.0 * z * z - 1.0),
        1.092_548 * x * z,
        0.546_274 * (x * x - y * y),
    ]
}

/// The clamped cosine's band factors: π, 2π/3, π/4.
const BAND: [f32; 9] = [PI, 2.0 * PI / 3.0, 2.0 * PI / 3.0, 2.0 * PI / 3.0, PI / 4.0, PI / 4.0, PI / 4.0, PI / 4.0, PI / 4.0];

impl Sh9 {
    /// The radiance of `img` projected onto the basis.
    pub fn project(img: &LatLong) -> Sh9 {
        let mut c = [[0.0f32; 3]; 9];
        for y in 0..img.h {
            let dw = img.texel_solid_angle(y);
            for x in 0..img.w {
                let (p, b) = (img.get(x, y), basis(img.texel_direction(x, y)));
                for (ci, bi) in c.iter_mut().zip(b) {
                    for k in 0..3 {
                        ci[k] += p[k] * bi * dw;
                    }
                }
            }
        }
        Sh9(c)
    }

    /// Irradiance E(n) = ∫ L(ω) max(n·ω, 0) dω at unit normal `n` (never negative).
    pub fn irradiance(&self, n: V3) -> V3 {
        let b = basis(n);
        let mut e = [0.0f32; 3];
        for ((ci, bi), a) in self.0.iter().zip(b).zip(BAND) {
            for k in 0..3 {
                e[k] += ci[k] * bi * a;
            }
        }
        e.map(|v| v.max(0.0))
    }

    /// E(n) / π for every direction of a `w` × `h` image: what a white diffuse surface facing that way reflects.
    pub fn diffuse_image(&self, w: usize, h: usize) -> LatLong {
        LatLong::from_fn(w, h, |n| self.irradiance(n).map(|v| v / PI))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uniform_sky_gives_pi_times_its_radiance() {
        let sh = Sh9::project(&LatLong::from_fn(64, 32, |_| [1.0, 0.5, 0.25]));
        for n in [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, -0.6, 0.8]] {
            let e = sh.irradiance(n);
            assert!((e[0] - PI).abs() < 0.02 && (e[2] - PI / 4.0).abs() < 0.01, "{e:?}");
        }
    }

    #[test]
    fn light_from_above_lights_upward_faces_only() {
        // the upper hemisphere at radiance 1: E(up) = π, E(down) ≈ 0, E(side) = π / 2
        let sh = Sh9::project(&LatLong::from_fn(128, 64, |d| if d[1] > 0.0 { [1.0; 3] } else { [0.0; 3] }));
        let (up, down, side) = (sh.irradiance([0.0, 1.0, 0.0])[0], sh.irradiance([0.0, -1.0, 0.0])[0], sh.irradiance([1.0, 0.0, 0.0])[0]);
        assert!((up - PI).abs() < 0.15 && down < 0.15 && (side - PI / 2.0).abs() < 0.05, "{up} {down} {side}");
    }
}
