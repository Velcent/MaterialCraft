//! `/paint/scene.assbin`: the project mesh as an Assimp binary dump. M1 only needs a summary.
//!
//! Painter writes its own variant of Assimp's format (see [`block`]); the scene layout is:
//! `u64` size, `u32` flags, root node block (skipped), `u32` mesh count + mesh blocks,
//! `u32` material count + material blocks, then animations, textures, lights, cameras (not read).

mod block;
mod header;
mod material;
mod mesh;
#[cfg(test)]
mod test_writer;
#[cfg(test)]
mod tests;

pub use header::{AssbinHeader, HEADER_LEN};
pub use material::MaterialSummary;
pub use mesh::MeshSummary;

use crate::container::bytes::ByteReader;
use crate::decode::DecodeError;
use block::{read_block, read_count, read_u32};

const RECORD: &str = "scene.assbin";
/// Smallest possible mesh or material block: its size field and one `u32`.
const MIN_ITEM_LEN: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssbinScene {
    pub meshes: Vec<MeshSummary>,
    pub materials: Vec<MaterialSummary>,
}

impl AssbinScene {
    pub fn parse(data: &[u8]) -> Result<Self, DecodeError> {
        let header = AssbinHeader::parse(data)?;
        if header.shortened {
            return Err(DecodeError::invalid(RECORD, "shortened dump carries no mesh data"));
        }
        let bytes = header.scene_bytes(data)?;
        let mut outer = ByteReader::new(&bytes);
        let mut r = ByteReader::new(read_block(&mut outer)?);
        let _flags = read_u32(&mut r)?;
        let _root_node = read_block(&mut r)?;
        let meshes = read_items(&mut r, MeshSummary::parse)?;
        let materials = read_items(&mut r, MaterialSummary::parse)?;
        Ok(AssbinScene { meshes, materials })
    }
}

/// Reads a `u32` count followed by that many blocks, parsing each with `parse`.
fn read_items<T>(r: &mut ByteReader, parse: impl Fn(&[u8]) -> Result<T, DecodeError>) -> Result<Vec<T>, DecodeError> {
    let count = read_count(r, MIN_ITEM_LEN)?;
    (0..count).map(|_| parse(read_block(r)?)).collect()
}
