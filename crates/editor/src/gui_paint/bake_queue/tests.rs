use super::*;
use materialcraft::bake::{MapKind, MapSelection};
use materialcraft::diag::{Collector, NullSink, RingBuffer};
use materialcraft::paint::{Document, TextureSet};
use std::time::{Duration, Instant};

/// A document on a bumpy sphere with `n` sets on its one slot, baked small from the low mesh.
fn sphere_doc(n: usize) -> Document {
    let mesh = bake::parse_obj(&bake::uv_sphere_obj(24, 12, 1.0, 0.05)).unwrap().to_mesh();
    let mut doc = Document::new(mesh);
    for i in 0..n {
        let mut set = TextureSet::new(format!("s{i}"), format!("Set {i}"), (32, 32));
        set.bake.common.padding = 2;
        doc.texture_sets.push(set);
    }
    doc
}

fn ids(doc: &Document) -> Vec<String> {
    doc.texture_sets.iter().map(|s| s.id.clone()).collect()
}

/// Polls until the queue ends (at most `limit`), calling `each` on every event.
fn drain(q: &mut BakeQueue, limit: Duration, mut each: impl FnMut(&BakeQueue, &BakeEvent)) -> Vec<BakeEvent> {
    let start = Instant::now();
    let mut all = Vec::new();
    while !q.is_finished() {
        assert!(start.elapsed() < limit, "the queue did not finish in {limit:?}");
        for ev in q.poll() {
            each(q, &ev);
            all.push(ev);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    all
}

fn history(log: &Collector) -> RingBuffer {
    let mut h = RingBuffer::new(1000);
    log.drain_into(&mut h);
    h
}

#[test]
fn bakes_sets_in_order_with_progress() {
    let doc = sphere_doc(2);
    let jobs = BakeJob::for_sets(&doc, &ids(&doc), Some(MapSelection::of(&[MapKind::Normal, MapKind::AmbientOcclusion])));
    assert_eq!(jobs.len(), 2);
    let log = Arc::new(Collector::new(1000));
    let mut q = BakeQueue::start(Arc::new(doc.mesh.clone()), None, jobs, log.clone());
    let mut seen = Vec::new();
    let events = drain(&mut q, Duration::from_secs(60), |q, ev| {
        if let BakeEvent::Started { .. } = ev {
            seen.push(q.progress().map(|(n, i, t)| (n.to_string(), i, t)));
        }
    });
    let baked: Vec<&String> = events.iter().filter_map(|e| if let BakeEvent::Baked { set, .. } = e { Some(set) } else { None }).collect();
    assert_eq!(baked, ["s0", "s1"]);
    assert!(matches!(events.last(), Some(BakeEvent::Finished { cancelled: false })));
    if let Some(BakeEvent::Baked { maps, .. }) = events.iter().find(|e| matches!(e, BakeEvent::Baked { .. })) {
        assert_eq!(maps.maps.keys().copied().collect::<Vec<_>>(), [MapKind::Normal, MapKind::AmbientOcclusion]);
        assert_eq!(maps.get(MapKind::Normal).map(|t| t.w), Some(32));
    }
    assert!(seen.iter().all(Option::is_some));
    assert!(history(&log).iter().all(|(_, e)| e.text.starts_with("Set ")), "log lines name the set");
}

#[test]
fn cancel_stops_the_queue_and_missing_high_meshes_fail_with_a_target() {
    let mut doc = sphere_doc(3);
    doc.texture_sets[1].bake.common.high = HighPoly { low_as_high: false, files: vec!["nowhere.obj".into()], placement: None };
    let log = Arc::new(Collector::new(1000));
    let mut q = BakeQueue::start(Arc::new(doc.mesh.clone()), None, BakeJob::for_sets(&doc, &ids(&doc), None), log.clone());
    let events = drain(&mut q, Duration::from_secs(120), |_, _| {});
    let failed: Vec<&String> = events.iter().filter_map(|e| if let BakeEvent::Failed { set, .. } = e { Some(set) } else { None }).collect();
    assert_eq!(failed, ["s1"]);
    let keyed = history(&log).iter().any(|(_, e)| e.target == Some(materialcraft::diag::Target::BakeSetting("common.high@s1".into())));
    assert!(keyed, "the failure points at the setting");

    let doc = sphere_doc(6);
    let mut q = BakeQueue::start(Arc::new(doc.mesh.clone()), None, BakeJob::for_sets(&doc, &ids(&doc), None), Arc::new(NullSink));
    let events = drain(&mut q, Duration::from_secs(120), |q, ev| {
        if let BakeEvent::Started { index: 1, .. } = ev {
            q.cancel();
        }
    });
    let baked = events.iter().filter(|e| matches!(e, BakeEvent::Baked { .. })).count();
    assert!(baked < 6, "{baked} sets baked after cancel");
    assert!(matches!(events.last(), Some(BakeEvent::Finished { cancelled: true })));
}

use materialcraft::paint::bake_config::HighPoly;

/// The gate of roadmap step 28: every texture set of Zubzub (18) in one queue, with a cancel, and the maps equal
/// byte for byte to mcparity's bake of the same sets with the same settings. Needs a local copy of the project:
/// `MC_ZUBZUB=/path/to/Zubzub.spp cargo test --release -p materialcraft-editor zubzub -- --ignored --nocapture`.
#[test]
#[ignore]
fn zubzub_bakes_in_one_queue_like_mcparity() {
    const SIZE: u32 = 256;
    let Some(path) = std::env::var_os("MC_ZUBZUB") else {
        eprintln!("skipped: set MC_ZUBZUB to the project's path");
        return;
    };
    let path = std::path::PathBuf::from(path);
    if !path.is_file() {
        eprintln!("skipped: {} not found", path.display());
        return;
    }
    let mut doc = materialcraft_substance::translate::import_spp(&path, &mut |_, _| {}).unwrap();
    assert_eq!(doc.texture_sets.len(), 18);
    for set in &mut doc.texture_sets {
        set.bake.common.size = SIZE;
    }
    let mesh = Arc::new(doc.mesh.clone());
    let dir = path.parent().map(std::path::Path::to_path_buf);
    let all = |doc: &Document| BakeJob::for_sets(doc, &ids(doc), Some(MapSelection::all()));

    // One queue, cancelled after the second set started.
    let mut q = BakeQueue::start(mesh.clone(), dir.clone(), all(&doc), Arc::new(NullSink));
    let events = drain(&mut q, Duration::from_secs(1800), |q, ev| {
        if let BakeEvent::Started { index: 1, .. } = ev {
            q.cancel();
        }
    });
    let done = events.iter().filter(|e| matches!(e, BakeEvent::Baked { .. })).count();
    assert!(done < 18 && matches!(events.last(), Some(BakeEvent::Finished { cancelled: true })), "{done} baked");
    eprintln!("cancelled after {done} set(s)");

    // The whole queue.
    let started = Instant::now();
    let mut q = BakeQueue::start(mesh, dir, all(&doc), Arc::new(NullSink));
    let events = drain(&mut q, Duration::from_secs(3600), |_, _| {});
    let mut baked = std::collections::BTreeMap::new();
    for ev in events {
        match ev {
            BakeEvent::Baked { set, maps } => drop(baked.insert(set, maps)),
            BakeEvent::Failed { set, error } => panic!("{set}: {error}"),
            _ => {}
        }
    }
    assert_eq!(baked.len(), 18);
    eprintln!("18 sets baked in {:.1} s", started.elapsed().as_secs_f32());

    // mcparity's bake of a few sets, same size, every map.
    let project = materialcraft_parity::project::OpenedProject::open(&path).unwrap();
    for case in project.sets.iter().filter(|s| ["Bretter", "Tentakel", "Seil"].contains(&s.name.as_str())) {
        let mut params = project.params(case, SIZE as usize);
        params.bake.maps = MapSelection::all();
        let set = doc.texture_sets.iter().find(|s| s.name == case.name).unwrap();
        assert_eq!(set.bake.settings_for(MapSelection::all()), params.bake, "{}: same settings", case.name);
        let source = project.high_source(case).unwrap();
        let inputs = project.inputs(case, &source).unwrap();
        let theirs = inputs.bake(&params).unwrap();
        let ours = &baked[&set.id];
        for kind in MapKind::ALL {
            let (a, b) = (ours.get(kind).unwrap(), theirs.get(kind).unwrap());
            let mae = a.px.iter().zip(&b.px).flat_map(|(p, q)| p.iter().zip(q).map(|(x, y)| (x - y).abs())).sum::<f32>() / (a.px.len() * 4) as f32;
            eprintln!("{} {kind}: MAE {:.6} (8-bit steps {:.4})", case.name, mae, mae * 255.0);
            assert_eq!(a.px, b.px, "{} {kind}: maps differ", case.name);
        }
    }
}
