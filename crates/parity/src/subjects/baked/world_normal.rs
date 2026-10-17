//! [`WorldNormal`]: World-space normal map (in Painter's normalised mesh space).
//! Both encode `0.5 + 0.5 n`; compared as stored.

use super::{NEEDS, baked_map};
use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, Need, ReferenceKind, Subject, XYZ};
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;

pub struct WorldNormal;

impl Subject for WorldNormal {
    fn id(&self) -> &'static str {
        "baked.world_normal"
    }

    fn description(&self) -> &'static str {
        "World-space normal map (in Painter's normalised mesh space)"
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Baked(BakedKind::WorldSpaceNormal)
    }

    fn needs(&self) -> &'static [Need] {
        NEEDS
    }

    fn components(&self) -> &'static [Component] {
        XYZ
    }

    fn run(&self, inputs: &Inputs, params: &Params) -> Result<Tex, CaseError> {
        baked_map(inputs, params, "world_normal")
    }
}
