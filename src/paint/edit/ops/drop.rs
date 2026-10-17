//! Dropping an asset onto the layer stack (LS-18, MK-16). The target decides (ADR 0008, q-effect-drop):
//!
//! - an effect **between layers** becomes a new effect layer that adjusts everything below it;
//! - an effect **onto a layer** is added on top of the layer's content effects;
//! - an effect **onto a mask** is added on top of the mask's effects;
//! - with `replace` (Ctrl held) it replaces that stack's effects instead of being added.
//!
//! A smart mask (a mask's effects saved as an asset) dropped onto a layer or its mask gives the layer a black
//! mask if it has none and adds its effects; with `replace` they replace the mask's effects (MK-16). Its anchors
//! get ids new in the texture set and its painted pixels the set's resolution.
//!
//! A material and a smart material make layers (LS-18, see [`super::drop_layers`]): a material becomes a fill
//! layer, a smart material a folder. Between layers it lands there; onto a folder on top of its children; onto
//! another layer directly above it, except that a material dropped onto a fill layer becomes that layer's fill.
//!
//! The asset tray builds a [`DropPayload`]; the layer panel turns the pointer position into a
//! [`DropTarget`], asks [`accepts`] while hovering and calls [`drop_onto`] on release.

use super::super::ids::IdPool;
use super::super::{Edit, EditError, LayerProperty, LayerRef, LayerSlot, StackEffect, StackId, Transaction};
use super::drop_layers::{self, MaterialAsset};
use super::{channels, layer, set};
use crate::paint::blend::BlendMode;
use crate::paint::doc::Document;
use crate::paint::effect::{ColorSelection, ContentEffect, EffectDomain, EffectKind, GreyBlend, Levels, MaskEffect, Procedural};
use crate::paint::fragment::adapt;
use crate::paint::layer::{Layer, LayerKind, PaintContent};
use crate::paint::mask::Mask;
use crate::paint::texture_set::TextureSet;

/// Where on the stack something is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropTarget {
    /// Between two layers (or at either end, or into a folder): a new layer goes to `at`.
    Between { set: String, at: LayerSlot },
    /// Onto a layer's row (its content).
    OntoLayer(LayerRef),
    /// Onto a layer's mask thumbnail.
    OntoMask(LayerRef),
}

impl DropTarget {
    pub fn set(&self) -> &str {
        match self {
            DropTarget::Between { set, .. } => set,
            DropTarget::OntoLayer(r) | DropTarget::OntoMask(r) => &r.set,
        }
    }
}

/// An effect as an asset: the same in either stack, made into a content or mask effect where it lands.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectAsset {
    /// Shown as the name of a new effect layer and as the effect's label.
    pub name: String,
    pub kind: AssetEffect,
}

/// The effect kinds an asset can be.
#[derive(Debug, Clone, PartialEq)]
pub enum AssetEffect {
    Generator(Procedural),
    Filter(Procedural),
    Levels(Levels),
    ColorSelection(ColorSelection),
}

impl EffectAsset {
    fn kind<D: EffectDomain>(&self) -> EffectKind<D> {
        match &self.kind {
            AssetEffect::Generator(p) => EffectKind::Generator(p.clone()),
            AssetEffect::Filter(p) => EffectKind::Filter(p.clone()),
            AssetEffect::Levels(l) => EffectKind::Levels(*l),
            AssetEffect::ColorSelection(c) => EffectKind::ColorSelection(c.clone()),
        }
    }

    /// The content effect: every channel of `set` with `mode` at full opacity.
    pub fn content(&self, set: &TextureSet, mode: BlendMode) -> ContentEffect {
        let mut e = ContentEffect::new(Vec::new(), self.kind());
        e.label = self.name.clone();
        for c in channels(set) {
            e = e.with_blend(c, mode, 1.0);
        }
        e
    }

    pub fn mask(&self) -> MaskEffect {
        let mut e = MaskEffect::new(GreyBlend::default(), self.kind());
        e.label = self.name.clone();
        e
    }
}

/// What can be dropped onto the stack.
#[derive(Debug, Clone, PartialEq)]
pub enum DropPayload {
    Effect(EffectAsset),
    /// A mask's effects, bottom to top.
    SmartMask(Vec<MaskEffect>),
    /// A material: becomes a fill layer (or the fill of a fill layer it is dropped onto).
    Material(MaterialAsset),
    /// A smart material: a folder named `name` holding `layers` (bottom to top). Where they land their ids are
    /// made unique, their images are resampled to the texture set's resolution and channels it does not declare are
    /// left out. `asset` is the library reference of the smart material, recorded on the folder (its provenance).
    Folder {
        name: String,
        layers: Vec<Layer>,
        asset: Option<String>,
    },
}

/// What a drop did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropOutcome {
    /// A new layer with this id.
    NewLayer(String),
    /// Effects went into this stack of the layer (added on top, or replacing what was there).
    Effects { layer: LayerRef, stack: StackId, replaced: bool },
    /// The fill layer's fill now comes from the dropped material.
    Filled(LayerRef),
}

/// True if dropping `payload` onto `target` would do something (for the hover marker).
pub fn accepts(doc: &Document, target: &DropTarget, payload: &DropPayload) -> bool {
    let Some(set) = doc.texture_set(target.set()) else { return false };
    match (target, payload) {
        (DropTarget::Between { at, .. }, DropPayload::Effect(_) | DropPayload::Material(_) | DropPayload::Folder { .. }) => match &at.parent {
            None => at.index <= set.layers.len(),
            Some(p) => set.layer(p).is_some_and(|g| matches!(g.kind, LayerKind::Group(_)) && at.index <= g.children().len()),
        },
        (DropTarget::Between { .. }, DropPayload::SmartMask(_)) => false,
        (DropTarget::OntoLayer(r), _) => set.layer(&r.layer).is_some(),
        (DropTarget::OntoMask(r), DropPayload::Effect(_)) => set.layer(&r.layer).is_some_and(|l| l.mask.is_some()),
        (DropTarget::OntoMask(r), DropPayload::SmartMask(_)) => set.layer(&r.layer).is_some(),
        (DropTarget::OntoMask(_), DropPayload::Material(_) | DropPayload::Folder { .. }) => false,
    }
}

/// Drops `payload` onto `target`; `replace` is the Ctrl modifier. See the module docs for the rules.
pub fn drop_onto(tx: &mut Transaction, target: &DropTarget, payload: &DropPayload, replace: bool) -> Result<DropOutcome, EditError> {
    match (target, payload) {
        (DropTarget::Between { set: set_id, at }, DropPayload::Effect(asset)) => {
            let s = set(tx, set_id)?;
            let layer = effect_layer(s, asset);
            let id = layer.id.clone();
            tx.apply(Edit::InsertLayer { set: set_id.clone(), at: at.clone(), layer: Box::new(layer) })?;
            Ok(DropOutcome::NewLayer(id))
        }
        (DropTarget::Between { .. }, DropPayload::SmartMask(_)) => Err(EditError::NotAccepted("a smart mask goes onto a layer or its mask")),
        (DropTarget::OntoLayer(r), DropPayload::Effect(asset)) => {
            let effect = asset.content(set(tx, &r.set)?, BlendMode::Normal);
            into_stack(tx, r, vec![StackEffect::Content(effect)], replace)
        }
        (DropTarget::OntoMask(r), DropPayload::Effect(asset)) => {
            if layer(tx, r)?.mask.is_none() {
                return Err(EditError::NoMask { set: r.set.clone(), layer: r.layer.clone() });
            }
            into_stack(tx, r, vec![StackEffect::Mask(asset.mask())], replace)
        }
        (DropTarget::OntoLayer(r) | DropTarget::OntoMask(r), DropPayload::SmartMask(effects)) => {
            let s = set(tx, &r.set)?;
            // The smart mask's anchors get ids new in the set (references among its effects follow) and its painted
            // pixels the set's resolution.
            let mut effects = effects.clone();
            IdPool::of(s).reid_effects(&mut effects);
            adapt::fit_mask_effects(&mut effects, s.resolution);
            if layer(tx, r)?.mask.is_none() {
                tx.apply(Edit::SetLayer { layer: r.clone(), property: LayerProperty::Mask(Some(Mask::new(0.0))) })?;
            }
            into_stack(tx, r, effects.into_iter().map(StackEffect::Mask).collect(), replace)
        }
        (_, DropPayload::Material(material)) => drop_layers::material(tx, target, material),
        (_, DropPayload::Folder { name, layers, asset }) => drop_layers::folder(tx, target, name, layers, asset.as_deref()),
    }
}

/// An effect layer: an empty layer whose channels are all set to pass through, with the effect on its
/// content stack, so the effect works on the composite below it. This is the form the importer gives
/// Painter's filter-only layers too.
fn effect_layer(set: &TextureSet, asset: &EffectAsset) -> Layer {
    let id = IdPool::of(set).layer("effect");
    let name = if asset.name.is_empty() { "Effect".to_string() } else { asset.name.clone() };
    let mut layer = Layer::new(id, name, LayerKind::Paint(PaintContent::default())).with_effect(asset.content(set, BlendMode::Normal));
    for c in channels(set) {
        layer.set_blend(c, BlendMode::Passthrough, 1.0);
    }
    layer
}

/// Adds `effects` on top of their stack, or replaces the stack's effects with them.
fn into_stack(tx: &mut Transaction, r: &LayerRef, effects: Vec<StackEffect>, replace: bool) -> Result<DropOutcome, EditError> {
    let stack = match effects.first() {
        Some(e) => e.stack(),
        None => return Err(EditError::NotAccepted("there are no effects to add")),
    };
    let l = layer(tx, r)?;
    let mut len = match stack {
        StackId::Content => l.effects.len(),
        StackId::Mask => l.mask.as_ref().map_or(0, |m| m.effects.len()),
    };
    if replace {
        while len > 0 {
            len -= 1;
            tx.apply(Edit::RemoveEffect { layer: r.clone(), stack, index: len })?;
        }
    }
    for e in effects {
        tx.apply(Edit::InsertEffect { layer: r.clone(), index: len, effect: e })?;
        len += 1;
    }
    Ok(DropOutcome::Effects { layer: r.clone(), stack, replaced: replace })
}
