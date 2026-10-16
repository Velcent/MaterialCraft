//! `/paint/scene.assbin`: the project mesh as an Assimp binary dump.
//!
//! Painter writes its own variant of Assimp's format (see [`block`] and [`scene`]).
//! [`AssbinScene`] is the cheap summary (counts and names); [`AssbinGeometry`] decodes everything.

mod block;
#[cfg(test)]
mod corpus_tests;
mod faces;
mod geometry;
#[cfg(test)]
mod geometry_tests;
mod header;
mod material;
mod mesh;
mod mesh_block;
mod node;
mod part;
mod repair;
mod scene;
#[cfg(test)]
mod test_writer;
#[cfg(test)]
mod tests;
mod transform;
#[cfg(test)]
mod visual_check;

pub use geometry::AssbinGeometry;
pub use header::{AssbinHeader, HEADER_LEN};
pub use material::MaterialSummary;
pub use mesh::MeshSummary;

use crate::decode::DecodeError;
use scene::{scene_bytes, SceneBlocks};

const RECORD: &str = "scene.assbin";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssbinScene {
    pub meshes: Vec<MeshSummary>,
    pub materials: Vec<MaterialSummary>,
}

impl AssbinScene {
    /// Reads names and counts only; vertex data is skipped, not decoded.
    pub fn parse(data: &[u8]) -> Result<Self, DecodeError> {
        let scene = scene_bytes(data)?;
        let blocks = SceneBlocks::split(&scene)?;
        Ok(AssbinScene {
            meshes: parse_all(&blocks.meshes, MeshSummary::parse)?,
            materials: parse_all(&blocks.materials, MaterialSummary::parse)?,
        })
    }

    /// The full geometry; same as [`AssbinGeometry::parse`].
    pub fn parse_geometry(data: &[u8]) -> Result<materialcraft::mesh::Mesh, DecodeError> {
        AssbinGeometry::parse(data)
    }
}

fn parse_all<T>(blocks: &[&[u8]], parse: impl Fn(&[u8]) -> Result<T, DecodeError>) -> Result<Vec<T>, DecodeError> {
    blocks.iter().map(|b| parse(b)).collect()
}
