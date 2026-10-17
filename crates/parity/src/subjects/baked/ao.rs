//! [`AmbientOcclusion`]: Ambient occlusion from the high-poly mesh (our hemisphere rays against Painter's AO baker).
//! Both are 0 (occluded) .. 1 (open); compared as stored.

use super::{baked_map, NEEDS};
use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, Need, ReferenceKind, Subject, GRAY};
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;

pub struct AmbientOcclusion;

impl Subject for AmbientOcclusion {
    fn id(&self) -> &'static str {
        "baked.ao"
    }

    fn description(&self) -> &'static str {
        "Ambient occlusion from the high-poly mesh (our hemisphere rays against Painter's AO baker)"
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Baked(BakedKind::AmbientOcclusion)
    }

    fn needs(&self) -> &'static [Need] {
        NEEDS
    }

    fn components(&self) -> &'static [Component] {
        GRAY
    }

    fn run(&self, inputs: &Inputs, params: &Params) -> Result<Tex, CaseError> {
        baked_map(inputs, params, "ambient_occlusion")
    }
}
