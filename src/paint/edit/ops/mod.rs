//! The layer panel's operations on a texture set's stack, each built from [`super::Edit`]s inside a
//! [`Transaction`], so it is one undo step and either happens completely or not at all.
//!
//! - [`layers`]: create, duplicate, delete, group and move layers (a selection of several at once);
//! - [`effects`]: new effects for either stack and switching them on and off;
//! - [`instances`]: paste as instance and instantiate across texture sets;
//! - [`masks`]: adding, removing, switching and inverting masks;
//! - [`drop`]: what dropping an asset onto the stack does (ADR 0008, q-effect-drop); [`drop_layers`]: the
//!   layers a dropped material or smart material makes;
//! - [`polygon_fill`]: the polygon fill tool's pick into the target stack's polygon fill effect;
//! - [`projection`]: a fill layer's or fill effect's projection; [`fill_source`]: its source (textures, values).
//!
//! Selections name layers by id. Before an operation a selection is put in stack order and layers whose
//! group is selected too are left out (the group carries them), see [`normalize`].

mod drop;
mod drop_layers;
mod effects;
mod fill_source;
mod instances;
mod layers;
mod masks;
mod polygon_fill;
mod projection;

#[cfg(test)]
mod tests;

pub use drop::{AssetEffect, DropOutcome, DropPayload, DropTarget, EffectAsset, accepts, drop_onto};
pub use drop_layers::MaterialAsset;
pub use effects::{NewEffect, add_effect, new_effect, set_effect_enabled};
pub use fill_source::set_fill_source;
pub use instances::{instantiate, paste_instance};
pub use layers::{NewLayer, create, delete, duplicate, duplicate_to, group, move_layers};
pub use masks::{MaskStart, add_mask, invert_mask, remove_mask, set_mask_enabled};
pub use polygon_fill::{PolygonFillChange, edit_polygon_fill};
pub use projection::{FillAt, FillRef, set_projection};

use super::tree::locate;
use super::{EditError, LayerRef, LayerSlot, Transaction};
use crate::paint::channel::Channel;
use crate::paint::layer::{Layer, LayerWalk};
use crate::paint::texture_set::TextureSet;

fn set<'a>(tx: &'a Transaction, id: &str) -> Result<&'a TextureSet, EditError> {
    tx.doc().texture_set(id).ok_or_else(|| EditError::NoTextureSet(id.to_string()))
}

fn layer<'a>(tx: &'a Transaction, r: &LayerRef) -> Result<&'a Layer, EditError> {
    set(tx, &r.set)?.layer(&r.layer).ok_or_else(|| EditError::NoLayer { set: r.set.clone(), layer: r.layer.clone() })
}

/// Where layer `id` sits in `set`.
pub fn slot_of(set: &TextureSet, id: &str) -> Option<LayerSlot> {
    locate(&set.layers, id, None)
}

/// The children list a slot's parent holds (`None` parent: the top level).
fn siblings<'a>(set: &'a TextureSet, parent: Option<&str>) -> Option<&'a [Layer]> {
    match parent {
        None => Some(&set.layers),
        Some(p) => set.layer(p).map(|l| l.children()),
    }
}

/// True if `id` is `ancestor` or inside it.
pub fn is_within(set: &TextureSet, ancestor: &str, id: &str) -> bool {
    set.layer(ancestor).is_some_and(|a| LayerWalk::new(std::slice::from_ref(a)).any(|(_, l)| l.id == id))
}

/// The selection in stack walk order (bottom to top, a group before its children), without unknown ids,
/// duplicates, and layers inside a selected group.
pub fn normalize(set: &TextureSet, ids: &[String]) -> Vec<String> {
    let chosen: Vec<&str> = set.walk_layers().map(|(_, l)| l.id.as_str()).filter(|id| ids.iter().any(|s| s == id)).collect();
    chosen.iter().filter(|id| !chosen.iter().any(|g| g != *id && is_within(set, g, id))).map(|id| id.to_string()).collect()
}

/// The set's channels, in display order.
fn channels(set: &TextureSet) -> Vec<Channel> {
    set.channels.iter().map(|c| c.channel).collect()
}
