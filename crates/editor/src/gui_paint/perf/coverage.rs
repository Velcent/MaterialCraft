//! How many texture sets, and how many of their paintable targets, a brush stroke updates tile by tile: over the
//! projects of a list (`MC_COVERAGE_LIST`, default the feature-cover list), imported as the editor imports them,
//! with Painter's procedurals kept and with them replaced: each target asks [`MaterialMaps::tiles_reached`] for a
//! stroke in the middle of the set. The reasons sets fall back to the whole set are counted. Ignored; run under the
//! heavy lock (`perf_coverage`). (The "before" column of the step-51 research note, the first tile path's rule of no
//! filter, generator or reference in the stack, is history: that rule is gone.)

use crate::gui_paint::channel_display::ChannelDisplay;
use crate::gui_paint::channel_view::{ChannelView, Shown};
use crate::gui_paint::maps_patch;
use materialcraft::generators;
use materialcraft::paint::effect::{EvalTarget, Reach};
use materialcraft::paint::output_tiles::WholeReason;
use materialcraft::paint::{EffectKind, Layer, LayerKind, MaterialMaps, NO_FILL_CONTEXT, OutputOptions, TILE_SIZE, TextureSet};
use materialcraft_substance::translate::replacements::{Choice, ProceduralPlan};
use materialcraft_substance::translate::{Cancel, import_spp_with};
use std::collections::BTreeMap;
use std::path::Path;

const LIST: &str = "../MaterialCraft-docs/corpus/feature-cover.txt";

#[derive(Default)]
struct Tally {
    projects: usize,
    failed: usize,
    sets: usize,
    /// Sets where every target takes the tile path / at least one does.
    sets_all: usize,
    sets_some: usize,
    targets: usize,
    targets_after: usize,
    reasons: BTreeMap<String, usize>,
}

#[test]
#[ignore]
fn perf_coverage() {
    let list = std::env::var("MC_COVERAGE_LIST").unwrap_or_else(|_| LIST.to_string());
    let Ok(text) = std::fs::read_to_string(&list) else {
        eprintln!("skipped: {list} not found");
        return;
    };
    let paths: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    for (name, plan) in [("procedurals kept", ProceduralPlan::default()), ("procedurals replaced", ProceduralPlan::all(Choice::Replace))] {
        let mut t = Tally::default();
        for p in &paths {
            let no = || false;
            let doc = match import_spp_with(Path::new(p), &plan, &mut |_, _| {}, Cancel::new(&no)) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("  {p}: {e}");
                    t.failed += 1;
                    continue;
                }
            };
            t.projects += 1;
            for i in 0..doc.texture_sets.len() {
                let Some(set) = doc.resolved_set(i) else { continue };
                count(&set, &OutputOptions::for_document(&doc), &mut t);
            }
        }
        report(name, &t);
    }
}

/// The layers a brush can paint: paint layers (their content) and every layer's mask.
fn targets(set: &TextureSet) -> Vec<&Layer> {
    set.walk_layers().map(|(_, l)| l).filter(|l| matches!(l.kind, LayerKind::Paint(_)) || l.mask.is_some()).collect()
}

fn count(set: &TextureSet, o: &OutputOptions, t: &mut Tally) {
    t.sets += 1;
    let mid = (set.resolution.0 / TILE_SIZE / 2, set.resolution.1 / TILE_SIZE / 2);
    let shown = Shown { view: ChannelView::Material, channel: ChannelDisplay::default() };
    let previewed = maps_patch::patchable(&shown, set);
    let mut taken = 0;
    let targets = targets(set);
    for l in &targets {
        t.targets += 1;
        let reached = MaterialMaps::tiles_reached(set, o, generators::builtin(), &NO_FILL_CONTEXT, &[l.id.as_str()], &[mid]);
        match (reached, previewed) {
            (Ok(_), true) => taken += 1,
            (Ok(_), false) => *t.reasons.entry("preview scaled (not square or above 4096)".into()).or_default() += 1,
            (Err(WholeReason::ReadsEverything), _) => {
                *t.reasons.entry(format!("reads everything: {}", whole_readers(set).join(", "))).or_default() += 1;
            }
            (Err(why), _) => *t.reasons.entry(format!("{why:?}")).or_default() += 1,
        }
    }
    t.targets_after += taken;
    t.sets_all += usize::from(!targets.is_empty() && taken == targets.len());
    t.sets_some += usize::from(taken > 0);
}

fn report(name: &str, t: &Tally) {
    let pct = |a: usize, b: usize| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };
    eprintln!("{name}: {} projects ({} failed to import), {} texture sets, {} paintable targets", t.projects, t.failed, t.sets, t.targets);
    eprintln!(
        "  sets, tile path: every target {} ({:.0} %), some target {} ({:.0} %)",
        t.sets_all,
        pct(t.sets_all, t.sets),
        t.sets_some,
        pct(t.sets_some, t.sets)
    );
    eprintln!("  targets, tile path: {} ({:.0} %)", t.targets_after, pct(t.targets_after, t.targets));
    for (why, n) in &t.reasons {
        eprintln!("  whole set: {why}: {n} targets");
    }
}

/// The effects of a set that read the whole image (by kind and procedural id), for naming the reason.
fn whole_readers(set: &TextureSet) -> Vec<String> {
    let reg = generators::builtin();
    let mut out = Vec::new();
    let name = |id: &str, p: Option<&str>| format!("{id} {}", p.unwrap_or(""));
    for (_, l) in set.walk_layers() {
        for e in &l.effects {
            let r = e.reach(reg, set, EvalTarget::Channel(materialcraft::paint::Channel::BaseColor));
            if r.input == Reach::Whole || r.stack_height == Some(Reach::Whole) {
                out.push(name(e.kind.id(), procedural(&e.kind)));
            }
        }
        for e in l.mask.iter().flat_map(|m| m.effects.iter()) {
            let r = e.reach(reg, set, EvalTarget::Mask);
            if r.input == Reach::Whole || r.stack_height == Some(Reach::Whole) {
                out.push(name(e.kind.id(), procedural(&e.kind)));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn procedural<D: materialcraft::paint::effect::EffectDomain>(k: &EffectKind<D>) -> Option<&str> {
    match k {
        EffectKind::Generator(p) | EffectKind::Filter(p) => Some(p.id.as_str()),
        _ => None,
    }
}
