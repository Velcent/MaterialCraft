//! The shared tiles a document or an edit holds, so memory can be counted per allocation (see
//! [`crate::paint::Tile::id`]).

use super::{Edit, LayerProperty, StackEffect};
use crate::paint::doc::Document;
use crate::paint::effect::EffectKind;
use crate::paint::image::{Tile, TiledImage};
use crate::paint::layer::{Layer, LayerWalk};
use crate::paint::mask::Mask;
use crate::paint::texture_set::TextureSet;

fn image_tiles(image: &TiledImage, f: &mut dyn FnMut(&Tile)) {
    image.tiles().for_each(|(_, t)| f(t));
}

fn layer_tiles(layer: &Layer, f: &mut dyn FnMut(&Tile)) {
    for (_, l) in LayerWalk::new(std::slice::from_ref(layer)) {
        l.images().into_iter().for_each(|(_, image)| image_tiles(image, f));
    }
}

fn mask_tiles(mask: &Mask, f: &mut dyn FnMut(&Tile)) {
    mask.images().for_each(|(_, image)| image_tiles(image, f));
}

fn set_tiles(set: &TextureSet, f: &mut dyn FnMut(&Tile)) {
    set.layers.iter().for_each(|l| layer_tiles(l, f));
    set.baked.iter().for_each(|b| image_tiles(&b.image, f));
}

/// Calls `f` for every allocated tile of every layer, mask and baked image of `doc` (a tile shared by
/// several images once per image).
pub fn document_tiles(doc: &Document, f: &mut dyn FnMut(&Tile)) {
    doc.texture_sets.iter().for_each(|s| set_tiles(s, f));
}

impl Edit {
    /// Calls `f` for every tile the edit holds (old tiles of a swap, the images of a removed layer …).
    pub fn for_each_tile(&self, f: &mut dyn FnMut(&Tile)) {
        match self {
            Edit::InsertLayer { layer, .. } => layer_tiles(layer, f),
            Edit::SetLayer { property: LayerProperty::Mask(Some(m)), .. } => mask_tiles(m, f),
            Edit::InsertEffect { effect, .. } | Edit::ReplaceEffect { effect, .. } => match effect {
                StackEffect::Content(e) => {
                    if let EffectKind::Paint(p) = &e.kind {
                        p.images.iter().for_each(|(_, image)| image_tiles(image, f));
                    }
                }
                StackEffect::Mask(e) => {
                    if let EffectKind::Paint(p) = &e.kind {
                        image_tiles(&p.image, f);
                    }
                }
            },
            Edit::SwapTiles { tiles, .. } => tiles.iter().filter_map(|s| s.tile.as_ref()).for_each(f),
            Edit::InsertTextureSet { set, .. } => set_tiles(set, f),
            Edit::ReplaceTextureSet(set) => set_tiles(set, f),
            Edit::SetBaked { maps, .. } => maps.iter().for_each(|b| image_tiles(&b.image, f)),
            _ => {}
        }
    }
}
