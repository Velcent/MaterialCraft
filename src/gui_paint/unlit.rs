//! Showing raw texture values on the model without lighting.
//!
//! The GPU viewer has no unlit mode, so a single channel is fed in as emissive on a black, rough,
//! non-metal surface while the lights are switched off. The shader then outputs
//! `pow(tonemap(emissive), 1/2.2)`; [`emissive_for`] inverts that, so a value `v` shows as the screen
//! colour `v`. The emissive texture is 8-bit sRGB (linear ≤ 1), which caps exact display at a
//! luminance of about 0.9; brighter values are clipped there.

/// Shader luminance weights (Rec. 709).
const LUM: [f32; 3] = [0.2126, 0.7152, 0.0722];
/// `tonemap` keeps the hue below this luminance; above, it starts mixing in a per-channel curve.
const HUE_SAFE_LUM: f32 = 2.0;

/// The shader's ACES fit on luminance.
fn aces(x: f32) -> f32 {
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// Inverse of [`aces`] for `y` in `0..aces(HUE_SAFE_LUM)`.
fn aces_inverse(y: f32) -> f32 {
    let (a, b, c) = (2.51 - 2.43 * y, 0.03 - 0.59 * y, -0.14 * y);
    (-b + (b * b - 4.0 * a * c).max(0.0).sqrt()) / (2.0 * a)
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// The sRGB-encoded emissive texel that the unlit viewer shows as the screen colour `rgb` (0..1).
pub fn emissive_for(rgb: [f32; 3]) -> [f32; 3] {
    let t = rgb.map(|v| v.clamp(0.0, 1.0).powf(2.2));
    let lt = t[0] * LUM[0] + t[1] * LUM[1] + t[2] * LUM[2];
    // the slope of `aces` at 0 is 0.03 / 0.14
    let k = if lt < 1e-6 { 0.14 / 0.03 } else { aces_inverse(lt.min(aces(HUE_SAFE_LUM) - 1e-4)) / lt };
    t.map(|v| linear_to_srgb((v * k).min(1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the shader shows for an sRGB emissive texel with the lights off.
    fn shown(e: [f32; 3]) -> [f32; 3] {
        let lin = e.map(|v| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) });
        let l = lin[0] * LUM[0] + lin[1] * LUM[1] + lin[2] * LUM[2];
        let k = aces(l) / l.max(1e-4);
        lin.map(|v| (v * k).min(1.0).powf(1.0 / 2.2))
    }

    #[test]
    fn round_trip_below_the_cap() {
        for rgb in [[0.8, 0.2, 0.2], [0.5, 0.5, 0.5], [0.1, 0.6, 0.9], [0.02, 0.02, 0.02], [0.85, 0.85, 0.85]] {
            let s = shown(emissive_for(rgb));
            for c in 0..3 {
                assert!((s[c] - rgb[c]).abs() < 0.01, "{rgb:?} shows as {s:?}");
            }
        }
    }
}
