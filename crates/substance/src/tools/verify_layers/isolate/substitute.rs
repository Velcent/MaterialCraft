//! [`apply`]: a copy of a Painter texture set with slots swapped for constants, ready for the ordinary layer
//! import. Everything swapped in is something the importer computes exactly (uniform fills at normal blending),
//! so the import's own verdict says whether the rest of the stack is computable.
//!
//! - [`Swap::Output`]: a candidate's output becomes the constant `v` and keeps the candidate's blending (a mask
//!   generator becomes a grey fill with its blend; a procedural content fill a uniform fill of `v` in the
//!   channels it fed; a filter a fill of `v` replacing what is below at the filter's opacity).
//! - [`Swap::Nuisance`]: a mask effect becomes black or white at full strength (overriding what is below it);
//!   a layer is switched off, or its content becomes black or white at normal blending, full opacity; a whole
//!   mask becomes black or white; a layer with unknown content and mask is switched off or black or white
//!   without a mask. Across these states the nuisance takes the extremes of everything it could do, so a texel
//!   whose composite does not change across them does not depend on it (the layers above respond monotonically
//!   to what is below them for every blend mode the importer identifies).

use super::slots::{EffectAddress, LayerAddress, Nuisance, Place};
use crate::model::{
    ChannelBlending, ChannelMask, EffectHeader, FillEffect, FillPlacement, LayerKind, MaskInitial, PainterBlendMode,
    PainterChannel, PainterEffect, PainterLayer, PainterMask, PainterSource, PainterTextureSet, UniformSource,
};
use crate::translate::layers::SetChannels;

#[derive(Debug, Clone, PartialEq)]
pub enum Swap {
    /// The candidate at this address writes `v` everywhere.
    Output(EffectAddress, f32),
    /// The nuisance in state `n` (`0..states()`).
    Nuisance(Nuisance, usize),
}

/// The set with every swap applied (addresses that do not resolve are ignored).
pub fn apply(set: &PainterTextureSet, swaps: &[Swap], channels: &SetChannels) -> PainterTextureSet {
    let mut out = set.clone();
    // effect-level swaps first: a layer nuisance replaces the whole content afterwards
    let mut ordered: Vec<&Swap> = swaps.iter().collect();
    ordered.sort_by_key(|s| match s {
        Swap::Nuisance(Nuisance::Layer(_) | Nuisance::Mask(_) | Nuisance::LayerWithMask(_), _) => 1,
        _ => 0,
    });
    for swap in ordered {
        match swap {
            Swap::Output(at, v) => output(&mut out, at, *v, channels),
            Swap::Nuisance(Nuisance::MaskEffect(at), n) => {
                if let Some(e) = effect_mut(&mut out, at) {
                    *e = PainterEffect::Fill(mask_fill(header_of(e), *n as f32, 1.0));
                }
            }
            Swap::Nuisance(Nuisance::Layer(at), n) => {
                if let Some(l) = layer_mut(&mut out, at) {
                    layer_state(l, *n, channels);
                }
            }
            Swap::Nuisance(Nuisance::Mask(at), n) => {
                if let Some(l) = layer_mut(&mut out, at) {
                    l.mask = Some(PainterMask {
                        enabled: true,
                        initial: if *n == 0 {
                            MaskInitial::Black
                        } else {
                            MaskInitial::White
                        },
                        effects: Vec::new(),
                    });
                }
            }
            Swap::Nuisance(Nuisance::LayerWithMask(at), n) => {
                if let Some(l) = layer_mut(&mut out, at) {
                    layer_state(l, *n, channels);
                    l.mask = None;
                }
            }
        }
    }
    out
}

fn output(set: &mut PainterTextureSet, at: &EffectAddress, v: f32, channels: &SetChannels) {
    let in_mask = matches!(at.place, Place::Mask(_));
    let Some(e) = effect_mut(set, at) else { return };
    match e {
        PainterEffect::Fill(f) if in_mask => f.sources = vec![uniform(ChannelMask(0), [v; 3])],
        PainterEffect::Fill(f) => {
            let mut sources = Vec::new();
            for s in std::mem::take(&mut f.sources) {
                match s {
                    PainterSource::Procedural(p) => {
                        for c in channels.all() {
                            let painter = channels.painter(c);
                            if p.channels.contains(painter) || p.channels.is_all() {
                                sources.push(uniform(ChannelMask(painter.bit()), value_for(painter, v)));
                            }
                        }
                    }
                    other => sources.push(other),
                }
            }
            f.sources = sources;
        }
        PainterEffect::Filter(filter) if in_mask => {
            let opacity = mask_opacity(&filter.header.blending);
            *e = PainterEffect::Fill(mask_fill(header_of(e), v, opacity));
        }
        PainterEffect::Filter(filter) => {
            let header = filter.header.clone();
            let mut blending: Vec<ChannelBlending> = header
                .blending
                .iter()
                .map(|b| ChannelBlending {
                    mode: PainterBlendMode::Normal,
                    ..*b
                })
                .collect();
            blending.push(normal(ChannelMask(u64::MAX), 1.0));
            let written = channels
                .all()
                .filter(|c| header.channels.is_empty() || channels.mask_has(header.channels, *c));
            let sources = written
                .map(|c| channels.painter(c))
                .map(|p| uniform(ChannelMask(p.bit()), value_for(p, v)))
                .collect();
            *e = PainterEffect::Fill(fill(EffectHeader { blending, ..header }, sources));
        }
        _ => {}
    }
}

/// State 0 switches the layer off; 1 and 2 make its content black or white at normal blending.
fn layer_state(l: &mut PainterLayer, n: usize, channels: &SetChannels) {
    if n == 0 {
        l.visible = false;
        return;
    }
    let v = if n == 1 { 0.0 } else { 1.0 };
    let all = ChannelMask(u64::MAX);
    let sources = channels
        .all()
        .map(|c| channels.painter(c))
        .map(|p| uniform(ChannelMask(p.bit()), value_for(p, v)))
        .collect();
    let header = EffectHeader {
        uid: None,
        enabled: true,
        label: String::new(),
        channels: all,
        blending: vec![normal(all, 1.0)],
    };
    l.kind = LayerKind::Fill;
    l.content = vec![PainterEffect::Fill(fill(header, sources))];
    l.blending = vec![normal(all, 1.0)];
}

/// `v` for every component; the normal channel stays flat (normals are not measured).
fn value_for(channel: PainterChannel, v: f32) -> [f32; 3] {
    match channel {
        PainterChannel::Normal => [0.5, 0.5, 1.0],
        _ => [v; 3],
    }
}

fn uniform(channels: ChannelMask, color: [f32; 3]) -> PainterSource {
    PainterSource::Uniform(UniformSource {
        uid: None,
        channels,
        color,
        opacity: 1.0,
    })
}

fn normal(channels: ChannelMask, opacity: f32) -> ChannelBlending {
    ChannelBlending {
        channels,
        mode: PainterBlendMode::Normal,
        opacity,
    }
}

/// A grey mask fill of `v` replacing what is below at `opacity`, keeping the effect's uid and switch.
fn mask_fill(header: EffectHeader, v: f32, opacity: f32) -> FillEffect {
    let header = EffectHeader {
        channels: ChannelMask(0),
        blending: vec![normal(ChannelMask(0), opacity)],
        ..header
    };
    fill(header, vec![uniform(ChannelMask(0), [v; 3])])
}

fn fill(header: EffectHeader, sources: Vec<PainterSource>) -> FillEffect {
    FillEffect {
        header,
        sources,
        projection: 0,
        filtering: 2,
        uv_transform: None,
        placement: FillPlacement::default(),
    }
}

/// The grey entry's opacity, as the mask import reads it.
fn mask_opacity(entries: &[ChannelBlending]) -> f32 {
    entries
        .iter()
        .find(|b| b.channels.is_empty())
        .or(entries.first())
        .map_or(1.0, |b| b.opacity)
}

fn header_of(e: &PainterEffect) -> EffectHeader {
    let mut header = e.header().cloned().unwrap_or_default();
    header.enabled = true;
    header
}

pub fn layer_mut<'a>(set: &'a mut PainterTextureSet, at: &LayerAddress) -> Option<&'a mut PainterLayer> {
    let (first, rest) = at.0.split_first()?;
    let mut layer = set.layers.get_mut(*first)?;
    for i in rest {
        layer = match &mut layer.kind {
            LayerKind::Group(children) => children.get_mut(*i)?,
            _ => return None,
        };
    }
    Some(layer)
}

fn effect_mut<'a>(set: &'a mut PainterTextureSet, at: &EffectAddress) -> Option<&'a mut PainterEffect> {
    let layer = layer_mut(set, &at.layer)?;
    match at.place {
        Place::Content(i) => layer.content.get_mut(i),
        Place::Mask(i) => layer.mask.as_mut()?.effects.get_mut(i),
    }
}
