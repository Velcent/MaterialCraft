//! [`ImageSlot`]: which image of a document an archive entry holds.

use crate::paint::{Document, LayerImage, TiledImage};

/// Identifies one image of a document. Layers are found by id (unique within their set).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageSlot {
    /// One of a layer's images (paint, content effect or mask image).
    Layer { set: usize, layer: String, image: LayerImage },
    /// `texture_sets[set].baked[index]`.
    Baked { set: usize, index: usize },
}

impl ImageSlot {
    /// The image this slot names in `doc`, if it exists.
    pub fn image_mut<'d>(&self, doc: &'d mut Document) -> Option<&'d mut TiledImage> {
        match self {
            ImageSlot::Layer { set, layer, image } => doc.texture_sets.get_mut(*set)?.layer_mut(layer)?.image_mut(*image),
            ImageSlot::Baked { set, index } => Some(&mut doc.texture_sets.get_mut(*set)?.baked.get_mut(*index)?.image),
        }
    }
}
