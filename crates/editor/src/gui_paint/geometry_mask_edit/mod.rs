//! The geometry mask edit mode (GM-03, GM-05): while a layer's or folder's geometry mask is being edited, the parts
//! it excludes are hidden in the viewport and skipped by picking, a click on a part in the viewport excludes it, and
//! a list window switches parts on and off. Every change is one [`LayerEvent::SetGeometryMask`], so one history step.
//!
//! - this file: [`MaskTarget`], what the mode works out from the document (hidden parts, the event for a change);
//! - [`window`]: the part list;
//! - [`host`]: wiring into `PaintMode` (the viewport, picking and the layer panel's state).

mod host;
mod window;

use super::panels::layers::LayerEvent;
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::{Document, GeometryMask, Layer, TextureSet};

/// The layer whose geometry mask is edited, looked up in the document.
pub struct MaskTarget<'a> {
    doc: &'a Document,
    set: &'a TextureSet,
    layer: &'a Layer,
    r: &'a LayerRef,
}

impl<'a> MaskTarget<'a> {
    /// `None` when the layer is not in the document (deleted, or its creation undone).
    pub fn find(doc: &'a Document, r: &'a LayerRef) -> Option<MaskTarget<'a>> {
        let set = doc.texture_set(&r.set)?;
        let layer = set.layer(&r.layer)?;
        Some(MaskTarget { doc, set, layer, r })
    }

    pub fn layer(&self) -> &Layer {
        self.layer
    }

    pub fn set(&self) -> &TextureSet {
        self.set
    }

    /// The names of the set's parts, in mesh order.
    pub fn parts(&self) -> Vec<String> {
        GeometryMask::set_parts(&self.doc.mesh, self.set.material_slot)
    }

    /// True if the mask lets the part named `name` through.
    pub fn includes(&self, name: &str) -> bool {
        self.layer.geometry_mask.as_ref().is_none_or(|m| m.includes_part(name))
    }

    /// Per mesh part: true if it belongs to the set and the mask excludes it (what the viewport hides).
    pub fn hidden_parts(&self) -> Vec<bool> {
        self.doc.mesh.parts.iter().map(|p| p.material == self.set.material_slot && !self.includes(&p.name)).collect()
    }

    /// The event that switches the part named `name` on (`include`) or off; `None` if that changes nothing.
    pub fn set_part(&self, name: &str, include: bool) -> Option<LayerEvent> {
        let mask = GeometryMask::with_part(self.layer.geometry_mask.as_ref(), name, include, &self.parts());
        (mask != self.layer.geometry_mask).then(|| LayerEvent::SetGeometryMask(self.r.clone(), mask))
    }

    /// The event for a click on mesh part `part` in the viewport: excludes it if it belongs to the set.
    pub fn exclude_mesh_part(&self, part: usize) -> Option<LayerEvent> {
        let p = self.doc.mesh.parts.get(part).filter(|p| p.material == self.set.material_slot)?;
        self.set_part(&p.name, false)
    }

    /// The event that sets the whole mask (`None`: every part); `None` if that changes nothing.
    pub fn replace(&self, mask: Option<GeometryMask>) -> Option<LayerEvent> {
        (mask != self.layer.geometry_mask).then(|| LayerEvent::SetGeometryMask(self.r.clone(), mask))
    }
}

#[cfg(test)]
mod tests;
