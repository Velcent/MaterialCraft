//! The filter entries of the replacement table: Substance filters MaterialCraft replaces with its own filters
//! ([`materialcraft::filters`]), with the translation of their parameters into ours (see that module for what
//! each of our parameters does). Parameters without a counterpart are left out and named in the entry's note.
//!
//! Painter's filters state sizes in their own units; ours are UV units. The factors below turn one into the
//! other. They are first guesses from the values the user's projects use, to be calibrated by black-box
//! comparison with Painter's results (`mcparity run filter.<id>`) and by eye.

use super::Replacement;
use super::params::{Ours, Painter};
use materialcraft::filters as f;

/// Painter's blur `intensity` → our blur radius (sigma, UV units).
const BLUR_UV: f32 = 0.004;
/// Painter's bevel `distance` → ours (UV units).
const BEVEL_UV: f32 = 0.25;
/// Painter's bevel `smoothing` → our smoothing (sigma, UV units).
const BEVEL_SMOOTHING_UV: f32 = 0.002;
/// Painter's mask outline `blur` → our blur (sigma, UV units).
const OUTLINE_BLUR_UV: f32 = 0.02;
/// Painter's slope blur length (intensity scaled by its divider) → ours (UV units).
const SLOPE_UV: f32 = 0.1;

const CALIBRATION: &str = "sizes converted by first-guess factors, calibration pending";

pub static FILTERS: &[Replacement] = &[
    Replacement {
        painter: &["hsl_perceptive"],
        label: "HSL Perceptive → MaterialCraft HSL",
        generator: f::HSL,
        note: "hue, saturation and lightness in OkLCh; Painter's mode switch is left out",
        translate: hsl,
    },
    Replacement {
        painter: &["sharpen"],
        label: "Sharpen → MaterialCraft sharpen",
        generator: f::SHARPEN,
        note: "unsharp masking of a fixed small radius; per UV island",
        translate: sharpen,
    },
    Replacement {
        painter: &["blur", "Blur", "fx_blur"],
        label: "Blur → MaterialCraft blur",
        generator: f::BLUR,
        note: "gaussian per UV island; the tiling switch is left out; sizes converted by a first-guess factor",
        translate: blur,
    },
    Replacement {
        painter: &["invert"],
        label: "Invert → MaterialCraft invert",
        generator: f::INVERT,
        note: "exact for masks",
        translate: invert,
    },
    Replacement {
        painter: &["bevel"],
        label: "Bevel → MaterialCraft bevel",
        generator: f::BEVEL,
        note: "edge ramp from the distance to the mask's 0.5 level; non-uniform blur and the mode are left out",
        translate: bevel,
    },
    Replacement {
        painter: &["height_adjust"],
        label: "Height Adjust → MaterialCraft height adjust",
        generator: f::HEIGHT_ADJUST,
        note: "invert, multiply, offset",
        translate: height_adjust,
    },
    Replacement {
        painter: &["mask_outline"],
        label: "Mask Outline → MaterialCraft mask outline",
        generator: f::MASK_OUTLINE,
        note: "band along the mask's 0.5 level; the switch and input threshold are left out",
        translate: mask_outline,
    },
    Replacement {
        painter: &["blur_slope"],
        label: "Blur Slope → MaterialCraft slope blur",
        generator: f::SLOPE_BLUR,
        note: "slope from MaterialCraft's own noise (a custom noise input is left out); sizes converted by a first-guess factor",
        translate: slope_blur,
    },
    Replacement {
        painter: &["warp", "Warp"],
        label: "Warp → MaterialCraft warp",
        generator: f::WARP,
        note: "displacement by MaterialCraft's own noise (a custom noise input, noise blur and contrast are left out)",
        translate: warp,
    },
    Replacement {
        painter: &["finish_rough"],
        label: "Finish Rough → MaterialCraft finish rough",
        generator: f::FINISH_ROUGH,
        note: "MaterialCraft grunge in the roughness; tri-planar placement and the switch are left out",
        translate: finish_rough,
    },
    Replacement {
        painter: &["clamp"],
        label: "Clamp → MaterialCraft clamp",
        generator: f::CLAMP,
        note: "exact",
        translate: clamp,
    },
    Replacement {
        painter: &["histogram_scan"],
        label: "Histogram Scan → MaterialCraft histogram scan",
        generator: f::HISTOGRAM_SCAN,
        note: CALIBRATION,
        translate: histogram_scan,
    },
    Replacement {
        painter: &["histogram_shift"],
        label: "Histogram Shift → MaterialCraft histogram shift",
        generator: f::HISTOGRAM_SHIFT,
        note: "wraps around 1",
        translate: histogram_shift,
    },
    Replacement {
        painter: &["contrast_luminosity"],
        label: "Contrast Luminosity → MaterialCraft contrast / luminosity",
        generator: f::CONTRAST_LUMINOSITY,
        note: CALIBRATION,
        translate: contrast_luminosity,
    },
];

/// A Painter value mapped by `m`, if it is there.
fn mapped(p: &Painter, name: &str, m: impl Fn(f32) -> f32) -> Option<f32> {
    p.f(name).map(m)
}

/// Painter's 0..1 sliders centred on 0.5 as ours, centred on 0.
fn centred(p: &Painter, name: &str, range: f32) -> Option<f32> {
    mapped(p, name, |v| (v - 0.5) * range)
}

/// An intensity with its divider switch (0..3, a power of ten).
fn divided(p: &Painter) -> Option<f32> {
    let divider = p.f("intensity_divider").unwrap_or(0.0).clamp(0.0, 6.0);
    mapped(p, "intensity", |v| v / 10f32.powf(divider.round()))
}

fn hsl(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .num("hue", centred(p, "hue", 1.0))
        .num("saturation", centred(p, "saturation", 2.0))
        .num("lightness", centred(p, "luminosity", 2.0))
        .done()
}

fn sharpen(p: &Painter) -> Vec<(String, String)> {
    Ours::default().num("amount", p.f("intensity")).done()
}

fn blur(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .num("radius", mapped(p, "intensity", |v| v * BLUR_UV))
        .done()
}

fn invert(p: &Painter) -> Vec<(String, String)> {
    Ours::default().nums(p, &[("invert", "invert")]).done()
}

fn bevel(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .num("distance", mapped(p, "distance", |v| v * BEVEL_UV))
        .num("smoothing", mapped(p, "smoothing", |v| v * BEVEL_SMOOTHING_UV))
        .done()
}

fn height_adjust(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .nums(
            p,
            &[
                ("offset", "offset"),
                ("multiplier", "height_multiplier"),
                ("invert", "invert"),
            ],
        )
        .done()
}

fn mask_outline(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .nums(p, &[("width", "width"), ("curve", "curve")])
        // Painter numbers the positions from 1
        .num("position", mapped(p, "mask_position", |v| v - 1.0))
        .num("blur", mapped(p, "blur", |v| v * OUTLINE_BLUR_UV))
        .done()
}

fn slope_blur(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .num("intensity", divided(p).map(|v| v * SLOPE_UV))
        .num("mode", mapped(p, "mode_blend", |v| v - 1.0))
        .num("samples", mapped(p, "quality", |q| (4.0 + q * 28.0).round()))
        .nums(p, &[("scale", "noise_scale"), ("seed", "$randomseed")])
        .done()
}

fn warp(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .num("intensity", divided(p))
        .nums(
            p,
            &[
                ("directional", "warp_type"),
                ("angle", "warpangle"),
                ("scale", "noise_scale"),
                ("seed", "$randomseed"),
            ],
        )
        .done()
}

fn finish_rough(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .nums(
            p,
            &[
                ("intensity", "intensity"),
                ("tile", "tile"),
                ("grunge", "tile_grunge"),
                ("seed", "$randomseed"),
            ],
        )
        .done()
}

fn clamp(p: &Painter) -> Vec<(String, String)> {
    Ours::default().nums(p, &[("min", "min"), ("max", "max")]).done()
}

fn histogram_scan(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .nums(
            p,
            &[
                ("position", "Position"),
                ("contrast", "Contrast"),
                ("invert_position", "Invert_Position"),
            ],
        )
        .done()
}

fn histogram_shift(p: &Painter) -> Vec<(String, String)> {
    Ours::default().nums(p, &[("position", "position")]).done()
}

fn contrast_luminosity(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .nums(p, &[("contrast", "Contrast"), ("luminosity", "Luminosity")])
        .done()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn centred_sliders_and_divided_intensities() {
        let v = params(&[("hue", "0.5"), ("saturation", "0"), ("luminosity", "0.75")]);
        let o = hsl(&Painter::new(&v));
        assert_eq!(o, params(&[("hue", "0"), ("saturation", "-1"), ("lightness", "0.5")]));
        let v = params(&[("intensity", "32"), ("intensity_divider", "3"), ("warp_type", "1")]);
        let o = warp(&Painter::new(&v));
        assert_eq!(o, params(&[("intensity", "0.032"), ("directional", "1")]));
        let v = params(&[("mask_position", "2"), ("width", "0.05")]);
        assert_eq!(
            mask_outline(&Painter::new(&v)),
            params(&[("width", "0.05"), ("position", "1")])
        );
    }
}
