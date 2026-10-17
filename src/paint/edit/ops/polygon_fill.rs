//! The polygon fill tool's edit (PT-45, ADR 0008 mc2-q-pt-01c): a pick changes the selection of the target stack's
//! topmost polygon fill effect, which is added on top when the stack has none. In a mask the fill is white; in a
//! layer's content every channel of the set gets white as grey. One call is one undo step.

use super::super::{Edit, EditError, LayerRef, StackEffect, StackId, Transaction};
use super::{channels, layer, set};
use crate::paint::blend::BlendMode;
use crate::paint::effect::{ContentEffect, EffectKind, GreyBlend, MaskEffect, PolygonFill, Selection};
use crate::paint::layer::Fill;

/// What [`edit_polygon_fill`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonFillChange {
    /// The selection of the effect at this index changed.
    Changed(usize),
    /// A new effect was added at this index.
    Added(usize),
    /// Nothing changed (the pick added nothing new or took out nothing selected).
    Unchanged,
}

/// Replaces the selection of the topmost polygon fill of stack `stack` of layer `r` with `change(old)`; without such
/// an effect, `change` gets an empty selection and a new effect is put on top (unless the result is empty too).
pub fn edit_polygon_fill(
    tx: &mut Transaction,
    r: &LayerRef,
    stack: StackId,
    change: impl FnOnce(&Selection) -> Selection,
) -> Result<PolygonFillChange, EditError> {
    let l = layer(tx, r)?;
    let found = match stack {
        StackId::Content => topmost(l.effects.iter().map(|e| match &e.kind {
            EffectKind::PolygonFill(_) => Some(StackEffect::Content(e.clone())),
            _ => None,
        })),
        StackId::Mask => {
            let mask = l.mask.as_ref().ok_or_else(|| EditError::NoMask { set: r.set.clone(), layer: r.layer.clone() })?;
            topmost(mask.effects.iter().map(|e| match &e.kind {
                EffectKind::PolygonFill(_) => Some(StackEffect::Mask(e.clone())),
                _ => None,
            }))
        }
    };
    match found {
        Some((index, mut effect)) => {
            let old = selection_of(&effect).cloned().unwrap_or_default();
            let new = change(&old);
            if new == old {
                return Ok(PolygonFillChange::Unchanged);
            }
            set_selection(&mut effect, new);
            tx.apply(Edit::ReplaceEffect { layer: r.clone(), index, effect })?;
            Ok(PolygonFillChange::Changed(index))
        }
        None => {
            let selection = change(&Selection::default());
            if selection.picks.is_empty() {
                return Ok(PolygonFillChange::Unchanged);
            }
            let effect = new_polygon_fill(tx, r, stack, selection)?;
            let index = super::add_effect(tx, r, effect, None)?;
            Ok(PolygonFillChange::Added(index))
        }
    }
}

/// The last `Some` of the stack (bottom to top) with its index.
fn topmost(effects: impl Iterator<Item = Option<StackEffect>>) -> Option<(usize, StackEffect)> {
    effects.enumerate().filter_map(|(i, e)| Some((i, e?))).last()
}

fn selection_of(e: &StackEffect) -> Option<&Selection> {
    match e {
        StackEffect::Content(ContentEffect { kind: EffectKind::PolygonFill(p), .. }) => Some(&p.selection),
        StackEffect::Mask(MaskEffect { kind: EffectKind::PolygonFill(p), .. }) => Some(&p.selection),
        _ => None,
    }
}

fn set_selection(e: &mut StackEffect, selection: Selection) {
    match e {
        StackEffect::Content(ContentEffect { kind: EffectKind::PolygonFill(p), .. }) => p.selection = selection,
        StackEffect::Mask(MaskEffect { kind: EffectKind::PolygonFill(p), .. }) => p.selection = selection,
        _ => {}
    }
}

/// A white polygon fill for `stack` of layer `r`.
fn new_polygon_fill(tx: &Transaction, r: &LayerRef, stack: StackId, selection: Selection) -> Result<StackEffect, EditError> {
    match stack {
        StackId::Mask => Ok(StackEffect::Mask(MaskEffect::new(GreyBlend::default(), EffectKind::PolygonFill(PolygonFill { selection, values: 1.0 })))),
        StackId::Content => {
            let set = set(tx, &r.set)?;
            let channels = channels(set);
            let values = Fill::uniform(channels.iter().map(|c| (*c, [1.0; 4])).collect());
            let mut e = ContentEffect::new(Vec::new(), EffectKind::PolygonFill(PolygonFill { selection, values }));
            for c in channels {
                e = e.with_blend(c, BlendMode::Normal, 1.0);
            }
            Ok(StackEffect::Content(e))
        }
    }
}
