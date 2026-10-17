//! `generator.<id>`: MaterialCraft's replacement generators (`materialcraft::generators`) against Painter's
//! texture cache. Cases are the channels of sets where every replaceable Substance procedural is replaced and
//! the only one involved is this generator (see `materialcraft_substance::tools::verify_layers::generator_cases`);
//! the compared images are the final looks of the channel (ours over Painter's defaults vs the cache).

use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, GRAY, Need, ReferenceKind, Subject};
use materialcraft::generators as g;
use materialcraft::tex::Tex;

/// One generator as a subject.
#[derive(Clone, Copy)]
pub struct Generator {
    id: &'static str,
    generator: &'static str,
    description: &'static str,
}

const SUBJECTS: [Generator; 7] = [
    Generator {
        id: "generator.mask_editor",
        generator: g::MASK_EDITOR,
        description: "Mask editor replacement, channels where it is the only replaced procedural",
    },
    Generator {
        id: "generator.dirt",
        generator: g::DIRT,
        description: "Dirt replacement, channels where it is the only replaced procedural",
    },
    Generator {
        id: "generator.ambient_occlusion",
        generator: g::AMBIENT_OCCLUSION,
        description: "Ambient occlusion replacement, channels where it is the only replaced procedural",
    },
    Generator {
        id: "generator.metal_edge_wear",
        generator: g::METAL_EDGE_WEAR,
        description: "Metal edge wear replacement, channels where it is the only replaced procedural",
    },
    Generator {
        id: "generator.curvature",
        generator: g::CURVATURE,
        description: "Curvature replacement, channels where it is the only replaced procedural",
    },
    Generator {
        id: "generator.mask_builder",
        generator: g::MASK_BUILDER,
        description: "Mask builder replacement, channels where it is the only replaced procedural",
    },
    Generator {
        id: "generator.uv_border_distance",
        generator: g::UV_BORDER_DISTANCE,
        description: "UV border distance replacement, channels where it is the only replaced procedural",
    },
];

pub fn all() -> Vec<Box<dyn Subject>> {
    SUBJECTS.iter().map(|s| Box::new(*s) as Box<dyn Subject>).collect()
}

impl Subject for Generator {
    fn id(&self) -> &'static str {
        self.id
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Cache
    }

    fn needs(&self) -> &'static [Need] {
        &[]
    }

    /// Grey channels; colour channels compare RGB (chosen per case).
    fn components(&self) -> &'static [Component] {
        GRAY
    }

    fn run(&self, _: &Inputs, _: &Params) -> Result<Tex, CaseError> {
        Err(CaseError::skipped(
            "generator subjects are measured through the imported layer stack",
        ))
    }

    fn generator(&self) -> Option<&'static str> {
        Some(self.generator)
    }
}
