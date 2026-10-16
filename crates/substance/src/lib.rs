//! Reads Substance 3D Painter projects (`.spp`) so MaterialCraft can import them.
//!
//! The crate is layered; each layer only uses the ones above it in this list:
//! - [`container`]: generic file formats (HDF5, Qt QDataStream, byte cursors). Knows nothing about Painter.
//! - [`decode`]: Painter's individual records (settings, resource images, texture cache, mesh, document).
//! - [`project`]: one opened `.spp` file, giving typed access to its parts.
//! - [`inspect`]: human-readable reports about one project or a whole folder of them.
//! - [`translate`]: a project → MaterialCraft's paint document (the importer).
//! - [`tools`]: what `mcsub` runs on top of the importer (map dumps, export comparison, batch conversion).
//!
//! Format notes live in the internal docs (`interop/substance-painter.md`), not here.

pub mod container;
pub mod decode;
pub mod inspect;
pub mod project;
pub mod tools;
pub mod translate;
