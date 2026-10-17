//! [`Position`]: Position map (ours: normalised to the high mesh's bounding box per axis).
//! Both encode position in 0..1 over a bounding box; Painter's mode, axis and normalisation settings are
//! listed as unmapped.

use super::{NEEDS, baked_map};
use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, Need, ReferenceKind, Subject, XYZ};
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;

pub struct Position;

impl Subject for Position {
    fn id(&self) -> &'static str {
        "baked.position"
    }

    fn description(&self) -> &'static str {
        "Position map (ours: normalised to the high mesh's bounding box per axis)"
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Baked(BakedKind::Position)
    }

    fn needs(&self) -> &'static [Need] {
        NEEDS
    }

    fn components(&self) -> &'static [Component] {
        XYZ
    }

    fn run(&self, inputs: &Inputs, params: &Params) -> Result<Tex, CaseError> {
        baked_map(inputs, params, "position")
    }
}
