//! MaterialCraft's starter asset library (roadmap step 25; ADR 0007 "starter assets", ADR 0008 rm-starter-scope;
//! requirement AS-22): built from MaterialCraft's own graphs, generators and procedural textures, never from Adobe
//! assets. [`build`] writes the library into a folder in the standard layout (one folder per usage), and the
//! `mcstarter` binary runs it from the command line.
//!
//! ```text
//! content/materials/*.mcg --Graph::publish--> materials/<id>.mca            (materials)
//! generator presets       --Fragment::smart_mask--> smart-masks/<id>.mcfrag (masks)
//! proc_textures looks     --sampled, 16-bit PNG--> textures/<id>.png        (textures)
//! PhotoCraft's tips + ours --16-bit PNG--> alphas/<id>.png                  (alphas)
//! brush presets on those tips --.mcbrush + stroke thumbnail--> brushes/<id>.mcbrush (brushes)
//! the viewport's studio   --lat-long OpenEXR--> environments/studio.exr     (environment)
//! every asset: <file>.mcmeta (name, usage, tags, provenance) and a thumbnail rendered here
//! ```
//!
//! Every asset is a [`Recipe`]: it makes the asset's bytes, its sidecar and its thumbnail from fixed inputs, so
//! the same build gives the same bytes ([`build`] writes them; `starter.json` lists them with the library's
//! [`LIBRARY_VERSION`]). Bump that version whenever a recipe changes what it makes: the editor rebuilds a starter
//! library whose version differs.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod alphas;
mod brushes;
mod build;
mod environment;
mod error;
mod image;
mod manifest;
mod masks;
mod materials;
mod patterns;
mod preview;
mod recipe;
mod reference;
mod textures;
mod texturizer;
mod translated;

pub use build::{Built, Options, build, recipes};
pub use error::StarterError;
pub use manifest::{LIBRARY_VERSION, MANIFEST, installed_version, is_current};
pub use recipe::{Made, Recipe};
