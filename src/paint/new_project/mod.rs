//! New projects from a mesh file (roadmap step 50): everything File > New does, without the GUI.
//!
//! ```text
//! NewProject (settings) ──> create() ──> MeshSource::read (OBJ / glTF binary: mesh, unit, cameras)
//!                                    ──> MaterialSets::of (one texture set per used material slot)
//!                                    ──> ProjectBuilder (sets from the template, view, shading, project settings)
//!                                    ──> BakedFiles (files named <set>_<map> become baked maps)
//!                                    ──> Created { document, notes }
//! ```
//!
//! - [`settings`]: [`NewProject`], the choices of the New project dialog; [`UnitScale`] and the resolution list.
//! - [`mesh_file`]: which mesh files can be read ([`MeshFormat`]) and reading one ([`MeshSource`]).
//! - [`material_sets`]: the texture sets a mesh gets, with unique ids.
//! - [`baked_names`]: matching file names to texture sets and mesh maps; [`baked_files`]: reading them.
//! - [`template`]: [`ProjectTemplate`], a saved set of defaults, and the built-in ones; its file format is
//!   [`crate::paint::format::template`].
//! - [`builder`]: [`ProjectBuilder`], the document from a read mesh; [`create`] does the whole job from files.
//! - [`error`]: why a project could not be made ([`NewProjectError`]).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod baked_files;
pub mod baked_names;
pub mod builder;
pub mod error;
pub mod material_sets;
pub mod mesh_file;
pub mod settings;
pub mod template;

#[cfg(test)]
mod tests;

pub use builder::{Created, ProjectBuilder, create};
pub use error::NewProjectError;
pub use material_sets::{MaterialSets, PlannedSet};
pub use mesh_file::{MeshFormat, MeshSource};
pub use settings::{NewProject, RESOLUTIONS, UnitScale};
pub use template::ProjectTemplate;
