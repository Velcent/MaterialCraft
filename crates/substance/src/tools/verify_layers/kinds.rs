//! [`NativeKinds`]: which of the native effect kinds (levels, anchor references, colour selection, polygon fill,
//! several fills in one layer, effect groups with their own blending, effect layers) each channel of a set uses,
//! over the visible layers that blend the channel; and [`KindTally`], the compared and reproduced channels per
//! kind over the corpus, so `verify-layers` reports what each kind contributes.

use super::ChannelResult;
use crate::model::{
    LayerKind, PainterBlendMode, PainterEffect, PainterLayer, PainterSource, PainterTextureSet, blending_for,
};
use crate::translate::layers::SetChannels;
use crate::translate::layers::cache_score::EXACT_STEPS;
use materialcraft::paint::Channel;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

pub const LEVELS: &str = "levels";
pub const REFERENCE: &str = "anchor reference";
pub const COLOUR_SELECTION: &str = "colour selection";
pub const POLYGON_FILL: &str = "polygon fill";
pub const MULTI_FILL: &str = "several fills in one layer";
pub const GROUP_BLENDING: &str = "effect group with its own blending";
pub const EFFECT_LAYER: &str = "effect layer (pass-through layer with effects)";

/// The native kinds per channel of one set.
#[derive(Debug, Default)]
pub struct NativeKinds(HashMap<Channel, BTreeSet<&'static str>>);

impl NativeKinds {
    pub fn of(set: &PainterTextureSet, channels: &SetChannels) -> NativeKinds {
        let mut out = NativeKinds::default();
        out.walk(&set.layers, channels);
        out
    }

    /// True if any channel uses `kind`.
    pub fn uses(&self, kind: &str) -> bool {
        self.0.values().any(|k| k.contains(kind))
    }

    pub fn get(&self, channel: Channel) -> impl Iterator<Item = &'static str> + '_ {
        self.0.get(&channel).into_iter().flatten().copied()
    }

    fn walk(&mut self, layers: &[PainterLayer], channels: &SetChannels) {
        for l in layers.iter().filter(|l| l.visible) {
            let mut kinds = BTreeSet::new();
            effect_kinds(&l.content, false, &mut kinds);
            if let Some(m) = l.mask.as_ref().filter(|m| m.enabled) {
                effect_kinds(&m.effects, true, &mut kinds);
            }
            let fills = l
                .content
                .iter()
                .filter(|e| matches!(e, PainterEffect::Fill(_)) && e.is_enabled())
                .count();
            if fills > 1 {
                kinds.insert(MULTI_FILL);
            }
            for channel in channels.all() {
                let b = blending_for(&l.blending, channels.painter(channel));
                if b.is_some_and(|b| b.mode == PainterBlendMode::Disable || b.opacity <= 0.0) {
                    continue;
                }
                let entry = self.0.entry(channel).or_default();
                entry.extend(kinds.iter().copied());
                let passthrough = b.is_some_and(|b| b.mode == PainterBlendMode::Passthrough);
                if passthrough && !matches!(l.kind, LayerKind::Group(_)) && !l.content.is_empty() {
                    entry.insert(EFFECT_LAYER);
                }
            }
            if let LayerKind::Group(children) = &l.kind {
                self.walk(children, channels);
            }
        }
    }
}

fn effect_kinds(effects: &[PainterEffect], in_mask: bool, out: &mut BTreeSet<&'static str>) {
    for e in effects.iter().filter(|e| e.is_enabled()) {
        match e {
            PainterEffect::Levels(_) => {
                out.insert(LEVELS);
            }
            PainterEffect::ColorSelection(_) => {
                out.insert(COLOUR_SELECTION);
            }
            PainterEffect::PolygonFill(_) => {
                out.insert(POLYGON_FILL);
            }
            PainterEffect::Fill(f) if f.sources.iter().any(|s| matches!(s, PainterSource::Reference(_))) => {
                out.insert(REFERENCE);
            }
            PainterEffect::Group(g) => {
                let own = g
                    .header
                    .blending
                    .iter()
                    .any(|b| b.mode != PainterBlendMode::Passthrough);
                if own && !in_mask && !g.effects.is_empty() {
                    out.insert(GROUP_BLENDING);
                }
                effect_kinds(&g.effects, in_mask, out);
            }
            _ => {}
        }
    }
}

/// Per kind: channels using it, how many of them were compared and reproduced, and sets whose every channel
/// was compared and reproduced while using it somewhere.
#[derive(Debug, Default)]
pub struct KindTally {
    kinds: BTreeMap<&'static str, [usize; 3]>,
    sets: BTreeMap<&'static str, usize>,
}

impl KindTally {
    /// One set's channels with the kinds each uses.
    pub fn add(&mut self, kinds: &NativeKinds, channels: &[ChannelResult]) {
        let mut used = BTreeSet::new();
        for c in channels {
            for k in kinds.get(c.channel) {
                used.insert(k);
                let t = self.kinds.entry(k).or_default();
                t[0] += 1;
                if let Some(s) = c.score {
                    t[1] += 1;
                    t[2] += usize::from(s.reproduced());
                }
            }
        }
        let all = !channels.is_empty() && channels.iter().all(|c| c.score.is_some_and(|s| s.reproduced()));
        for k in used.into_iter().filter(|_| all) {
            *self.sets.entry(k).or_default() += 1;
        }
    }

    pub fn merge(&mut self, other: KindTally) {
        for (k, v) in other.kinds {
            let t = self.kinds.entry(k).or_default();
            (0..3).for_each(|i| t[i] += v[i]);
        }
        for (k, n) in other.sets {
            *self.sets.entry(k).or_default() += n;
        }
    }
}

impl fmt::Display for KindTally {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "native effect kinds (channels of visible layers using the kind; reproduced = mean ≤ {EXACT_STEPS}):"
        )?;
        for (k, [used, compared, ok]) in &self.kinds {
            let sets = self.sets.get(k).copied().unwrap_or(0);
            writeln!(
                f,
                "  {k}: {used} channels, {compared} compared, {ok} reproduced; {sets} sets fully reproduced"
            )?;
        }
        Ok(())
    }
}
