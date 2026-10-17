//! Per-value tone helpers the generators share: balance/contrast scans, level thresholds, brightness and the
//! blend modes masks are layered with.

/// Smooth 0..1 ramp between `a` and `b`.
pub fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let d = b - a;
    let t = if d.abs() < 1e-6 { if x >= b { 1.0 } else { 0.0 } } else { ((x - a) / d).clamp(0.0, 1.0) };
    t * t * (3.0 - 2.0 * t)
}

pub fn unit(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
}

/// Balance and contrast of a 0..1 value: balance 0.5 and contrast 0 leave it unchanged; a higher balance
/// moves the result towards white, contrast 1 makes it a hard threshold at `1 − balance`.
pub fn scan(v: f32, balance: f32, contrast: f32) -> f32 {
    let centre = 1.0 - unit(balance);
    let width = (1.0 - unit(contrast)).max(0.005);
    unit((v - centre) / width + 0.5)
}

/// A level threshold with a soft edge: values above `1 − level` become white; `contrast` 0 is a wide ramp,
/// 1 a hard edge. Used for "how much" sliders (dirt level, wear level).
pub fn threshold(v: f32, level: f32, contrast: f32) -> f32 {
    let t = 1.0 - unit(level);
    let w = (1.0 - unit(contrast)) * 0.5 + 0.01;
    smooth(t - w, t + w, v)
}

/// Brightness (−1..1, added) and contrast (−1..1, around 0.5) of a 0..1 value.
pub fn brightness_contrast(v: f32, brightness: f32, contrast: f32) -> f32 {
    let c = contrast.clamp(-0.99, 0.99);
    let k = if c >= 0.0 { 1.0 / (1.0 - c) } else { 1.0 + c };
    unit((v - 0.5) * k + 0.5 + brightness)
}

pub fn invert_if(v: f32, invert: bool) -> f32 {
    if invert { 1.0 - v } else { v }
}

/// How a mask component is laid over what is below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layering {
    Replace,
    Add,
    Subtract,
    Multiply,
    /// Above 0.5 adds, below subtracts.
    AddSub,
    Max,
    Min,
    Overlay,
    Screen,
}

impl Layering {
    /// Our own numbering, used by generator parameters: 0 replace, 1 add, 2 subtract, 3 multiply, 4 add/sub,
    /// 5 max, 6 min, 7 overlay, 8 screen; anything else is add.
    pub fn from_index(i: i32) -> Layering {
        match i {
            0 => Layering::Replace,
            2 => Layering::Subtract,
            3 => Layering::Multiply,
            4 => Layering::AddSub,
            5 => Layering::Max,
            6 => Layering::Min,
            7 => Layering::Overlay,
            8 => Layering::Screen,
            _ => Layering::Add,
        }
    }

    /// `top` over `below` at `opacity` (all 0..1).
    pub fn mix(self, below: f32, top: f32, opacity: f32) -> f32 {
        let r = match self {
            Layering::Replace => top,
            Layering::Add => below + top,
            Layering::Subtract => below - top,
            Layering::Multiply => below * top,
            Layering::AddSub => below + 2.0 * top - 1.0,
            Layering::Max => below.max(top),
            Layering::Min => below.min(top),
            Layering::Overlay => {
                if below < 0.5 {
                    2.0 * below * top
                } else {
                    1.0 - 2.0 * (1.0 - below) * (1.0 - top)
                }
            }
            Layering::Screen => 1.0 - (1.0 - below) * (1.0 - top),
        };
        let o = unit(opacity);
        unit(below + (unit(r) - below) * o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_settings_change_nothing() {
        for v in [0.0, 0.3, 0.8] {
            assert!((scan(v, 0.5, 0.0) - v).abs() < 1e-6);
            assert!((brightness_contrast(v, 0.0, 0.0) - v).abs() < 1e-6);
            assert_eq!(Layering::Multiply.mix(v, 0.5, 0.0), v);
        }
        assert_eq!(scan(0.6, 0.5, 1.0), 1.0);
        assert_eq!(scan(0.4, 0.5, 1.0), 0.0);
        assert!(threshold(0.9, 0.75, 0.5) > 0.99 && threshold(0.0, 0.75, 0.5) < 0.01);
        assert_eq!(Layering::from_index(3).mix(0.5, 0.5, 1.0), 0.25);
        assert_eq!(Layering::from_index(99), Layering::Add);
        assert_eq!(Layering::Overlay.mix(0.5, 0.5, 1.0), 0.5);
        assert_eq!(invert_if(0.2, true), 0.8);
    }
}
