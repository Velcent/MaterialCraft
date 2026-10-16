//! One mesh block: name, counts and vertex layout. Vertex and face data are skipped, not decoded.
//!
//! Layout (after the block size): name, `u32` primitive types, `u32` material index,
//! `u32` component bits, then the vertex arrays in Assimp's order, each prefixed with its own
//! `u32` element count, then `u32` face count followed by the faces and bones (not read).

use super::block::{read_count, read_string, read_u32, skip_items};
use crate::container::bytes::ByteReader;
use crate::decode::DecodeError;

const HAS_POSITIONS: u32 = 0x1;
const HAS_NORMALS: u32 = 0x2;
const HAS_TANGENTS_AND_BITANGENTS: u32 = 0x4;
const TEXCOORD_BASE: u32 = 0x100;
const COLOR_BASE: u32 = 0x1_0000;
/// Assimp's limit for both colour sets and texture coordinate sets.
const MAX_SETS: u32 = 8;

const VEC3_LEN: usize = 12;
const COLOR4_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshSummary {
    pub name: String,
    pub vertices: u32,
    pub faces: u32,
    pub uv_channels: u32,
    pub material_index: u32,
}

impl MeshSummary {
    /// Parses the content of one mesh block.
    pub fn parse(block: &[u8]) -> Result<Self, DecodeError> {
        let mut r = ByteReader::new(block);
        let name = read_string(&mut r)?;
        let _primitive_types = read_u32(&mut r)?;
        let material_index = read_u32(&mut r)?;
        let components = read_u32(&mut r)?;
        let vertices = skip_vertex_arrays(&mut r, components)?;
        let faces = read_u32(&mut r)?;
        Ok(MeshSummary {
            name,
            vertices,
            faces,
            uv_channels: set_count(components, TEXCOORD_BASE),
            material_index,
        })
    }
}

/// Skips all vertex arrays; returns the vertex count (the length of the position array).
fn skip_vertex_arrays(r: &mut ByteReader, components: u32) -> Result<u32, DecodeError> {
    let mut vertices = 0;
    if components & HAS_POSITIONS != 0 {
        vertices = skip_array(r, VEC3_LEN)?;
    }
    if components & HAS_NORMALS != 0 {
        skip_array(r, VEC3_LEN)?;
    }
    if components & HAS_TANGENTS_AND_BITANGENTS != 0 {
        skip_array(r, VEC3_LEN)?;
        skip_array(r, VEC3_LEN)?;
    }
    for _ in 0..set_count(components, COLOR_BASE) {
        skip_array(r, COLOR4_LEN)?;
    }
    for _ in 0..set_count(components, TEXCOORD_BASE) {
        let _components_per_uv = read_u32(r)?;
        skip_array(r, VEC3_LEN)?;
    }
    Ok(vertices)
}

/// Skips a counted array and returns its element count.
fn skip_array(r: &mut ByteReader, item_len: usize) -> Result<u32, DecodeError> {
    let count = read_count(r, item_len)?;
    skip_items(r, count, item_len)?;
    // `read_count` read the value from a u32, so it fits.
    Ok(count as u32)
}

/// Number of consecutive sets present, as Assimp stops at the first missing one.
fn set_count(components: u32, base: u32) -> u32 {
    (0..MAX_SETS).take_while(|&n| components & (base << n) != 0).count() as u32
}
