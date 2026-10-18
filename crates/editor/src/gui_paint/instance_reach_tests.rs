//! Roadmap step 44's gate: an edit of a source reaches its 50 instances in 50 texture sets, and only the sets with
//! an instance are composited again, not the unrelated ones.

use super::*;
use crate::gui_paint::channel_display::ChannelDisplay;
use crate::gui_paint::channel_view::{ChannelView, Shown};
use crate::gui_paint::document_state::OpenDocument;
use crate::gui_paint::maps_worker::{MapsWorker, RunSetup, SetContent, SetMaps, Want};
use crate::gui_paint::stack_editor;
use materialcraft::history::CoalesceKey;
use materialcraft::material_fill::{FolderResolver, MaterialCache, MaterialFills};
use materialcraft::paint::edit::LayerSlot;
use materialcraft::paint::{BlendMode, Channel, Fill, InstanceLink, Layer, LayerKind, TextureSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const INSTANCES: usize = 50;
const UNRELATED: usize = 5;

fn fill(id: &str, v: f32) -> Layer {
    Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [v; 4])]))).with_blend(Channel::Roughness, BlendMode::Normal, 1.0)
}

fn set(id: &str) -> TextureSet {
    TextureSet::new(id, id, (8, 8)).with_channel(Channel::Roughness)
}

/// Set 0 "src" holds fill "f" and folder "g"; sets 1…50 an instance of "f" each (every tenth inside a folder, every
/// seventh an instance of "g"); the last sets have no instances.
fn document() -> OpenDocument {
    let mut src = set("src");
    src.layers = vec![fill("f", 1.0), Layer::new("g", "G", LayerKind::Group(vec![fill("in-g", 0.25)])).with_blend(Channel::Roughness, BlendMode::Normal, 1.0)];
    let mut sets = vec![src];
    for i in 0..INSTANCES {
        let mut s = set(&format!("i{i}"));
        s.layers.push(fill("base", 0.0));
        let of = if i % 7 == 3 { "g" } else { "f" };
        let instance = Layer::new("inst", "Instance", LayerKind::Instance(InstanceLink::new("src", of)));
        s.layers.push(if i % 10 == 0 {
            Layer::new("holder", "Holder", LayerKind::Group(vec![instance])).with_blend(Channel::Roughness, BlendMode::Normal, 1.0)
        } else {
            instance
        });
        sets.push(s);
    }
    for i in 0..UNRELATED {
        let mut s = set(&format!("u{i}"));
        s.layers.push(fill("own", 0.5));
        sets.push(s);
    }
    let doc = materialcraft::paint::Document { texture_sets: sets, ..Default::default() };
    OpenDocument::new(doc, PathBuf::from("instances.mcpaint"), None)
}

/// The sets a panel edit recomposites: its own and those it reaches.
fn recomposited(doc: &mut OpenDocument, ev: LayerEvent) -> Vec<usize> {
    let applied = stack_editor::apply(doc, ev).expect("the edit applies");
    applied.set.into_iter().chain(applied.also).collect()
}

/// Runs a maps worker over `sets` to the end; the maps of every set it composited.
fn composite(doc: &OpenDocument, sets: Vec<usize>) -> Vec<SetMaps> {
    let materials = MaterialFills::new(Arc::new(FolderResolver::new([PathBuf::from("/nonexistent")])), Arc::new(MaterialCache::default()));
    let setup =
        RunSetup { shown: Shown { view: ChannelView::Material, channel: ChannelDisplay::default() }, isolate: None, materials, tint: None, surfaces: None };
    let jobs = sets.iter().map(|&s| (s, Want::Full)).collect();
    let mut worker = MapsWorker::start(doc.maps_view(&sets), jobs, setup);
    let start = Instant::now();
    let mut out = Vec::new();
    while !worker.is_finished() {
        out.extend(worker.poll());
        assert!(start.elapsed() < Duration::from_secs(60), "the maps worker did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
    out.extend(worker.poll());
    out
}

fn roughness(maps: &SetMaps) -> u8 {
    match &maps.content {
        SetContent::Full(m) => m.rma[0],
        SetContent::Patch(_) => panic!("whole sets were asked for"),
    }
}

#[test]
fn a_source_edit_reaches_its_instances_and_nothing_else() {
    let mut doc = document();
    let f_sets: Vec<usize> = (0..INSTANCES).filter(|i| i % 7 != 3).map(|i| i + 1).collect();
    let before = composite(&doc, (0..doc.doc().texture_sets.len()).collect());
    assert!(before.iter().filter(|m| f_sets.contains(&m.set)).all(|m| roughness(m) == 255), "every instance shows the source");

    let ev = LayerEvent::SetOpacity { layer: LayerRef::new("src", "f"), channel: Channel::Roughness, opacity: 0.2, key: CoalesceKey(1) };
    let sets = recomposited(&mut doc, ev);
    assert_eq!(sets, std::iter::once(0).chain(f_sets.iter().copied()).collect::<Vec<_>>(), "the source's set and the sets with an instance of it");
    let after = composite(&doc, sets.clone());
    assert_eq!(after.len(), sets.len(), "one composite per reached set, none for the {UNRELATED} unrelated sets");
    // Fill 1.0 at the new opacity 0.2 over a base of 0: 0.2 (sets with the instance in a folder blend it there).
    for m in after.iter().filter(|m| m.set != 0) {
        let held = (m.set - 1) % 10 == 0;
        let expected = if held { after.iter().find(|n| n.set == 1).map(roughness) } else { Some(51) };
        assert_eq!(Some(roughness(m)), expected, "set {} follows the source", m.set);
        assert_ne!(roughness(m), 255);
    }
}

#[test]
fn folder_edits_reach_folder_instances() {
    let mut doc = document();
    let g_sets: Vec<usize> = (0..INSTANCES).filter(|i| i % 7 == 3).map(|i| i + 1).collect();
    // A layer added inside the source folder, an edit of a layer in it, and a move out of it.
    let create = LayerEvent::Create { set: "src".into(), what: materialcraft::paint::edit::ops::NewLayer::Fill, above: Some("in-g".into()) };
    assert_eq!(recomposited(&mut doc, create), std::iter::once(0).chain(g_sets.iter().copied()).collect::<Vec<_>>());
    let ev = LayerEvent::SetVisible(LayerRef::new("src", "in-g"), false);
    assert_eq!(recomposited(&mut doc, ev)[1..], g_sets[..]);
    let out = LayerEvent::Move { set: "src".into(), ids: vec!["in-g".into()], to: LayerSlot::top(0), copy: false };
    assert_eq!(recomposited(&mut doc, out)[1..], g_sets[..]);
    // An unrelated edit reaches nothing else.
    assert_eq!(recomposited(&mut doc, LayerEvent::SetVisible(LayerRef::new("u0", "own"), false)), [INSTANCES + 1]);
    // Instantiating reaches the sets that get an instance.
    let ev = LayerEvent::Instantiate { source: LayerRef::new("src", "f"), sets: vec!["u1".into(), "u3".into()] };
    assert_eq!(recomposited(&mut doc, ev), [0, INSTANCES + 2, INSTANCES + 4]);
}
