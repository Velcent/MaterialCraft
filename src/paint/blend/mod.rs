//! Blend modes: how a layer's value combines with the stack below, per channel.
//!
//! The list follows Substance Painter's blending modes. All formulas work per component on
//! working-space values (stored values, or linear light for sRGB channels; see
//! [`crate::paint::working_space`]), `b` = below, `t` = top, and do not clamp unless stated, so float
//! channels such as height may leave 0..1. Normal-map modes decode the vectors first (see
//! [`crate::paint::normal`]).

use super::normal;

/// One blend mode. See [`BlendMode::apply`] for the formulas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendMode {
    /// `t`
    Normal,
    /// The layer replaces the stack below, coverage included: `lerp(below, top, opacity)` on all four components.
    Replace,
    /// The layer does not affect the channel.
    Disable,
    /// `b * t`
    Multiply,
    /// `b / t` (`t = 0` gives 1 where `b > 0`, else 0)
    Divide,
    /// `t / b`
    InverseDivide,
    /// `min(b, t)`
    Darken,
    /// `max(b, t)`
    Lighten,
    /// Linear dodge: `b + t`
    Add,
    /// `b - t`
    Subtract,
    /// `|b - t|`
    Difference,
    /// `b + t - 2bt`
    Exclusion,
    /// Painter's "signed addition": `b + 2t - 1` (above 0.5 adds, below subtracts), unclamped.
    SignedAdd,
    /// `1 - (1 - b)(1 - t)`
    Screen,
    /// `b < 0.5 ? 2bt : 1 - 2(1-b)(1-t)`
    Overlay,
    /// Overlay with `b` and `t` swapped.
    HardLight,
    /// W3C compositing spec soft light.
    SoftLight,
    /// `b / (1 - t)`, clamped to 1.
    ColorDodge,
    /// `1 - (1 - b) / t`, clamped to 0.
    ColorBurn,
    /// `max(0, b + t - 1)`
    LinearBurn,
    /// Colour burn with `2t` below 0.5, colour dodge with `2t - 1` above.
    VividLight,
    /// `clamp(b + 2t - 1, 0, 1)`
    LinearLight,
    /// `t < 0.5 ? min(b, 2t) : max(b, 2t - 1)`
    PinLight,
    /// W3C non-separable modes (Hue, Saturation, Color, Luminosity), on the RGB components.
    Hue,
    Saturation,
    Color,
    /// Luminosity of the top over hue and saturation of the base (W3C "luminosity").
    Value,
    /// Whiteout: `normalize(b.xy + t.xy, b.z * t.z)`.
    NormalMapCombine,
    /// Reoriented normal mapping with the top as detail on the base.
    NormalMapDetail,
    /// Reoriented normal mapping with the base as detail on the top.
    NormalMapInverseDetail,
    /// Groups: children blend straight into the stack below. A layer that is not a group becomes an effect layer:
    /// its content and effects start from the composite below and replace it by the layer's strength. As a
    /// plain formula (inside a content stack) it acts as `Normal`.
    Passthrough,
}

impl BlendMode {
    pub const ALL: [BlendMode; 31] = [
        BlendMode::Normal,
        BlendMode::Replace,
        BlendMode::Disable,
        BlendMode::Multiply,
        BlendMode::Divide,
        BlendMode::InverseDivide,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Add,
        BlendMode::Subtract,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::SignedAdd,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::LinearBurn,
        BlendMode::VividLight,
        BlendMode::LinearLight,
        BlendMode::PinLight,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Value,
        BlendMode::NormalMapCombine,
        BlendMode::NormalMapDetail,
        BlendMode::NormalMapInverseDetail,
        BlendMode::Passthrough,
    ];

    /// Stable id used in files, e.g. `normal`, `linear_burn`, `normal_map_detail`.
    pub fn id(self) -> &'static str {
        use BlendMode::*;
        match self {
            Normal => "normal",
            Replace => "replace",
            Disable => "disable",
            Multiply => "multiply",
            Divide => "divide",
            InverseDivide => "inverse_divide",
            Darken => "darken",
            Lighten => "lighten",
            Add => "add",
            Subtract => "subtract",
            Difference => "difference",
            Exclusion => "exclusion",
            SignedAdd => "signed_add",
            Screen => "screen",
            Overlay => "overlay",
            HardLight => "hard_light",
            SoftLight => "soft_light",
            ColorDodge => "color_dodge",
            ColorBurn => "color_burn",
            LinearBurn => "linear_burn",
            VividLight => "vivid_light",
            LinearLight => "linear_light",
            PinLight => "pin_light",
            Hue => "hue",
            Saturation => "saturation",
            Color => "color",
            Value => "value",
            NormalMapCombine => "normal_map_combine",
            NormalMapDetail => "normal_map_detail",
            NormalMapInverseDetail => "normal_map_inverse_detail",
            Passthrough => "passthrough",
        }
    }

    pub fn from_id(id: &str) -> Option<BlendMode> {
        BlendMode::ALL.into_iter().find(|m| m.id() == id)
    }

    pub fn is_normal_map(self) -> bool {
        matches!(self, BlendMode::NormalMapCombine | BlendMode::NormalMapDetail | BlendMode::NormalMapInverseDetail)
    }

    /// The blended value at full strength. RGB follow the formula; alpha is the top's alpha.
    pub fn apply(self, base: [f32; 4], top: [f32; 4]) -> [f32; 4] {
        use BlendMode::*;
        let rgb = match self {
            Normal | Replace | Passthrough => [top[0], top[1], top[2]],
            Disable => [base[0], base[1], base[2]],
            Hue | Saturation | Color | Value => nonseparable::apply(self, rgb3(base), rgb3(top)),
            NormalMapCombine | NormalMapDetail | NormalMapInverseDetail => {
                let (b, t) = (normal::decode(base), normal::decode(top));
                let n = match self {
                    NormalMapCombine => normal::whiteout(b, t),
                    NormalMapDetail => normal::reoriented(b, t),
                    _ => normal::reoriented(t, b),
                };
                rgb3(normal::encode(n))
            }
            _ => [0, 1, 2].map(|i| separable(self, base[i], top[i])),
        };
        [rgb[0], rgb[1], rgb[2], top[3]]
    }

    /// Blends `top` over `base` with strength `opacity × top alpha` (top alpha = coverage).
    ///
    /// `base[3]` is the coverage accumulated so far and becomes `base.a + w (1 - base.a)`; RGB is
    /// `lerp(base, apply(base, top), w)`, renormalised for normal-map modes. `Replace` lerps all four
    /// components by `opacity` alone; `Disable` returns `base`.
    pub fn mix(self, base: [f32; 4], top: [f32; 4], opacity: f32) -> [f32; 4] {
        let opacity = if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 0.0 };
        match self {
            BlendMode::Disable => base,
            BlendMode::Replace => [0, 1, 2, 3].map(|i| lerp(base[i], top[i], opacity)),
            _ => {
                let cov = if top[3].is_finite() { top[3].clamp(0.0, 1.0) } else { 0.0 };
                let w = opacity * cov;
                if w <= 0.0 {
                    return base;
                }
                let blended = self.apply(base, top);
                let mut out = [0, 1, 2].map(|i| lerp(base[i], blended[i], w));
                if self.is_normal_map() {
                    out = rgb3(normal::encode(normal::decode([out[0], out[1], out[2], 1.0])));
                }
                [out[0], out[1], out[2], base[3] + w * (1.0 - base[3])]
            }
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn rgb3(p: [f32; 4]) -> [f32; 3] {
    [p[0], p[1], p[2]]
}

mod formulas;

use formulas::{nonseparable, separable};

#[cfg(test)]
mod tests;
