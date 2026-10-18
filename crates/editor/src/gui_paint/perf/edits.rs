//! (b) Single edits on big projects: each document is opened (every set composited), then one layer edit at a time
//! goes through the layer panel's path ([`stack_editor::apply`], then the recomposite of the sets it reaches) and
//! the time to the updated maps is measured: opacity, visibility, blend mode, a mask edit, an effect parameter and
//! an undo (which goes through the history's path).

use super::{Bench, ms};
use crate::gui_paint::editing::{Moved, history_move};
use crate::gui_paint::panels::layers::LayerEvent;
use crate::gui_paint::stack_editor;
use materialcraft::history::CoalesceKey;
use materialcraft::paint::edit::{Edit, LayerRef, StackEffect};
use materialcraft::paint::effect::EffectKind;
use materialcraft::paint::{BlendMode, Channel, Compositor, Layer, NO_FILL_CONTEXT, TextureSet};
use std::path::Path;
use std::time::Instant;

const DOCS: &str = "../Corpus/Converted/AS-replaced-replayed.mcpaint";

#[test]
#[ignore]
fn perf_layer_edits() {
    let list = std::env::var("MC_PERF_DOCS").unwrap_or_else(|_| DOCS.to_string());
    for path in list.split(':').filter(|p| !p.is_empty()).map(Path::new) {
        if path.exists() {
            edits_on(path);
        } else {
            eprintln!("skipped: {} not found", path.display());
        }
    }
}

fn edits_on(path: &Path) {
    let t = Instant::now();
    let loaded = materialcraft::paint::format::load_path(path).expect("load");
    let load = t.elapsed();
    let (mut b, opened) = Bench::open(loaded, path.to_path_buf());
    let sets = b.doc.doc().texture_sets.len();
    let stats = b.scene.stats();
    eprintln!(
        "{}: {sets} sets, loaded in {:.0} ms, all sets composited in {:.0} ms (compositing {:.0} ms, preview maps {:.0} ms)",
        path.display(),
        ms(load),
        ms(opened),
        ms(stats.composite),
        ms(stats.prepare)
    );
    let Some((set, layer)) = edit_target(b.doc.doc().texture_sets.as_slice()) else {
        eprintln!("  no layer to edit");
        return;
    };
    let ts = &b.doc.doc().texture_sets[set];
    let l = ts.layer(&layer).expect("layer");
    let channel = l.blend.first().map_or(Channel::BaseColor, |e| e.channel);
    let opacity = l.blend.first().map_or(1.0, |e| e.opacity);
    let mode = l.blend.first().map_or(BlendMode::Normal, |e| e.mode);
    eprintln!("  edited: set {} ({} layers), layer {:?} at depth 0, {} channels", ts.name, ts.walk_layers().count(), l.name, ts.channels.len());
    per_channel(ts);
    let r = LayerRef::new(ts.id.clone(), layer.clone());
    let masked = ts.walk_layers().find(|(_, l)| l.mask.is_some()).map(|(_, l)| LayerRef::new(ts.id.clone(), l.id.clone()));
    let key = CoalesceKey(0x5e7f);

    let mut edits: Vec<(&str, LayerEvent)> = vec![
        ("opacity", LayerEvent::SetOpacity { layer: r.clone(), channel, opacity: opacity * 0.5, key }),
        ("hide", LayerEvent::SetVisible(r.clone(), false)),
        ("show", LayerEvent::SetVisible(r.clone(), true)),
        (
            "blend mode",
            LayerEvent::SetBlendMode { layer: r.clone(), channel, mode: if mode == BlendMode::Multiply { BlendMode::Normal } else { BlendMode::Multiply } },
        ),
    ];
    if let Some(m) = masked {
        edits.push(("mask (invert)", LayerEvent::InvertMask(m)));
    }
    for (what, ev) in edits {
        let applied = stack_editor::apply(&mut b.doc, ev).expect("edit");
        let sets: Vec<usize> = applied.set.into_iter().chain(applied.also).collect();
        report(what, &mut b, sets);
        let _ = stack_editor::apply(&mut b.doc, LayerEvent::EndGesture);
    }
    if param_edit(&mut b, set) {
        report("effect parameter", &mut b, vec![set]);
    }
    // undo goes through the history path (`editing::move_in_history`)
    let (result, moved) = history_move(&mut b.doc, |d| d.undo().map(|_| ()));
    result.expect("undo");
    match moved {
        Moved::Changed(changed) => report("undo", &mut b, changed),
        Moved::SetsAddedOrRemoved => report("undo", &mut b, (0..sets).collect()),
    }
}

fn report(what: &str, b: &mut Bench, sets: Vec<usize>) {
    let asked = sets.len();
    let (took, stats) = b.refresh(sets);
    eprintln!(
        "  {what:<18} {:>8.1} ms to the updated maps; {asked} set(s) asked, {} composited: compositing {:.1} ms, preview maps {:.1} ms, upload {:.1} MB",
        ms(took),
        stats.sets,
        ms(stats.composite),
        ms(stats.prepare),
        stats.upload_bytes as f64 / 1e6
    );
}

/// The set with the most layers and its middle top-level layer with a blend entry (preferring one with a mask).
fn edit_target(sets: &[TextureSet]) -> Option<(usize, String)> {
    let (set, s) = sets.iter().enumerate().max_by_key(|(_, s)| s.walk_layers().count())?;
    let candidates: Vec<&Layer> = s.layers.iter().filter(|l| !l.blend.is_empty()).collect();
    let middle = candidates.len() / 2;
    let pick = candidates.iter().skip(middle).find(|l| l.mask.is_some()).or(candidates.get(middle))?;
    Some((set, pick.id.clone()))
}

/// Changes a numeric parameter of the first generator or filter in a mask of set `set` (one history step).
fn param_edit(b: &mut Bench, set: usize) -> bool {
    let ts = &b.doc.doc().texture_sets[set];
    let found = ts.walk_layers().find_map(|(_, l)| {
        let mask = l.mask.as_ref()?;
        mask.effects.iter().enumerate().find_map(|(i, e)| {
            let p = match &e.kind {
                EffectKind::Generator(p) | EffectKind::Filter(p) => p,
                _ => return None,
            };
            let k = p.params.iter().position(|(_, v)| v.parse::<f32>().is_ok())?;
            let mut changed = e.clone();
            if let EffectKind::Generator(q) | EffectKind::Filter(q) = &mut changed.kind {
                let v: f32 = q.params[k].1.parse().unwrap_or(0.0);
                q.params[k].1 = format!("{}", v * 0.9 + 0.01);
            }
            Some((LayerRef::new(ts.id.clone(), l.id.clone()), i, changed))
        })
    });
    let Some((layer, index, effect)) = found else { return false };
    b.doc.edit("Parameter", None, |tx| tx.apply(Edit::ReplaceEffect { layer, index, effect: StackEffect::Mask(effect) })).is_ok()
}

/// The full recompute of each channel of `set` through the core (no surface maps or material sources), for the
/// per-channel split.
fn per_channel(set: &TextureSet) {
    let c = Compositor::with_effects(set, materialcraft::generators::builtin()).with_fills(&NO_FILL_CONTEXT);
    let mut line = String::new();
    for spec in &set.channels {
        let t = Instant::now();
        let _ = c.composite_tex(spec.channel);
        line.push_str(&format!(" {:?} {:.0} ms,", spec.channel, ms(t.elapsed())));
    }
    eprintln!("  full recompute per channel (core):{line}");
}
