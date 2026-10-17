//! Mesh map baking: detail of a high mesh written into the UV space of a low mesh's texture set (tangent and world
//! normals, position, ambient occlusion, curvature, thickness, ID).
//!
//! ```text
//! mesh::Mesh + TextureSetRef --LowSurface (mesh::SurfaceMap)--> texels on the low surface
//!        --Samples::project (rays from the cage, HighScene BVH)--> high surface per texel
//!        --one baker per map (normal, world_normal, position, ao, curvature, thickness, id)--> values
//!        --downsample, dilate--> BakedMaps
//! ```
//!
//! [`bake_set`] is the entry point: one texture set (material slots + UV set) of a [`crate::mesh::Mesh`] against a
//! [`HighScene`] built once per high mesh (or from the low mesh itself for "low as high"), with [`BakeSettings`]
//! (map selection, frontal and rear distances, per-baker settings), a [`CancelToken`] and a diagnostics sink.
//! [`bake`] keeps the original OBJ-soup call of the node editor and the command line.
//!
//! Behaviour and defaults are calibrated against the user's own Painter bakes with `mcparity run baked`
//! (MaterialCraft-docs/parity/README.md); nothing here comes from Adobe code or documentation. The baker knows
//! nothing about the paint model: callers store the maps where they belong.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod ao;
mod cancel;
mod context;
mod curvature;
mod error;
mod finder;
mod high;
mod id;
mod low;
mod maps;
mod normal;
mod output;
mod position;
mod project;
mod rays;
mod run;
mod settings;
mod soup;
mod thickness;
mod world_normal;

pub use cancel::CancelToken;
pub use error::BakeError;
pub use finder::{MeshFinder, is_mesh_name};
pub use high::{HighScene, HighTriangle};
pub use low::TextureSetRef;
pub use maps::{BakedMaps, MapKind, MapSelection};
pub use run::bake_set;
pub use settings::{
    AoSettings, Attenuation, BakeSettings, CurvatureRange, CurvatureSettings, PositionBounds, PositionMode, PositionSettings, RayDistribution, TangentSource,
    ThicknessSettings,
};
pub use soup::{Mesh, Tri, parse_obj, uv_sphere_obj};

use crate::diag::NullSink;
use crate::tex::Tex;
use std::collections::BTreeMap;

/// Bakes every selected map of the OBJ soup `low` (all of it one texture set) from `high`, by map name
/// ([`MapKind::name`]).
pub fn bake(low: &Mesh, high: &Mesh, s: &BakeSettings) -> Result<BTreeMap<String, Tex>, String> {
    if low.tris.is_empty() || high.tris.is_empty() {
        return Err("both meshes need faces".into());
    }
    let maps = bake_set(&low.to_mesh(), &TextureSetRef::slot(0), &high.to_high(), s, &CancelToken::new(), &NullSink).map_err(|e| e.to_string())?;
    Ok(maps.into_named())
}

#[cfg(test)]
mod known_answers;
#[cfg(test)]
mod tests;
