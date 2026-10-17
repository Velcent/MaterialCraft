//! Document-level edits: adding, removing and replacing texture sets.

use super::{Edit, EditError};
use crate::paint::doc::Document;
use crate::paint::texture_set::TextureSet;

fn position(doc: &Document, id: &str) -> Result<usize, EditError> {
    doc.texture_sets.iter().position(|s| s.id == id).ok_or_else(|| EditError::NoTextureSet(id.to_string()))
}

pub(super) fn insert_set(doc: &mut Document, index: usize, set: TextureSet) -> Result<Edit, EditError> {
    if set.id.is_empty() || doc.texture_set(&set.id).is_some() {
        return Err(EditError::DuplicateId { set: set.id.clone(), id: set.id });
    }
    if index > doc.texture_sets.len() {
        return Err(EditError::IndexOutOfRange { index, len: doc.texture_sets.len() });
    }
    let id = set.id.clone();
    doc.texture_sets.insert(index, set);
    Ok(Edit::RemoveTextureSet { set: id })
}

pub(super) fn remove_set(doc: &mut Document, id: &str) -> Result<Edit, EditError> {
    let index = position(doc, id)?;
    Ok(Edit::InsertTextureSet { index, set: Box::new(doc.texture_sets.remove(index)) })
}

pub(super) fn replace_set(doc: &mut Document, set: TextureSet) -> Result<Edit, EditError> {
    let index = position(doc, &set.id)?;
    match doc.texture_sets.get_mut(index) {
        Some(slot) => Ok(Edit::ReplaceTextureSet(Box::new(std::mem::replace(slot, set)))),
        None => Err(EditError::NoTextureSet(set.id)),
    }
}
