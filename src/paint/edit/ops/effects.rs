//! Effects the layer panel adds (MK-14) and switches on or off.

use super::super::ids::IdPool;
use super::super::{Edit, EditError, LayerRef, StackEffect, StackId, Transaction};
use super::{channels, layer};
use crate::paint::blend::BlendMode;
use crate::paint::effect::{Anchor, ColorSelection, ContentEffect, EffectKind, GreyBlend, Levels, MaskEffect, MaskPixels};
use crate::paint::layer::{Fill, PaintContent};
use crate::paint::texture_set::{BakedKind, TextureSet};
use crate::paint::{PixelFormat, TiledImage};

/// The effects the add-effect menu offers. Generators and filters come from the asset tray (see
/// [`super::drop`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewEffect {
    /// A paint effect to paint into (content: an image per channel; mask: one grey image).
    Paint,
    /// A uniform fill (content: each channel's default; mask: mid grey).
    Fill,
    Levels,
    /// A colour selection on the ID map.
    ColorSelection,
    /// A named point of the stack for references.
    Anchor,
}

impl NewEffect {
    pub const ALL: [NewEffect; 5] = [NewEffect::Paint, NewEffect::Fill, NewEffect::Levels, NewEffect::ColorSelection, NewEffect::Anchor];

    pub fn label(self) -> &'static str {
        match self {
            NewEffect::Paint => "Paint",
            NewEffect::Fill => "Fill",
            NewEffect::Levels => "Levels",
            NewEffect::ColorSelection => "Colour selection",
            NewEffect::Anchor => "Anchor point",
        }
    }
}

/// A new effect of kind `what` for stack `stack` of a layer in `set`. Content effects affect every channel of
/// the set with `Normal` at full opacity.
pub fn new_effect(set: &TextureSet, what: NewEffect, stack: StackId) -> StackEffect {
    let selection = || ColorSelection { map: BakedKind::Id, colors: Vec::new(), tolerance: 0.1, hardness: 1.0 };
    let anchor = || Anchor { id: IdPool::of(set).anchor("anchor") };
    match stack {
        StackId::Content => {
            let kind = match what {
                NewEffect::Paint => EffectKind::Paint(PaintContent::from_images(
                    set.channels
                        .iter()
                        .filter_map(|s| Some((s.channel, TiledImage::new(set.resolution.0, set.resolution.1, s.layer_format()).ok()?)))
                        .collect(),
                )),
                NewEffect::Fill => EffectKind::Fill(Fill::uniform(channels(set).into_iter().map(|c| (c, c.default_value())).collect())),
                NewEffect::Levels => EffectKind::Levels(Levels::default()),
                NewEffect::ColorSelection => EffectKind::ColorSelection(selection()),
                NewEffect::Anchor => EffectKind::Anchor(anchor()),
            };
            let mut e = ContentEffect::new(Vec::new(), kind);
            for c in channels(set) {
                e = e.with_blend(c, BlendMode::Normal, 1.0);
            }
            StackEffect::Content(e)
        }
        StackId::Mask => {
            let kind = match what {
                NewEffect::Paint => match mask_image(set) {
                    Some(image) => EffectKind::Paint(MaskPixels::new(image)),
                    None => EffectKind::Fill(0.0),
                },
                NewEffect::Fill => EffectKind::Fill(0.5),
                NewEffect::Levels => EffectKind::Levels(Levels::default()),
                NewEffect::ColorSelection => EffectKind::ColorSelection(selection()),
                NewEffect::Anchor => EffectKind::Anchor(anchor()),
            };
            StackEffect::Mask(MaskEffect::new(GreyBlend::default(), kind))
        }
    }
}

/// An empty grey image with coverage at the set's resolution, for painting into a mask.
pub(super) fn mask_image(set: &TextureSet) -> Option<TiledImage> {
    let format = PixelFormat::from_id("graya8")?;
    TiledImage::new(set.resolution.0, set.resolution.1, format).ok()
}

/// Adds `effect` on top of its stack of layer `r` (at `index` if given); returns the index it got.
pub fn add_effect(tx: &mut Transaction, r: &LayerRef, effect: StackEffect, index: Option<usize>) -> Result<usize, EditError> {
    let l = layer(tx, r)?;
    let len = match effect.stack() {
        StackId::Content => l.effects.len(),
        StackId::Mask => l.mask.as_ref().map(|m| m.effects.len()).ok_or_else(|| EditError::NoMask { set: r.set.clone(), layer: r.layer.clone() })?,
    };
    let index = index.unwrap_or(len);
    tx.apply(Edit::InsertEffect { layer: r.clone(), index, effect })?;
    Ok(index)
}

/// Switches effect `index` of a stack on or off.
pub fn set_effect_enabled(tx: &mut Transaction, r: &LayerRef, stack: StackId, index: usize, enabled: bool) -> Result<(), EditError> {
    let l = layer(tx, r)?;
    let effect = match stack {
        StackId::Content => l.effects.get(index).cloned().map(|mut e| {
            e.enabled = enabled;
            StackEffect::Content(e)
        }),
        StackId::Mask => l.mask.as_ref().and_then(|m| m.effects.get(index)).cloned().map(|mut e| {
            e.enabled = enabled;
            StackEffect::Mask(e)
        }),
    };
    let effect = effect.ok_or(EditError::IndexOutOfRange { index, len: 0 })?;
    tx.apply(Edit::ReplaceEffect { layer: r.clone(), index, effect })
}
