//! Shader files and shader instances as data (Paint mode VS-02 … VS-12).
//!
//! - A **shader file** ([`ShaderFile`]) is our own GLSL (a vertex and a fragment shader implementing the viewer's
//!   program interface, see [`decl::RESERVED_UNIFORMS`]) plus a JSON declaration of its parameters
//!   ([`ShaderDecl`]: types, ranges, defaults, groups). The built-in default shader is the viewport's PBR program
//!   ([`ShaderFile::builtin`]); more are loaded from folders into a [`ShaderLibrary`].
//! - A **shader instance** ([`ShaderInstance`]) is a named set of values for one shader file, with displacement
//!   ([`Displacement`]: channel, scale, unit, midlevel) and tessellation ([`Tessellation`]: CPU subdivision
//!   levels). A document keeps its instances and which texture set uses which in [`ShaderSettings`]; sets can
//!   share an instance.
//! - Changes are [`ShaderEdit`]s on the settings' own undo history (`crate::history::ShaderHistory`), separate
//!   from the project timeline (ADR 0008 q-undo-stacks).
//!
//! Pure data, std only: `gpu3d` compiles the programs and sends the values, `paint::format` stores the
//! instances, the editor shows the panel.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod decl;
mod displacement;
mod edit;
mod file;
mod instance;
mod param;
mod settings;

#[cfg(test)]
mod tests;

pub use decl::{DeclError, ShaderDecl};
pub use displacement::{
    Displacement, DisplacementSource, DisplacementUnit, MAX_LEVELS, SceneScale, TRIANGLE_BUDGET, Tessellation, TessellationMode, TessellationPlan, UnitWarning,
    WARN_LEVELS, subdivided_count,
};
pub use edit::{ShaderEdit, ShaderEditError};
pub use file::{DECL_EXTENSION, DEFAULT_SHADER, LoadError, ShaderFile, ShaderLibrary};
pub use instance::{ShaderInstance, UniformValue};
pub use param::{ParamDecl, ParamKind, ParamValue};
pub use settings::{Assignment, ShaderSettings};
