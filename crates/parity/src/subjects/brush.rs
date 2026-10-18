//! `brush.replay`: the importer's replay of Painter's brush strokes (`materialcraft_brush`, roadmap step 32)
//! against Painter's texture cache. Its cases are isolated ones (`materialcraft_substance::tools::verify_layers::
//! isolate`): brush work in a mask (a paint effect, or an effect group of brush work) is the candidate, Painter's
//! mask right after it read back from the cache through the rest of the stack, ours the same way with the strokes
//! replayed. Painter's random sequence differs from ours, so jittered strokes are close, not identical.

use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, GRAY, Need, ReferenceKind, Subject};
use materialcraft::tex::Tex;
use materialcraft_substance::tools::verify_layers::isolate::slots::STROKES_ID;

pub struct Replay;

impl Subject for Replay {
    fn id(&self) -> &'static str {
        "brush.replay"
    }

    fn description(&self) -> &'static str {
        "Replayed brush strokes: brush work in masks, read back from the texture cache"
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Cache
    }

    fn needs(&self) -> &'static [Need] {
        &[]
    }

    fn components(&self) -> &'static [Component] {
        GRAY
    }

    fn run(&self, _: &Inputs, _: &Params) -> Result<Tex, CaseError> {
        Err(CaseError::skipped(
            "brush.replay is measured through the imported layer stack",
        ))
    }

    /// The id isolated stroke candidates carry, so the isolated runner hands them to this subject.
    fn generator(&self) -> Option<&'static str> {
        Some(STROKES_ID)
    }
}
