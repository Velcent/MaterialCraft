//! [`ImageSlot`]: which image of a document an archive entry holds.

use crate::paint::{Channel, Document, LayerKind, TiledImage};

/// Identifies one image of a document. Layers are found by id (unique within their set).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageSlot {
    /// A paint layer's channel image.
    Paint {
        set: usize,
        layer: String,
        channel: Channel,
    },
    /// A layer's mask.
    Mask { set: usize, layer: String },
    /// `texture_sets[set].baked[index]`.
    Baked { set: usize, index: usize },
}

impl ImageSlot {
    /// The image this slot names in `doc`, if it exists.
    pub fn image_mut<'d>(&self, doc: &'d mut Document) -> Option<&'d mut TiledImage> {
        match self {
            ImageSlot::Paint {
                set,
                layer,
                channel,
            } => match &mut doc.texture_sets.get_mut(*set)?.layer_mut(layer)?.kind {
                LayerKind::Paint(p) => p.image_mut(*channel),
                _ => None,
            },
            ImageSlot::Mask { set, layer } => doc
                .texture_sets
                .get_mut(*set)?
                .layer_mut(layer)?
                .mask
                .as_mut()
                .map(|m| m.image_mut()),
            ImageSlot::Baked { set, index } => {
                Some(&mut doc.texture_sets.get_mut(*set)?.baked.get_mut(*index)?.image)
            }
        }
    }
}
