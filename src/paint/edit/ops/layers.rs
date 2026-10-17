//! Creating, duplicating, deleting, grouping and moving layers (LS-10, LS-12…LS-17).

use super::super::ids::IdPool;
use super::super::{Edit, EditError, LayerRef, LayerSlot, Transaction};
use super::{channels, is_within, normalize, set, siblings, slot_of};
use crate::paint::blend::BlendMode;
use crate::paint::layer::{Fill, Layer, LayerKind, PaintContent};
use crate::paint::texture_set::TextureSet;

/// What a create button makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewLayer {
    /// An empty paint layer with an (unallocated) image for every channel of the set.
    Paint,
    /// A uniform fill with each channel's default value.
    Fill,
    /// An empty folder.
    Folder,
}

impl NewLayer {
    fn id_base(self) -> &'static str {
        match self {
            NewLayer::Paint => "layer",
            NewLayer::Fill => "fill",
            NewLayer::Folder => "folder",
        }
    }

    fn name(self) -> &'static str {
        match self {
            NewLayer::Paint => "Layer",
            NewLayer::Fill => "Fill layer",
            NewLayer::Folder => "Folder",
        }
    }

    /// The new layer, affecting every channel of `set` with `Normal` at full opacity.
    fn build(self, set: &TextureSet, id: String) -> Layer {
        let number = id.trim_start_matches(|c: char| !c.is_ascii_digit()).to_string();
        let kind = match self {
            NewLayer::Paint => {
                let images = set.channels.iter().filter_map(|spec| Some((spec.channel, empty_image(set, spec.layer_format())?))).collect();
                LayerKind::Paint(PaintContent::from_images(images))
            }
            NewLayer::Fill => LayerKind::Fill(Fill::uniform(channels(set).into_iter().map(|c| (c, c.default_value())).collect())),
            NewLayer::Folder => LayerKind::Group(Vec::new()),
        };
        let mut layer = Layer::new(id, format!("{} {number}", self.name()), kind);
        for c in channels(set) {
            layer.set_blend(c, BlendMode::Normal, 1.0);
        }
        layer
    }
}

fn empty_image(set: &TextureSet, format: crate::paint::PixelFormat) -> Option<crate::paint::TiledImage> {
    crate::paint::TiledImage::new(set.resolution.0, set.resolution.1, format).ok()
}

/// The slot directly above `id` (in the same group), or the top of the stack without one.
fn above(set: &TextureSet, id: Option<&str>) -> LayerSlot {
    match id.and_then(|id| slot_of(set, id)) {
        Some(s) => LayerSlot { parent: s.parent, index: s.index + 1 },
        None => LayerSlot::top(set.layers.len()),
    }
}

/// Creates a layer directly above layer `above_of` (top of the stack if `None`); returns its id.
pub fn create(tx: &mut Transaction, set_id: &str, what: NewLayer, above_of: Option<&str>) -> Result<String, EditError> {
    let s = set(tx, set_id)?;
    let id = IdPool::of(s).layer(what.id_base());
    let layer = what.build(s, id.clone());
    let at = above(s, above_of);
    tx.apply(Edit::InsertLayer { set: set_id.to_string(), at, layer: Box::new(layer) })?;
    Ok(id)
}

/// Copies every selected layer (with its children, masks and effects, under new ids) directly above the
/// original; returns the copies' ids in stack order (LS-13).
pub fn duplicate(tx: &mut Transaction, set_id: &str, ids: &[String]) -> Result<Vec<String>, EditError> {
    let chosen = normalize(set(tx, set_id)?, ids);
    let mut pool = IdPool::of(set(tx, set_id)?);
    let mut copies = Vec::new();
    for id in chosen {
        let s = set(tx, set_id)?;
        let at = above(s, Some(&id));
        let copy = copy_of(s, &id, &mut pool)?;
        copies.push(copy.id.clone());
        tx.apply(Edit::InsertLayer { set: set_id.to_string(), at, layer: Box::new(copy) })?;
    }
    Ok(copies)
}

/// Ctrl+drag: copies the selection and places the copies at `to` (a slot counted before the copies are
/// added); returns the copies' ids.
pub fn duplicate_to(tx: &mut Transaction, set_id: &str, ids: &[String], to: LayerSlot) -> Result<Vec<String>, EditError> {
    let chosen = normalize(set(tx, set_id)?, ids);
    let mut pool = IdPool::of(set(tx, set_id)?);
    let mut copies = Vec::new();
    for (k, id) in chosen.iter().enumerate() {
        let copy = copy_of(set(tx, set_id)?, id, &mut pool)?;
        copies.push(copy.id.clone());
        let at = LayerSlot { parent: to.parent.clone(), index: to.index + k };
        tx.apply(Edit::InsertLayer { set: set_id.to_string(), at, layer: Box::new(copy) })?;
    }
    Ok(copies)
}

fn copy_of(set: &TextureSet, id: &str, pool: &mut IdPool) -> Result<Layer, EditError> {
    let mut copy = set.layer(id).cloned().ok_or_else(|| EditError::NoLayer { set: set.id.clone(), layer: id.to_string() })?;
    pool.reid(&mut copy);
    copy.name = format!("{} copy", copy.name);
    Ok(copy)
}

/// Removes every selected layer (LS-12).
pub fn delete(tx: &mut Transaction, set_id: &str, ids: &[String]) -> Result<(), EditError> {
    let chosen = normalize(set(tx, set_id)?, ids);
    if chosen.is_empty() {
        return Err(EditError::NotAccepted("nothing is selected"));
    }
    for id in chosen {
        tx.apply(Edit::RemoveLayer(LayerRef::new(set_id, id)))?;
    }
    Ok(())
}

/// Puts the selection into a new folder at the place of its topmost layer, keeping its order (LS-17);
/// returns the folder's id.
pub fn group(tx: &mut Transaction, set_id: &str, ids: &[String]) -> Result<String, EditError> {
    let s = set(tx, set_id)?;
    let chosen = normalize(s, ids);
    let top = chosen.last().ok_or(EditError::NotAccepted("nothing is selected"))?;
    let at = above(s, Some(top));
    let folder = create_folder(s, &mut IdPool::of(s));
    let folder_id = folder.id.clone();
    tx.apply(Edit::InsertLayer { set: set_id.to_string(), at, layer: Box::new(folder) })?;
    move_layers(tx, set_id, &chosen, LayerSlot::in_group(folder_id.clone(), 0))?;
    Ok(folder_id)
}

fn create_folder(set: &TextureSet, pool: &mut IdPool) -> Layer {
    NewLayer::Folder.build(set, pool.layer(NewLayer::Folder.id_base()))
}

/// Moves the selection to `to`, keeping its order (LS-14, LS-15). `to` is a slot as the panel's marker shows
/// it: an index among the destination's children as they are *before* the move. Moving a group into itself is
/// refused.
pub fn move_layers(tx: &mut Transaction, set_id: &str, ids: &[String], to: LayerSlot) -> Result<(), EditError> {
    let s = set(tx, set_id)?;
    let chosen = normalize(s, ids);
    if chosen.is_empty() {
        return Err(EditError::NotAccepted("nothing is selected"));
    }
    if let Some(p) = &to.parent
        && let Some(g) = chosen.iter().find(|id| is_within(s, id, p))
    {
        return Err(EditError::IntoItself { set: set_id.to_string(), layer: g.clone() });
    }
    let children = siblings(s, to.parent.as_deref()).ok_or_else(|| match &to.parent {
        Some(p) if s.layer(p).is_some() => EditError::NotAGroup { set: set_id.to_string(), layer: p.clone() },
        p => EditError::NoLayer { set: set_id.to_string(), layer: p.clone().unwrap_or_default() },
    })?;
    if to.index > children.len() {
        return Err(EditError::IndexOutOfRange { index: to.index, len: children.len() });
    }
    // The index once the moved layers are out of the destination.
    let index = children.iter().take(to.index).filter(|c| !chosen.contains(&c.id)).count();
    let in_place =
        children.iter().skip(index).take(chosen.len()).map(|c| &c.id).eq(chosen.iter()) && children.iter().take(index).all(|c| !chosen.contains(&c.id));
    if in_place {
        return Ok(()); // already there: no edit, nothing to record
    }
    let mut taken = Vec::with_capacity(chosen.len());
    for id in &chosen {
        let s = set(tx, set_id)?;
        let layer = s.layer(id).cloned().ok_or_else(|| EditError::NoLayer { set: set_id.to_string(), layer: id.clone() })?;
        tx.apply(Edit::RemoveLayer(LayerRef::new(set_id, id.clone())))?;
        taken.push(layer);
    }
    for (k, layer) in taken.into_iter().enumerate() {
        let at = LayerSlot { parent: to.parent.clone(), index: index + k };
        tx.apply(Edit::InsertLayer { set: set_id.to_string(), at, layer: Box::new(layer) })?;
    }
    Ok(())
}
