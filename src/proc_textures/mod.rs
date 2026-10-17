//! MaterialCraft's own noise, grunge, spot, cell and pattern textures for Paint mode: each a grey *point
//! function* of (u, v), periodic with period 1 in both, so it can be sampled anywhere a projector puts a tap
//! (texture space, UV placement, and the volume projections once they are evaluated).
//!
//! - [`ProcTexture`]: one texture, built from its id and parameter values ([`texture`]); `value(u, v)` is the
//!   grey value, [`ProcTexture::sample_taps`] evaluates projector taps.
//! - [`TEXTURES`]: the catalogue (id, label, [`Family`]) in menu order.
//! - [`TextureSource`]: a texture as a [`GeneratorSource`] of the paint model: each texel of the requested
//!   rectangle is projected through the projection its parameters describe and the taps are evaluated.
//!   [`register`] adds every texture to an [`EffectRegistry`].
//!
//! The looks are written from scratch from MaterialCraft's noise, grunge and pattern functions ([`crate::noise`],
//! [`crate::grunge`]). They stand in for the procedural textures Substance Painter projects use (the importer's
//! replacement table maps those onto these ids) and are look-alikes, never pixel copies: they are judged by
//! histogram and power spectrum against Painter's saved results (`mcparity` subjects `texture.<id>`) and by eye.
//!
//! Common parameters (every texture): `seed`, `scale` (multiplies the look's cell counts; whole numbers keep the
//! period), `disorder` (0..1, a smooth warp), then the tone: `balance` (0.5 neutral, higher brighter), `contrast`
//! (0 neutral, 1 a hard step), `position` / `range` (a second scan around `position`), `invert`. Placement on the
//! texture set (only [`TextureSource`]): `tiling`, `offset_x`, `offset_y`, `rotation` (degrees).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod cells;
mod grunges;
mod look;
mod noises;
mod patterns;
mod source;
mod spots;
#[cfg(test)]
mod tests;
mod tone;

pub use look::{Look, ProcTexture};
pub use source::TextureSource;
pub use tone::Tone;

use crate::paint::effect::{EffectRegistry, Params};

/// What kind of look a texture is (menu grouping).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Noise,
    Grunge,
    Spots,
    Cells,
    Pattern,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::Noise => "Noises",
            Family::Grunge => "Grunges",
            Family::Spots => "Spots",
            Family::Cells => "Cells",
            Family::Pattern => "Patterns",
        }
    }
}

/// One texture of the catalogue.
#[derive(Debug, Clone, Copy)]
pub struct TextureInfo {
    /// The id effects and documents name it by.
    pub id: &'static str,
    pub label: &'static str,
    pub family: Family,
    /// Builds the look (before tone and disorder) from the parameters and the seed.
    build: fn(&Params, u32) -> Box<dyn Look>,
}

const fn t(id: &'static str, label: &'static str, family: Family, build: fn(&Params, u32) -> Box<dyn Look>) -> TextureInfo {
    TextureInfo { id, label, family, build }
}

use Family::{Cells as C, Grunge as G, Noise as N, Pattern as P, Spots as S};

/// Every texture, in menu order.
pub static TEXTURES: &[TextureInfo] = &[
    t("clouds_1", "Clouds soft", N, noises::clouds_1),
    t("clouds_2", "Clouds", N, noises::clouds_2),
    t("clouds_3", "Clouds fine", N, noises::clouds_3),
    t("perlin", "Perlin noise", N, noises::perlin),
    t("fractal_sum", "Fractal sum", N, noises::fractal_sum),
    t("moisture", "Moisture", N, noises::moisture),
    t("anisotropic", "Anisotropic noise", N, noises::anisotropic),
    t("fibers", "Fibers", N, noises::fibers),
    t("white_noise", "White noise", N, noises::white_noise),
    t("grunge_01", "Grunge 1 specks", G, grunges::map_01),
    t("grunge_02", "Grunge 2 drips", G, grunges::map_02),
    t("grunge_03", "Grunge 3 streaks", G, grunges::map_03),
    t("grunge_04", "Grunge 4 holes", G, grunges::map_04),
    t("grunge_05", "Grunge 5 fine streaks", G, grunges::map_05),
    t("grunge_06", "Grunge 6 band", G, grunges::map_06),
    t("grunge_07", "Grunge 7 blotches", G, grunges::map_07),
    t("grunge_08", "Grunge 8 spatter", G, grunges::map_08),
    t("grunge_09", "Grunge 9 grain", G, grunges::map_09),
    t("grunge_10", "Grunge 10 cracks", G, grunges::map_10),
    t("grunge_11", "Grunge 11 crazing", G, grunges::map_11),
    t("grunge_12", "Grunge 12 dots", G, grunges::map_12),
    t("grunge_13", "Grunge 13 fine cracks", G, grunges::map_13),
    t("grunge_14", "Grunge 14 smears", G, grunges::map_14),
    t("grunge_15", "Grunge 15 runs", G, grunges::map_15),
    t("dirt_soft", "Dirt soft", G, grunges::dirt),
    t("rust_fine", "Rust fine", G, grunges::rust_fine),
    t("leaks", "Leaks", G, grunges::leaks),
    t("plaster_cracks", "Plaster cracks", G, grunges::plaster_cracks),
    t("concrete", "Concrete", G, grunges::concrete),
    t("scratches", "Scratches", G, grunges::scratches),
    t("grunge_spots", "Grunge spots", G, grunges::spots),
    t("smudges", "Smudges", G, grunges::smudges),
    t("spots_1", "Spots round", S, spots::bnw_1),
    t("spots_2", "Spots large", S, spots::bnw_2),
    t("spots_3", "Spots grainy", S, spots::bnw_3),
    t("gaussian_spots_1", "Soft spots", S, spots::gaussian_1),
    t("gaussian_spots_2", "Soft spots dense", S, spots::gaussian_2),
    t("cells_1", "Cells soft", C, cells::cells_1),
    t("cells_2", "Cell borders", C, cells::cells_2),
    t("cells_3", "Cells hard", C, cells::cells_3),
    t("cells_4", "Cells flat", C, cells::cells_4),
    t("crystal_1", "Crystal", C, cells::crystal_1),
    t("crystal_2", "Crystal facets", C, cells::crystal_2),
    t("voronoi_fractal", "Voronoi fractal", C, cells::voronoi_fractal),
    t("stripes", "Stripes", P, patterns::stripes),
    t("checker", "Checker", P, patterns::checker),
    t("weave", "Weave", P, patterns::weave),
    t("weave_gaps", "Weave with gaps", P, patterns::weave_gaps),
    t("triangle_grid", "Triangle grid", P, patterns::triangle_grid),
    t("triangles", "Triangles", P, patterns::triangles),
    t("starburst", "Starburst", P, patterns::starburst),
];

/// The catalogue entry of `id`.
pub fn info(id: &str) -> Option<&'static TextureInfo> {
    TEXTURES.iter().find(|t| t.id == id)
}

/// Texture `id` with `params` (name → value text, as documents store them).
pub fn texture(id: &str, params: &[(String, String)]) -> Option<ProcTexture> {
    let info = info(id)?;
    let p = Params::new(params);
    Some(ProcTexture::new((info.build)(&p, p.seed("seed")), &p))
}

/// Grunge map `map` (1..=15) with its default settings and no tone: what the generators' texture inputs use
/// when they name a map by number.
pub fn grunge_look(map: u32, seed: u32) -> ProcTexture {
    ProcTexture::plain(Box::new(grunges::GrungeMap::new(map, seed, 0.5, 0.5, 0.0, 1.0, None)))
}

/// Adds every texture to `registry` as a generator under its id.
pub fn register(registry: &mut EffectRegistry) {
    for info in TEXTURES {
        registry.add_generator(info.id, Box::new(TextureSource::new(info.id)));
    }
}

/// The multiple of a look's cell count the `scale` parameter asks for (a whole number ≥ 1).
fn scale(p: &Params) -> i32 {
    p.int("scale", 1).clamp(1, 256)
}

/// `base` cells times the scale, within what the noise functions take.
fn cells(p: &Params, base: i32) -> i32 {
    base.saturating_mul(scale(p)).clamp(1, 4096)
}
