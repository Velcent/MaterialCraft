//! [`Thickness`]: Thickness (ours: distance to the far side along the inverted normal over the AO distance).
//! Compared as stored; Painter's own range normalisation is not rebuilt yet.

use super::{NEEDS, baked_map};
use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, GRAY, Need, ReferenceKind, Subject};
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;

pub struct Thickness;

impl Subject for Thickness {
    fn id(&self) -> &'static str {
        "baked.thickness"
    }

    fn description(&self) -> &'static str {
        "Thickness (ours: distance to the far side along the inverted normal over the AO distance)"
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Baked(BakedKind::Thickness)
    }

    fn needs(&self) -> &'static [Need] {
        NEEDS
    }

    fn components(&self) -> &'static [Component] {
        GRAY
    }

    fn run(&self, inputs: &Inputs, params: &Params) -> Result<Tex, CaseError> {
        baked_map(inputs, params, "thickness")
    }
}
