//! Mask states the layer panel changes: add (MK-02), remove (MK-09: undoable), enable (MK-06) and invert the
//! starting value (MK-08).

use super::super::{Edit, EditError, LayerProperty, LayerRef, Transaction};
use super::layer;
use crate::paint::effect::{ColorSelection, EffectKind, GreyBlend, MaskEffect};
use crate::paint::mask::Mask;
use crate::paint::texture_set::BakedKind;

/// What a new mask starts as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskStart {
    /// Lets the whole layer through.
    White,
    /// Hides the whole layer until painted.
    Black,
    /// Black with a colour selection on the ID map on top.
    ColorSelection,
}

impl MaskStart {
    pub const ALL: [MaskStart; 3] = [MaskStart::White, MaskStart::Black, MaskStart::ColorSelection];

    pub fn label(self) -> &'static str {
        match self {
            MaskStart::White => "White mask",
            MaskStart::Black => "Black mask",
            MaskStart::ColorSelection => "Mask from colour selection",
        }
    }

    fn build(self) -> Mask {
        match self {
            MaskStart::White => Mask::new(1.0),
            MaskStart::Black => Mask::new(0.0),
            MaskStart::ColorSelection => {
                let pick = ColorSelection { map: BakedKind::Id, colors: Vec::new(), tolerance: 0.1, hardness: 1.0 };
                Mask::new(0.0).with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::ColorSelection(pick)))
            }
        }
    }
}

/// Gives layer `r` a new mask; an existing one is replaced with its effects (the step can be undone).
pub fn add_mask(tx: &mut Transaction, r: &LayerRef, start: MaskStart) -> Result<(), EditError> {
    tx.apply(Edit::SetLayer { layer: r.clone(), property: LayerProperty::Mask(Some(start.build())) })
}

pub fn remove_mask(tx: &mut Transaction, r: &LayerRef) -> Result<(), EditError> {
    if layer(tx, r)?.mask.is_none() {
        return Err(EditError::NoMask { set: r.set.clone(), layer: r.layer.clone() });
    }
    tx.apply(Edit::SetLayer { layer: r.clone(), property: LayerProperty::Mask(None) })
}

pub fn set_mask_enabled(tx: &mut Transaction, r: &LayerRef, enabled: bool) -> Result<(), EditError> {
    tx.apply(Edit::SetLayer { layer: r.clone(), property: LayerProperty::MaskEnabled(enabled) })
}

/// Black ↔ white: the starting value becomes `1 − value`; the effects stay as they are.
pub fn invert_mask(tx: &mut Transaction, r: &LayerRef) -> Result<(), EditError> {
    let mask = layer(tx, r)?.mask.as_ref().ok_or_else(|| EditError::NoMask { set: r.set.clone(), layer: r.layer.clone() })?;
    let inverted = 1.0 - mask.initial.clamp(0.0, 1.0);
    tx.apply(Edit::SetLayer { layer: r.clone(), property: LayerProperty::MaskInitial(inverted) })
}
