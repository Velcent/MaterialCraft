//! [`TangentNormal`]: Tangent-space normal map from the high-poly mesh.
//! Ours is OpenGL (+Y up); the reference is converted to OpenGL when the project stores DirectX bakes.

use super::{baked_map, NEEDS};
use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, Need, ReferenceKind, Subject, XYZ};
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;

pub struct TangentNormal;

impl Subject for TangentNormal {
    fn id(&self) -> &'static str {
        "baked.normal"
    }

    fn description(&self) -> &'static str {
        "Tangent-space normal map from the high-poly mesh"
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Baked(BakedKind::Normal)
    }

    fn needs(&self) -> &'static [Need] {
        NEEDS
    }

    fn components(&self) -> &'static [Component] {
        XYZ
    }

    fn run(&self, inputs: &Inputs, params: &Params) -> Result<Tex, CaseError> {
        baked_map(inputs, params, "normal")
    }
}
