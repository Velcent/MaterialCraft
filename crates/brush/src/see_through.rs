//! See-through layers at work (PT-25, PT-29): [`repaint`] paints a see-through layer's recorded strokes again on
//! the composite below it, and [`SeeThroughCache`] keeps, per layer, what that composite was made of, so a layer is
//! painted again exactly when the layers below it changed (and then only once).
//!
//! A replay starts from the composite below ([`materialcraft::paint::see_through::base_images`]), runs every
//! recorded stroke in order through the same stroke session a live stroke uses, and frees the tiles no stroke
//! touched, so the layer holds pixels only where its strokes went. Repainting is derived state: it changes the
//! layer's images outside the history, like a cache, and undoing a change below repaints again.

use crate::BrushError;
use crate::document::Base;
use crate::replay::{ReplayScene, recorded_brush, recorded_values, replay};
use crate::target::mode_for;
use materialcraft::paint::see_through::{BelowKey, base_images, see_through_layers, trim_to};
use materialcraft::paint::stroke::StrokeRecord;
use materialcraft::paint::{ColorSpace, Compositor, EffectRegistry, FillContext, LayerKind, TextureSet};
use materialcraft::stamp::ChannelTarget;
use std::collections::HashMap;

/// The images of a see-through layer: `record` painted again on `base` (one image per channel of the layer, in
/// its order). `set` gives the channels' colour spaces.
pub fn repaint(set: &TextureSet, record: &StrokeRecord, base: &Base, scene: &ReplayScene) -> Result<Base, BrushError> {
    let mut work = base.clone();
    for stroke in &record.strokes {
        let brush = recorded_brush(stroke)?;
        let values = recorded_values(&brush);
        let mode = mode_for(stroke.tool, &brush.retouch);
        let mut targets: Vec<ChannelTarget> = work
            .iter_mut()
            .filter_map(|(c, image)| {
                let value = values.value(*c)?;
                let srgb = set.channel_spec(*c).map_or(c.default_color_space(), |s| s.color_space) == ColorSpace::Srgb;
                Some(ChannelTarget { image, value, mode, srgb })
            })
            .collect();
        if targets.is_empty() {
            continue;
        }
        replay(stroke, scene, &mut targets)?;
    }
    for ((_, img), (_, b)) in work.iter_mut().zip(base) {
        trim_to(img, b);
    }
    Ok(work)
}

/// How the composite below a layer is made: the generators and filters, and the fill context.
#[derive(Clone, Copy)]
pub struct BelowSource<'a> {
    pub effects: &'a EffectRegistry,
    pub fills: &'a FillContext<'a>,
}

impl BelowSource<'_> {
    fn base(&self, set: &TextureSet, layer: &str) -> Option<Base> {
        base_images(&Compositor::with_effects(set, self.effects).with_fills(self.fills), set, layer)
    }
}

struct Entry {
    key: BelowKey,
    base: Base,
}

/// What each see-through layer was last painted on, by texture set and layer id.
#[derive(Default)]
pub struct SeeThroughCache {
    entries: HashMap<(String, String), Entry>,
}

/// What a refresh did.
#[derive(Debug, Default, PartialEq)]
pub struct Refreshed {
    /// Layers painted again.
    pub repainted: Vec<String>,
    /// Layers that could not be painted again, with why (they keep their pixels).
    pub problems: Vec<(String, BrushError)>,
}

impl SeeThroughCache {
    /// Forgets everything (another document).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Whether layer `id` of `set` is up to date with the layers below it.
    pub fn is_current(&self, set: &TextureSet, id: &str) -> bool {
        let key = BelowKey::of(set, id);
        self.entries.get(&(set.id.clone(), id.to_string())).is_some_and(|e| Some(&e.key) == key.as_ref())
    }

    /// The composite below layer `id` as last painted on; [`Self::refresh`] first so it is current.
    pub fn base(&self, set: &TextureSet, id: &str) -> Option<&Base> {
        self.entries.get(&(set.id.clone(), id.to_string())).map(|e| &e.base)
    }

    /// Paints every see-through layer of `set` whose layers below changed since it was last painted (bottom to top,
    /// so a see-through layer above another sees it painted). Layers seen for the first time are painted too: what a
    /// file stored may have been painted on other layers below.
    pub fn refresh(&mut self, set: &mut TextureSet, below: BelowSource, scene: &ReplayScene) -> Refreshed {
        let mut out = Refreshed::default();
        for id in see_through_layers(set) {
            let Some(key) = BelowKey::of(set, &id) else { continue };
            let slot = (set.id.clone(), id.clone());
            if self.entries.get(&slot).is_some_and(|e| e.key == key) {
                continue;
            }
            let Some(base) = below.base(set, &id) else { continue };
            let record = match set.layer(&id).map(|l| &l.kind) {
                Some(LayerKind::Paint(p)) => p.strokes.as_ref().and_then(|s| s.record()).cloned().unwrap_or_default(),
                _ => continue,
            };
            match repaint(set, &record, &base, scene) {
                Ok(images) => {
                    if let Some(LayerKind::Paint(p)) = set.layer_mut(&id).map(|l| &mut l.kind) {
                        for ((_, img), (_, new)) in p.images.iter_mut().zip(images) {
                            *img = new;
                        }
                    }
                    out.repainted.push(id.clone());
                }
                Err(e) => out.problems.push((id.clone(), e)),
            }
            // also after a problem: the same stroke would fail again until something changes
            self.entries.insert(slot, Entry { key, base });
        }
        out
    }
}
