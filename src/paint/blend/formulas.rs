//! Per-component formulas of the blend modes (see [`BlendMode`] for the list).

use super::BlendMode;

fn dodge(b: f32, t: f32) -> f32 {
    if t >= 1.0 { if b > 0.0 { 1.0 } else { 0.0 } } else { (b / (1.0 - t)).min(1.0) }
}

fn burn(b: f32, t: f32) -> f32 {
    if t <= 0.0 { if b >= 1.0 { 1.0 } else { 0.0 } } else { (1.0 - (1.0 - b) / t).max(0.0) }
}

fn div(a: f32, b: f32) -> f32 {
    if b.abs() <= 1e-6 { if a > 0.0 { 1.0 } else { 0.0 } } else { a / b }
}

pub(super) fn separable(mode: BlendMode, b: f32, t: f32) -> f32 {
    use BlendMode::*;
    match mode {
        Multiply => b * t,
        Divide => div(b, t),
        InverseDivide => div(t, b),
        Darken => b.min(t),
        Lighten => b.max(t),
        Add => b + t,
        Subtract => b - t,
        Difference => (b - t).abs(),
        Exclusion => b + t - 2.0 * b * t,
        SignedAdd => b + 2.0 * t - 1.0,
        Screen => 1.0 - (1.0 - b) * (1.0 - t),
        Overlay => separable(HardLight, t, b),
        HardLight => {
            if t < 0.5 {
                2.0 * b * t
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - t)
            }
        }
        SoftLight => {
            if t <= 0.5 {
                b - (1.0 - 2.0 * t) * b * (1.0 - b)
            } else {
                let d = if b <= 0.25 { ((16.0 * b - 12.0) * b + 4.0) * b } else { b.max(0.0).sqrt() };
                b + (2.0 * t - 1.0) * (d - b)
            }
        }
        ColorDodge => dodge(b, t),
        ColorBurn => burn(b, t),
        LinearBurn => (b + t - 1.0).max(0.0),
        VividLight => {
            if t < 0.5 {
                burn(b, 2.0 * t)
            } else {
                dodge(b, 2.0 * t - 1.0)
            }
        }
        LinearLight => (b + 2.0 * t - 1.0).clamp(0.0, 1.0),
        PinLight => {
            if t < 0.5 {
                b.min(2.0 * t)
            } else {
                b.max(2.0 * t - 1.0)
            }
        }
        _ => t,
    }
}

/// W3C "Compositing and Blending Level 1" non-separable modes.
pub(super) mod nonseparable {
    use crate::paint::BlendMode;
    type C = [f32; 3];

    fn lum(c: C) -> f32 {
        0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
    }

    fn clip(c: C) -> C {
        let l = lum(c);
        let n = c[0].min(c[1]).min(c[2]);
        let x = c[0].max(c[1]).max(c[2]);
        c.map(|v| {
            let mut v = v;
            if n < 0.0 && l - n > 1e-9 {
                v = l + (v - l) * l / (l - n);
            }
            if x > 1.0 && x - l > 1e-9 {
                v = l + (v - l) * (1.0 - l) / (x - l);
            }
            v
        })
    }

    fn set_lum(c: C, l: f32) -> C {
        let d = l - lum(c);
        clip(c.map(|v| v + d))
    }

    fn sat(c: C) -> f32 {
        c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
    }

    fn set_sat(c: C, s: f32) -> C {
        let (mn, mx) = (c[0].min(c[1]).min(c[2]), c[0].max(c[1]).max(c[2]));
        if mx - mn <= 1e-9 {
            return [0.0; 3];
        }
        c.map(|v| (v - mn) * s / (mx - mn))
    }

    pub fn apply(mode: BlendMode, b: C, t: C) -> C {
        match mode {
            BlendMode::Hue => set_lum(set_sat(t, sat(b)), lum(b)),
            BlendMode::Saturation => set_lum(set_sat(b, sat(t)), lum(b)),
            BlendMode::Color => set_lum(t, lum(b)),
            _ => set_lum(b, lum(t)),
        }
    }
}
