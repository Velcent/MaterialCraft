//! `texture.<id>`: MaterialCraft's procedural textures (`materialcraft::proc_textures`) against Painter's
//! texture cache. Cases come the same way as for the generator subjects: channels where every replaceable
//! Substance procedural is replaced and the only one involved is this texture (a mask fill made of it). The
//! textures are look-alikes, so the numbers to read are the histogram and spectrum distances
//! ([`crate::metrics::Distribution`]); pixel MAE says little for noise.

use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, GRAY, Need, ReferenceKind, Subject};
use materialcraft::proc_textures::TEXTURES;
use materialcraft::tex::Tex;
use std::sync::OnceLock;

/// One texture as a subject.
#[derive(Clone, Copy)]
pub struct Texture {
    id: &'static str,
    texture: &'static str,
    description: &'static str,
}

/// One subject per texture of the catalogue (the ids are made once and live for the program).
fn subjects() -> &'static [Texture] {
    static ALL: OnceLock<Vec<Texture>> = OnceLock::new();
    ALL.get_or_init(|| {
        TEXTURES
            .iter()
            .map(|t| Texture {
                id: Box::leak(format!("texture.{}", t.id).into_boxed_str()),
                texture: t.id,
                description: Box::leak(
                    format!(
                        "{} look-alike, mask fills where it is the only replaced procedural",
                        t.label
                    )
                    .into_boxed_str(),
                ),
            })
            .collect()
    })
}

pub fn all() -> Vec<Box<dyn Subject>> {
    subjects().iter().map(|s| Box::new(*s) as Box<dyn Subject>).collect()
}

impl Subject for Texture {
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

    fn components(&self) -> &'static [Component] {
        GRAY
    }

    fn run(&self, _: &Inputs, _: &Params) -> Result<Tex, CaseError> {
        Err(CaseError::skipped(
            "texture subjects are measured through the imported layer stack",
        ))
    }

    fn generator(&self) -> Option<&'static str> {
        Some(self.texture)
    }
}
