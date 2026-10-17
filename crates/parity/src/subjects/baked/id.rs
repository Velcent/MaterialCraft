//! [`IdMap`]: Colour/ID map (ours: one random colour per OBJ group or mesh part).
//! Painter colours by its own source (vertex colour, material, ...); ours picks random colours per group,
//! so only the region layout is comparable. Kept to show the gap.

use super::{NEEDS, baked_map};
use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, Need, RGB, ReferenceKind, Subject};
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;

pub struct IdMap;

impl Subject for IdMap {
    fn id(&self) -> &'static str {
        "baked.id"
    }

    fn description(&self) -> &'static str {
        "Colour/ID map (ours: one random colour per OBJ group or mesh part)"
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Baked(BakedKind::Id)
    }

    fn needs(&self) -> &'static [Need] {
        NEEDS
    }

    fn components(&self) -> &'static [Component] {
        RGB
    }

    fn run(&self, inputs: &Inputs, params: &Params) -> Result<Tex, CaseError> {
        baked_map(inputs, params, "id")
    }
}
