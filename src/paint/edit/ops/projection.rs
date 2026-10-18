//! A fill's projection (PJ-01 … PJ-22) as an edit: a fill layer's fill or a content stack's fill effect, named by
//! [`FillRef`], gets a new [`Projection`] through [`Edit::SetLayer`] or [`Edit::ReplaceEffect`]. Both kinds of
//! edit coalesce in the history, so a gizmo or slider drag recorded under one key is one undo step.

use super::{EditError, LayerRef, Transaction, layer};
use crate::paint::doc::Document;
use crate::paint::edit::{Edit, LayerProperty, StackEffect};
use crate::paint::effect::EffectKind;
use crate::paint::layer::{Fill, LayerKind};
use crate::projection::Projection;

/// Where a fill sits on its layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FillAt {
    /// The fill layer's own fill.
    Layer,
    /// The fill effect at this index of the layer's content stack (mask effects fill a grey value and have no
    /// projection).
    Effect(usize),
}

/// A fill by layer and place.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FillRef {
    pub layer: LayerRef,
    pub at: FillAt,
}

impl FillRef {
    pub fn new(layer: LayerRef, at: FillAt) -> FillRef {
        FillRef { layer, at }
    }

    /// The fill this names in `doc`, if it is there and is a fill.
    pub fn fill<'a>(&self, doc: &'a Document) -> Option<&'a Fill> {
        let l = doc.texture_set(&self.layer.set)?.layer(&self.layer.layer)?;
        match self.at {
            FillAt::Layer => match &l.kind {
                LayerKind::Fill(f) => Some(f),
                _ => None,
            },
            FillAt::Effect(i) => match &l.effects.get(i)?.kind {
                EffectKind::Fill(f) => Some(f),
                _ => None,
            },
        }
    }
}

/// Gives the fill `r` names `projection`; its source stays.
pub fn set_projection(tx: &mut Transaction, r: &FillRef, projection: Projection) -> Result<(), EditError> {
    let l = layer(tx, &r.layer)?;
    let edit = match r.at {
        FillAt::Layer => match &l.kind {
            LayerKind::Fill(f) => Edit::SetLayer { layer: r.layer.clone(), property: LayerProperty::Fill(Fill { projection, ..f.clone() }) },
            _ => return Err(EditError::NotAccepted("the layer is not a fill layer")),
        },
        FillAt::Effect(index) => {
            let mut effect = l.effects.get(index).cloned().ok_or(EditError::IndexOutOfRange { index, len: l.effects.len() })?;
            match &mut effect.kind {
                EffectKind::Fill(f) => f.projection = projection,
                _ => return Err(EditError::NotAccepted("the effect is not a fill")),
            }
            Edit::ReplaceEffect { layer: r.layer.clone(), index, effect: StackEffect::Content(effect) }
        }
    };
    tx.apply(edit)
}
