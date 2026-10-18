//! MaterialCraft's own filter effects for Paint mode: colour and tone adjustments, blur, sharpen, bevel, mask
//! outline, warp, slope blur and a roughness finish. They plug into the paint model behind
//! [`crate::paint::effect::FilterKernel`]; [`register`] adds them to an [`EffectRegistry`] under the ids in
//! [`FILTERS`] ([`crate::generators::builtin`] holds them with the generators and textures).
//!
//! Filters that read neighbours never mix UV islands: they run per island on tiles padded from the texture
//! set's surface map ([`islands`]), so a blur or bevel stops at an island's border instead of pulling in the
//! texels of whatever island lies next to it in UV space. Per-texel filters ([`kernel::PointFilter`]) need no
//! padding.
//!
//! Everything here is written from scratch. The filters stand in for the filters Substance Painter projects use
//! (the importer's replacement table maps those onto these ids, with their parameters), shaped after what
//! those filters visibly do and calibrated only by measuring results against Painter's saved output on the
//! user's projects (`mcparity filter.<id>`). Kernels come from MaterialCraft's designer side where it has them
//! (`ops::distance_field`, `ops::rgb_to_lch`); the gaussian is our own non-wrapping version for padded tiles.
//! Lengths are in UV units of the texture's larger side, so a filter looks the same at every resolution.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod blur;
mod colour;
mod displace;
mod finish;
mod gauss;
mod islands;
pub mod kernel;
mod pad;
pub mod plane;
mod shape;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_reach;
mod tonal;

use crate::paint::effect::{EffectRegistry, FilterKernel};
use kernel::{Area, Point};

/// A filter MaterialCraft provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterInfo {
    /// The id effects name it by.
    pub id: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    /// True if it reads neighbours (and so runs per UV island).
    pub area: bool,
}

pub const HSL: &str = "hsl";
pub const BLUR: &str = "blur";
pub const SHARPEN: &str = "sharpen";
pub const INVERT: &str = "invert";
pub const HEIGHT_ADJUST: &str = "height_adjust";
pub const BEVEL: &str = "bevel";
pub const MASK_OUTLINE: &str = "mask_outline";
pub const SLOPE_BLUR: &str = "slope_blur";
pub const WARP: &str = "warp";
pub const FINISH_ROUGH: &str = "finish_rough";
pub const CLAMP: &str = "clamp";
pub const HISTOGRAM_SCAN: &str = "histogram_scan";
pub const HISTOGRAM_SHIFT: &str = "histogram_shift";
pub const CONTRAST_LUMINOSITY: &str = "contrast_luminosity";

const fn f(id: &'static str, label: &'static str, summary: &'static str, area: bool) -> FilterInfo {
    FilterInfo { id, label, summary, area }
}

/// Every filter, in menu order.
pub const FILTERS: &[FilterInfo] = &[
    f(HSL, "HSL", "hue, saturation and lightness, perceptual", false),
    f(BLUR, "Blur", "gaussian blur within each UV island", true),
    f(SHARPEN, "Sharpen", "unsharp masking within each UV island", true),
    f(INVERT, "Invert", "one minus the value", false),
    f(HEIGHT_ADJUST, "Height adjust", "scale, offset or invert height", false),
    f(BEVEL, "Bevel", "a ramp along the mask's edge, inwards or outwards", true),
    f(MASK_OUTLINE, "Mask outline", "a band along the mask's edge", true),
    f(SLOPE_BLUR, "Slope blur", "smears along the slope of a noise", true),
    f(WARP, "Warp", "displaces by a noise", true),
    f(FINISH_ROUGH, "Finish rough", "grunge in the roughness", false),
    f(CLAMP, "Clamp", "limits values to a range", false),
    f(HISTOGRAM_SCAN, "Histogram scan", "a contrast ramp at a chosen level", false),
    f(HISTOGRAM_SHIFT, "Histogram shift", "shifts values, wrapping around", false),
    f(CONTRAST_LUMINOSITY, "Contrast / luminosity", "contrast around mid grey and brightness", false),
];

/// The filter registered under `id`.
pub fn filter(id: &str) -> Option<Box<dyn FilterKernel>> {
    Some(match id {
        HSL => Box::new(Point(colour::Hsl)),
        BLUR => Box::new(Area(blur::Blur)),
        SHARPEN => Box::new(Area(blur::Sharpen)),
        INVERT => Box::new(Point(tonal::Invert)),
        HEIGHT_ADJUST => Box::new(Point(tonal::HeightAdjust)),
        BEVEL => Box::new(Area(shape::Bevel)),
        MASK_OUTLINE => Box::new(Area(shape::MaskOutline)),
        SLOPE_BLUR => Box::new(Area(displace::SlopeBlur)),
        WARP => Box::new(Area(displace::Warp)),
        FINISH_ROUGH => Box::new(Point(finish::FinishRough)),
        CLAMP => Box::new(Point(tonal::Clamp)),
        HISTOGRAM_SCAN => Box::new(Point(tonal::HistogramScan)),
        HISTOGRAM_SHIFT => Box::new(Point(tonal::HistogramShift)),
        CONTRAST_LUMINOSITY => Box::new(Point(tonal::ContrastLuminosity)),
        _ => return None,
    })
}

/// Adds every filter to `registry`.
pub fn register(registry: &mut EffectRegistry) {
    for info in FILTERS {
        if let Some(k) = filter(info.id) {
            registry.add_filter(info.id, k);
        }
    }
}
