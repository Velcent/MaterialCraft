//! Playing a list of [`EditKind`]s: single layer edits through the layer panel's path ([`stack_editor::apply`],
//! then the recomposite of the sets it reaches), undo and redo through the history's path, each timed to the
//! updated maps (step 51's edit benchmark, generalised). The edits act on the middle top-level layer of the set
//! with the most layers; mask edits on that set's first masked layer, effect edits on its first effect.

use super::Bench;
use super::measure::{Recomposite, put_all, put_group};
use crate::gui_paint::panels::layers::LayerEvent;
use crate::gui_paint::stack_editor;
use materialcraft::history::CoalesceKey;
use materialcraft::paint::edit::{Edit, LayerRef, StackEffect, StackId};
use materialcraft::paint::effect::EffectKind;
use materialcraft::paint::{BlendMode, Channel, Compositor, Layer, NO_FILL_CONTEXT, TextureSet};
use materialcraft_perf::report::ms;
use materialcraft_perf::{EditKind, RunReport};
use std::time::Instant;

/// What the edits act on.
struct Targets {
    set: usize,
    layer: LayerRef,
    masked: Option<LayerRef>,
    /// A layer with an effect: the stack and the effect's index.
    effect: Option<(LayerRef, StackId, usize)>,
}

/// Plays `edits` `repeat` times; edits that can't be made on this document are noted and skipped.
pub fn play(b: &mut Bench, edits: &[EditKind], repeat: u32, r: &mut RunReport) -> Result<(), String> {
    let t = targets(b.doc.doc().texture_sets.as_slice()).ok_or("no layer to edit")?;
    let ts = b.doc.doc().texture_sets.get(t.set).ok_or("no such set")?;
    let name = ts.layer(&t.layer.layer).map_or(String::new(), |l| l.name.clone());
    r.note(format!("set {:?} ({} layers, {} channels), layer {name:?}", ts.name, ts.walk_layers().count(), ts.channels.len()));
    per_channel(ts, r);

    let mut all = Vec::new();
    let mut by_kind: Vec<(EditKind, Vec<Recomposite>)> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();
    for _ in 0..repeat {
        for &kind in edits {
            let done = match edit(b, &t, kind) {
                Ok(done) => done,
                Err(why) => {
                    if !missing.contains(&kind.name()) {
                        missing.push(kind.name());
                        r.note(format!("{}: {why}", kind.name()));
                    }
                    continue;
                }
            };
            all.extend(done.iter().copied());
            match by_kind.iter_mut().find(|(k, _)| *k == kind) {
                Some((_, list)) => list.extend(done),
                None => by_kind.push((kind, done)),
            }
        }
    }
    for (kind, list) in &by_kind {
        put_group(r, &format!("edit.{}", kind.name()), list);
    }
    put_all(r, &all);
    if all.is_empty() {
        return Err("none of the edits could be made".into());
    }
    Ok(())
}

/// Makes one edit (two for the toggles) and measures each recomposite.
fn edit(b: &mut Bench, t: &Targets, kind: EditKind) -> Result<Vec<Recomposite>, String> {
    let r = t.layer.clone();
    let ts = b.doc.doc().texture_sets.get(t.set).ok_or("no such set")?;
    let l = ts.layer(&r.layer).ok_or("the layer is gone")?;
    let channel = l.blend.first().map_or(Channel::BaseColor, |e| e.channel);
    let opacity = l.blend.first().map_or(1.0, |e| e.opacity);
    let mode = l.blend.first().map_or(BlendMode::Normal, |e| e.mode);
    let key = CoalesceKey(0x5e7f);
    let events: Vec<LayerEvent> = match kind {
        EditKind::Opacity => vec![LayerEvent::SetOpacity { layer: r, channel, opacity: opacity * 0.5, key }],
        EditKind::Hide => vec![LayerEvent::SetVisible(r, false)],
        EditKind::Show => vec![LayerEvent::SetVisible(r, true)],
        EditKind::BlendMode => {
            vec![LayerEvent::SetBlendMode { layer: r, channel, mode: if mode == BlendMode::Multiply { BlendMode::Normal } else { BlendMode::Multiply } }]
        }
        EditKind::Mask => vec![LayerEvent::InvertMask(t.masked.clone().ok_or("no masked layer")?)],
        EditKind::MaskToggle => {
            let m = t.masked.clone().ok_or("no masked layer")?;
            vec![LayerEvent::SetMaskEnabled(m.clone(), false), LayerEvent::SetMaskEnabled(m, true)]
        }
        EditKind::EffectToggle => {
            let (layer, stack, index) = t.effect.clone().ok_or("no effect")?;
            vec![
                LayerEvent::SetEffectEnabled { layer: layer.clone(), stack, index, enabled: false },
                LayerEvent::SetEffectEnabled { layer, stack, index, enabled: true },
            ]
        }
        EditKind::EffectParam => {
            if !param_edit(b, t.set) {
                return Err("no effect with a numeric parameter".into());
            }
            return Ok(vec![Recomposite::refresh(b, vec![t.set])]);
        }
        EditKind::Undo => return Ok(vec![Recomposite::history(b, |d| d.undo().map(|_| ())).map_err(|e| e.to_string())?]),
        EditKind::Redo => return Ok(vec![Recomposite::history(b, |d| d.redo().map(|_| ())).map_err(|e| e.to_string())?]),
    };
    let mut done = Vec::new();
    for ev in events {
        let applied = stack_editor::apply(&mut b.doc, ev).map_err(|e| e.to_string())?;
        done.push(Recomposite::refresh(b, applied.set.into_iter().chain(applied.also).collect()));
        let _ = stack_editor::apply(&mut b.doc, LayerEvent::EndGesture);
    }
    Ok(done)
}

/// The set with the most layers and its middle top-level layer with a blend entry (preferring one with a mask).
fn targets(sets: &[TextureSet]) -> Option<Targets> {
    let (set, s) = sets.iter().enumerate().max_by_key(|(_, s)| s.walk_layers().count())?;
    let candidates: Vec<&Layer> = s.layers.iter().filter(|l| !l.blend.is_empty()).collect();
    let middle = candidates.len() / 2;
    let pick = candidates.iter().skip(middle).find(|l| l.mask.is_some()).or(candidates.get(middle))?;
    let at = |l: &Layer| LayerRef::new(s.id.clone(), l.id.clone());
    let masked = s.walk_layers().find(|(_, l)| l.mask.is_some()).map(|(_, l)| at(l));
    let effect = s.walk_layers().find_map(|(_, l)| match &l.mask {
        Some(m) if !m.effects.is_empty() => Some((at(l), StackId::Mask, 0)),
        _ if !l.effects.is_empty() => Some((at(l), StackId::Content, 0)),
        _ => None,
    });
    Some(Targets { set, layer: at(pick), masked, effect })
}

/// Changes a numeric parameter of the first generator or filter in a mask of set `set` (one history step).
fn param_edit(b: &mut Bench, set: usize) -> bool {
    let Some(ts) = b.doc.doc().texture_sets.get(set) else { return false };
    let found = ts.walk_layers().find_map(|(_, l)| {
        let mask = l.mask.as_ref()?;
        mask.effects.iter().enumerate().find_map(|(i, e)| {
            let p = match &e.kind {
                EffectKind::Generator(p) | EffectKind::Filter(p) => p,
                _ => return None,
            };
            let k = p.params.iter().position(|(_, v)| v.parse::<f32>().is_ok())?;
            let mut changed = e.clone();
            if let EffectKind::Generator(q) | EffectKind::Filter(q) = &mut changed.kind
                && let Some((_, value)) = q.params.get_mut(k)
            {
                let v: f32 = value.parse().unwrap_or(0.0);
                *value = format!("{}", v * 0.9 + 0.01);
            }
            Some((LayerRef::new(ts.id.clone(), l.id.clone()), i, changed))
        })
    });
    let Some((layer, index, effect)) = found else { return false };
    b.doc.edit("Parameter", None, |tx| tx.apply(Edit::ReplaceEffect { layer, index, effect: StackEffect::Mask(effect) })).is_ok()
}

/// The full recompute of each channel of `set` through the core (no surface maps or material sources), for the
/// per-channel split: `core.<channel>_ms`.
fn per_channel(set: &TextureSet, r: &mut RunReport) {
    let c = Compositor::with_effects(set, materialcraft::generators::builtin()).with_fills(&NO_FILL_CONTEXT);
    for spec in &set.channels {
        let t = Instant::now();
        let _ = c.composite_tex(spec.channel);
        r.put(format!("core.{:?}_ms", spec.channel), ms(t.elapsed()));
    }
}
