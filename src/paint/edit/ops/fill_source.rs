//! A fill's source as an edit (the fill properties: a channel's texture and its parameters): the fill [`FillRef`]
//! names gets a new [`FillSource`] through [`Edit::SetLayer`] or [`Edit::ReplaceEffect`]; its projection and
//! original stay. Both kinds of edit coalesce in the history, so a slider drag under one key is one undo step.

use super::{EditError, FillAt, FillRef, Transaction, layer};
use crate::paint::edit::{Edit, LayerProperty, StackEffect};
use crate::paint::effect::EffectKind;
use crate::paint::layer::{Fill, FillSource, LayerKind};

/// Gives the fill `r` names `source`.
pub fn set_fill_source(tx: &mut Transaction, r: &FillRef, source: FillSource) -> Result<(), EditError> {
    let l = layer(tx, &r.layer)?;
    let edit = match r.at {
        FillAt::Layer => match &l.kind {
            LayerKind::Fill(f) => Edit::SetLayer { layer: r.layer.clone(), property: LayerProperty::Fill(Fill { source, ..f.clone() }) },
            _ => return Err(EditError::NotAccepted("the layer is not a fill layer")),
        },
        FillAt::Effect(index) => {
            let mut effect = l.effects.get(index).cloned().ok_or(EditError::IndexOutOfRange { index, len: l.effects.len() })?;
            match &mut effect.kind {
                EffectKind::Fill(f) => f.source = source,
                _ => return Err(EditError::NotAccepted("the effect is not a fill")),
            }
            Edit::ReplaceEffect { layer: r.layer.clone(), index, effect: StackEffect::Content(effect) }
        }
    };
    tx.apply(edit)
}
