//! `filter.<id>`: MaterialCraft's filters (`materialcraft::filters`) against Painter's texture cache. Cases
//! come the same way as for the generator subjects: channels where every replaceable Substance procedural is
//! replaced and the only one involved is this filter (a mask filter over content the import reproduces
//! exactly); the compared images are the final looks of the channel.

use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, GRAY, Need, ReferenceKind, Subject};
use materialcraft::filters::FILTERS;
use materialcraft::tex::Tex;
use std::sync::OnceLock;

/// One filter as a subject.
#[derive(Clone, Copy)]
pub struct Filter {
    id: &'static str,
    filter: &'static str,
    description: &'static str,
}

/// One subject per filter (the ids are made once and live for the program).
fn subjects() -> &'static [Filter] {
    static ALL: OnceLock<Vec<Filter>> = OnceLock::new();
    ALL.get_or_init(|| {
        FILTERS
            .iter()
            .map(|f| Filter {
                id: Box::leak(format!("filter.{}", f.id).into_boxed_str()),
                filter: f.id,
                description: Box::leak(
                    format!(
                        "{} replacement, channels where it is the only replaced procedural",
                        f.label
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

impl Subject for Filter {
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
            "filter subjects are measured through the imported layer stack",
        ))
    }

    /// The id the importer's replacement reasons name (`… replaced by <id>`), as for generators.
    fn generator(&self) -> Option<&'static str> {
        Some(self.filter)
    }
}
