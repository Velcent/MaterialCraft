//! [`Curvature`]: Curvature (ours: from the baked normal map's derivatives; Painter: from mesh samples).
//! Both use 0.5 for flat and brighter for convex; compared as stored.

use super::{NEEDS, baked_map};
use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, GRAY, Need, ReferenceKind, Subject};
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;

pub struct Curvature;

impl Subject for Curvature {
    fn id(&self) -> &'static str {
        "baked.curvature"
    }

    fn description(&self) -> &'static str {
        "Curvature (ours: from the baked normal map's derivatives; Painter: from mesh samples)"
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Baked(BakedKind::Curvature)
    }

    fn needs(&self) -> &'static [Need] {
        NEEDS
    }

    fn components(&self) -> &'static [Component] {
        GRAY
    }

    fn run(&self, inputs: &Inputs, params: &Params) -> Result<Tex, CaseError> {
        baked_map(inputs, params, "curvature")
    }
}
