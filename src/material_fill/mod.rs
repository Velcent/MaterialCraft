//! Material fills: MaterialCraft materials (`.mca` graphs, with a fill's parameter overrides) as fill sources
//! for Paint mode (roadmap step 14; requirements LS-06, LS-18; ADR 0008 rm-material-fills).
//!
//! The paint model stores a material fill as data ([`crate::paint::FillSource::Material`]: a reference and
//! parameter overrides) and knows no graph engine. This module sits above both: [`MaterialFills`] implements the
//! compositor's [`crate::paint::FillSources`] hook, so a fill layer or fill effect with a material is composited
//! through its projection like an image fill (tiling, UV placement, texture space).
//!
//! ```text
//! FillSource::Material { path, params }
//!   --MaterialResolver--> .mca text --Material::from_text--> graph with overrides, MaterialKey
//!   --MaterialCache::get_or_render--> RenderedMaterial (one texture + levels per channel, at the set's size)
//!   --FillSources::sample(taps)--> working-space values per tap
//! ```
//!
//! A reference resolves through a [`MaterialResolver`]: [`FolderResolver`] looks it up under folders (the
//! document's folder, asset library roots), [`MemoryResolver`] holds graphs in memory. Unknown references are
//! not handled, so the compositor leaves the fill out and reports it as not computable.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod cache;
mod material;
mod reference;
mod render;
mod sources;
#[cfg(test)]
mod tests;

pub use cache::{DEFAULT_BUDGET, MaterialCache};
pub use material::{Material, MaterialError, MaterialKey};
pub use reference::{FolderResolver, MAX_GRAPH_BYTES, MaterialResolver, MemoryResolver, ResolveError};
pub use render::{ChannelTexture, MAX_RENDER_SIZE, RenderedMaterial};
pub use sources::MaterialFills;
