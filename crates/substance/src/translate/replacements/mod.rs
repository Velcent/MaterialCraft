//! The replacement table: Substance procedurals MaterialCraft can stand in for with its own generators
//! ([`materialcraft::generators`]), procedural textures ([`materialcraft::proc_textures`], [`TEXTURES`]) and
//! filters ([`materialcraft::filters`], [`FILTERS`]), one [`Replacement`] per entry, with the translation of
//! Painter's parameter values into ours. This is the only place that knows Painter's procedural and parameter
//! names.
//!
//! A replaced effect becomes our generator with the Painter procedural kept as its
//! [`Procedural::original`], so a later, better entry here can convert the document again
//! ([`reconvert`]). Which procedurals get replaced is the user's [`ProceduralPlan`].
//!
//! Our generators are written from scratch from the visible behaviour of these procedurals (parameter names
//! and values, what they do on the user's projects) and calibrated only by black-box measurement against
//! Painter's saved results (`mcparity run generator…`).

mod filters;
mod params;
pub mod plan;
mod table;
mod textures;

pub use filters::FILTERS;
pub use plan::{Choice, ProceduralPlan};
pub use table::TABLE;
pub use textures::TEXTURES;

use super::layers::effect_data::{PLACEMENT_PREFIX, SUBSTANCE_PREFIX};
use materialcraft::paint::effect::Procedural;
use params::Painter;

/// One Substance procedural (by graph name) and the generator of ours that replaces it.
#[derive(Debug)]
pub struct Replacement {
    /// Painter's graph names this entry covers (the same graph in different packages or versions).
    pub painter: &'static [&'static str],
    /// What the import review shows.
    pub label: &'static str,
    /// Our generator, texture or filter id ([`materialcraft::generators`], [`materialcraft::proc_textures`],
    /// [`materialcraft::filters`]).
    pub generator: &'static str,
    /// What the replacement leaves out or approximates, for the review.
    pub note: &'static str,
    translate: fn(&Painter) -> Vec<(String, String)>,
}

impl Replacement {
    /// Our generator for `original` (an imported `substance:<graph>` procedural), keeping it as the original.
    pub fn convert(&self, original: &Procedural) -> Procedural {
        let painter = Painter::new(&original.params);
        let mut params = (self.translate)(&painter);
        if self.is_texture() {
            // a texture places itself like the fill it replaces (tiling, tri-planar volume …)
            params.extend(
                original
                    .params
                    .iter()
                    .filter_map(|(k, v)| Some((k.strip_prefix(PLACEMENT_PREFIX)?.to_string(), v.clone()))),
            );
        }
        Procedural::new(self.generator)
            .with_params(params)
            .replacing(original.clone())
    }

    /// True for the procedural-texture entries ([`TEXTURES`]), which take a placement.
    pub fn is_texture(&self) -> bool {
        TEXTURES.iter().any(|t| std::ptr::eq(t, self))
    }
}

/// Every entry: the mask generators ([`TABLE`]), the procedural textures ([`TEXTURES`]), then the filters
/// ([`FILTERS`]).
pub fn entries() -> impl Iterator<Item = &'static Replacement> {
    TABLE.iter().chain(TEXTURES).chain(FILTERS)
}

/// True if the entry stands in for a Painter filter (our id names a filter, not a generator).
pub fn is_filter(r: &Replacement) -> bool {
    FILTERS.iter().any(|f| std::ptr::eq(f, r))
}

/// The entry for Painter graph `name`, if MaterialCraft has a replacement.
pub fn find(name: &str) -> Option<&'static Replacement> {
    entries().find(|r| r.painter.contains(&name))
}

/// Our procedural texture standing in for Painter texture `name` (a texture entry's id), if there is one.
pub fn texture_for(name: &str) -> Option<&'static str> {
    TEXTURES.iter().find(|r| r.painter.contains(&name)).map(|r| r.generator)
}

/// The Painter graph name of an imported procedural id (`substance:mg_dirt` → `mg_dirt`).
pub fn painter_name(id: &str) -> Option<&str> {
    id.strip_prefix(SUBSTANCE_PREFIX)
}

/// The replacement of an imported procedural under `plan`, or `None` to keep it.
pub fn replace(p: &Procedural, plan: &ProceduralPlan) -> Option<(&'static Replacement, Procedural)> {
    let name = painter_name(&p.id)?;
    let r = find(name).filter(|_| plan.choice(name) == Choice::Replace)?;
    Some((r, r.convert(p)))
}

/// Converts a generator again from its kept original with today's table: a replaced procedural gets the
/// current translation, a kept one is replaced if `plan` asks for it. Returns `None` when nothing changes.
pub fn reconvert(p: &Procedural, plan: &ProceduralPlan) -> Option<Procedural> {
    let original = p.original.as_deref().unwrap_or(p);
    let name = painter_name(&original.id)?;
    let next = match find(name) {
        Some(r) if plan.choice(name) == Choice::Replace => r.convert(original),
        _ => original.clone(),
    };
    (next != *p).then_some(next)
}

#[cfg(test)]
mod tests;
