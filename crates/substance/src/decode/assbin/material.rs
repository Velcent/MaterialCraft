//! One material block. Only `?mat.name` is read; in Painter it is the texture set name.
//!
//! Layout (after the block size): `u32` property count, then one block per property:
//! key string, `u32` semantic, `u32` index, `u32` type, `u32` data length, data.
//! Note: Painter writes type before length; upstream Assimp writes length before type.

use super::block::{read_block, read_count, read_string, read_u32};
use super::RECORD;
use crate::container::bytes::ByteReader;
use crate::decode::DecodeError;

const NAME_KEY: &str = "?mat.name";
/// Assimp's `aiPTI_String`: data is a `u32` length, the bytes and a NUL.
const TYPE_STRING: u32 = 3;
/// Smallest possible property block: size field, empty key and four `u32`s.
const MIN_PROPERTY_LEN: usize = 8 + 4 + 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialSummary {
    /// Assimp `?mat.name`; in Painter this is the texture set name. Empty if the material has none.
    pub name: String,
}

impl MaterialSummary {
    /// Parses the content of one material block.
    pub fn parse(block: &[u8]) -> Result<Self, DecodeError> {
        let mut r = ByteReader::new(block);
        let count = read_count(&mut r, MIN_PROPERTY_LEN)?;
        let mut name = String::new();
        for _ in 0..count {
            let property = read_block(&mut r)?;
            if let Some(found) = name_property(property)? {
                name = found;
            }
        }
        Ok(MaterialSummary { name })
    }
}

/// The material name if this property block is `?mat.name`.
fn name_property(block: &[u8]) -> Result<Option<String>, DecodeError> {
    let mut r = ByteReader::new(block);
    if read_string(&mut r)? != NAME_KEY {
        return Ok(None);
    }
    let _semantic = read_u32(&mut r)?;
    let _index = read_u32(&mut r)?;
    let kind = read_u32(&mut r)?;
    let len = read_u32(&mut r)? as usize;
    if kind != TYPE_STRING {
        return Err(DecodeError::invalid(
            RECORD,
            format!("{NAME_KEY} has type {kind}, expected a string"),
        ));
    }
    let data = r.bytes(len).map_err(DecodeError::truncated(RECORD))?;
    let name = read_string(&mut ByteReader::new(data))?;
    Ok(Some(name))
}
