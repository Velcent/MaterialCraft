//! MaterialCraft's own mask generators, driven by a texture set's baked mesh maps (ambient occlusion,
//! curvature, world-space normal, position, thickness) and by the layer stack. They plug into the paint
//! model behind [`crate::paint::effect::GeneratorSource`]; [`register`] adds them to an
//! [`EffectRegistry`] under the ids in [`GENERATORS`], and [`builtin`] is a registry holding all of them plus the
//! procedural textures and the filters. Texture inputs are [`crate::proc_textures`] looks.
//!
//! Everything here is written from scratch: the generators stand in for the smart-mask generators Substance
//! Painter projects use (the importer's replacement table maps those onto these ids), shaped after what those
//! procedurals visibly do and calibrated only by measuring results against Painter's saved output on the
//! user's projects (mcparity). Grunge and noise come from MaterialCraft's designer side ([`crate::grunge`],
//! [`crate::noise`]); the height comparison reuses [`crate::ops::height_blend_factor`].
//!
//! Most generators are a [`texel::Recipe`]: a value per texel from the set's maps, computed once for the whole
//! texture and cached ([`cache`]) so every tile and channel pass only copies. Blur and smoothness sliders read
//! the maps through image pyramids ([`pyramid`]).

mod ambient_occlusion;
mod breakup;
mod cache;
mod curvature;
mod dirt;
mod edge_wear;
mod height_combination;
mod maps;
mod mask_builder;
mod mask_editor;
use crate::paint::effect::params;
mod parts;
mod pyramid;
mod texel;
mod tone;
mod uv_border;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_reach;

pub use tone::Layering;

use crate::paint::BakedKind;
use crate::paint::effect::{EffectRegistry, GeneratorSource};
use std::sync::OnceLock;
use texel::Cached;

/// A generator MaterialCraft provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratorInfo {
    /// The id effects name it by.
    pub id: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
}

pub const DIRT: &str = "dirt";
pub const MASK_EDITOR: &str = "mask_editor";
pub const AMBIENT_OCCLUSION: &str = "ambient_occlusion";
pub const METAL_EDGE_WEAR: &str = "metal_edge_wear";
pub const CURVATURE: &str = "curvature";
pub const MASK_BUILDER: &str = "mask_builder";
pub const UV_BORDER_DISTANCE: &str = "uv_border_distance";
pub const HEIGHT_COMBINATION: &str = "height_combination";

/// Every generator, in menu order.
pub const GENERATORS: &[GeneratorInfo] = &[
    GeneratorInfo { id: MASK_EDITOR, label: "Mask editor", summary: "occlusion, curvature, directions, position, thickness and two textures, layered" },
    GeneratorInfo { id: DIRT, label: "Dirt", summary: "dirt in occluded places and cavities, off the edges" },
    GeneratorInfo { id: AMBIENT_OCCLUSION, label: "Ambient occlusion", summary: "the baked occlusion as a mask" },
    GeneratorInfo { id: METAL_EDGE_WEAR, label: "Metal edge wear", summary: "wear on exposed edges" },
    GeneratorInfo { id: CURVATURE, label: "Curvature", summary: "edges and cavities over several radii" },
    GeneratorInfo { id: MASK_BUILDER, label: "Mask builder", summary: "weighted occlusion, curvature, gradient and directions with noise" },
    GeneratorInfo { id: UV_BORDER_DISTANCE, label: "UV border distance", summary: "white along the UV island borders" },
    GeneratorInfo { id: HEIGHT_COMBINATION, label: "Height combination", summary: "the layer where its height rises above the stack below" },
];

/// Masks the layer panel offers as a starting point (id and menu label).
pub const MASK_STARTS: &[(&str, &str)] = &[(HEIGHT_COMBINATION, "Mask with height combination")];

/// The generator registered under `id`.
pub fn generator(id: &str) -> Option<Box<dyn GeneratorSource>> {
    Some(match id {
        DIRT => Box::new(Cached { id: DIRT, recipe: dirt::Dirt }),
        MASK_EDITOR => Box::new(Cached { id: MASK_EDITOR, recipe: mask_editor::MaskEditor }),
        AMBIENT_OCCLUSION => Box::new(Cached { id: AMBIENT_OCCLUSION, recipe: ambient_occlusion::AmbientOcclusion }),
        METAL_EDGE_WEAR => Box::new(Cached { id: METAL_EDGE_WEAR, recipe: edge_wear::EdgeWear }),
        CURVATURE => Box::new(Cached { id: CURVATURE, recipe: curvature::Curvature }),
        MASK_BUILDER => Box::new(Cached { id: MASK_BUILDER, recipe: mask_builder::MaskBuilder }),
        UV_BORDER_DISTANCE => Box::new(uv_border::UvBorder),
        HEIGHT_COMBINATION => Box::new(height_combination::HeightCombination),
        _ => return None,
    })
}

/// The baked maps the generator `id` reads (AS-05: what a smart material or smart mask using it needs baked).
/// Maps read only by optional settings (tri-planar placement of breakup noise reads world normal and position)
/// are not listed. Nothing for unknown ids.
pub fn baked_maps(id: &str) -> Vec<BakedKind> {
    use BakedKind::{AmbientOcclusion, Curvature, Position, Thickness, WorldSpaceNormal};
    match id {
        DIRT | METAL_EDGE_WEAR => vec![AmbientOcclusion, Curvature],
        MASK_EDITOR => vec![AmbientOcclusion, WorldSpaceNormal, Position, Thickness],
        AMBIENT_OCCLUSION => vec![AmbientOcclusion],
        CURVATURE => vec![Curvature],
        MASK_BUILDER => vec![AmbientOcclusion, Curvature, WorldSpaceNormal, Position],
        UV_BORDER_DISTANCE => vec![Position],
        _ => Vec::new(),
    }
}

/// Adds every generator to `registry`.
pub fn register(registry: &mut EffectRegistry) {
    for info in GENERATORS {
        if let Some(g) = generator(info.id) {
            registry.add_generator(info.id, g);
        }
    }
}

/// A registry with every generator, every procedural texture ([`crate::proc_textures`]) and every filter
/// ([`crate::filters`]), built once.
pub fn builtin() -> &'static EffectRegistry {
    static REGISTRY: OnceLock<EffectRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut r = EffectRegistry::new();
        register(&mut r);
        crate::proc_textures::register(&mut r);
        crate::filters::register(&mut r);
        r
    })
}
