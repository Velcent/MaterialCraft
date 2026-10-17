//! Bake edits: a texture set's bake configuration and its baked maps.

use super::{Edit, EditError};
use crate::paint::bake_config::SetBakeConfig;
use crate::paint::doc::Document;
use crate::paint::texture_set::BakedMap;

pub(super) fn set_config(doc: &mut Document, set: String, config: SetBakeConfig) -> Result<Edit, EditError> {
    let Some(s) = doc.texture_set_mut(&set) else { return Err(EditError::NoTextureSet(set)) };
    let old = std::mem::replace(&mut s.bake, config);
    Ok(Edit::SetBakeConfig { set, config: Box::new(old) })
}

pub(super) fn set_baked(doc: &mut Document, set: String, maps: Vec<BakedMap>) -> Result<Edit, EditError> {
    let Some(s) = doc.texture_set_mut(&set) else { return Err(EditError::NoTextureSet(set)) };
    let old = std::mem::replace(&mut s.baked, maps);
    Ok(Edit::SetBaked { set, maps: old })
}
