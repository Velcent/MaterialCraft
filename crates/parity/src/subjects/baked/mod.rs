//! Baked mesh maps: our baker ([`materialcraft::bake`]) against the maps Painter baked into the project.
//! One subject per map kind; all of them share one bake per case ([`Inputs::bake`]), so running every
//! kind costs one bake.

mod ao;
mod curvature;
mod id;
mod normal;
mod position;
mod thickness;
mod world_normal;

pub use ao::AmbientOcclusion;
pub use curvature::Curvature;
pub use id::IdMap;
pub use normal::TangentNormal;
pub use position::Position;
pub use thickness::Thickness;
pub use world_normal::WorldNormal;

use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Need};
use materialcraft::tex::Tex;

/// What every baked subject needs.
pub const NEEDS: &[Need] = &[Need::LowMesh, Need::HighMesh];

/// One map of the case's bake, by the baker's name for it.
pub fn baked_map(inputs: &Inputs, params: &Params, name: &str) -> Result<Tex, CaseError> {
    let maps = inputs.bake(params)?;
    maps.get(name)
        .cloned()
        .ok_or_else(|| CaseError::failed(format!("the baker produced no '{name}' map")))
}
