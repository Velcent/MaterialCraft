//! Edits while background workers hold a view of the document: the edit must not copy the document.

use super::*;
use materialcraft::mesh::{Mesh, MeshPart};
use materialcraft::paint::edit::{Edit, LayerProperty, LayerRef};
use materialcraft::paint::{BlendMode, Channel, ForeignFile, Layer, LayerKind, PaintContent, PixelFormat, PixelLayout, SampleType, TextureSet, TiledImage};
use std::time::{Duration, Instant};

/// A grid mesh of about `2 * n * n` triangles.
fn grid(n: u32) -> Mesh {
    let mut part = MeshPart { name: "grid".into(), ..MeshPart::default() };
    for y in 0..=n {
        for x in 0..=n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            part.positions.push([u, v, 0.0]);
            part.normals.push([0.0, 0.0, 1.0]);
        }
    }
    part.uv_sets = vec![part.positions.iter().map(|p| [p[0], p[1]]).collect()];
    for y in 0..n {
        for x in 0..n {
            let i = y * (n + 1) + x;
            part.triangles.push([i, i + 1, i + n + 1]);
            part.triangles.push([i + 1, i + n + 2, i + n + 1]);
        }
    }
    Mesh { materials: vec!["m".into()], parts: vec![part] }
}

/// One `size`² texture set with `layers` painted layers (base colour), a grid mesh and some kept files.
fn synthetic(size: u32, layers: usize, mesh: u32) -> Document {
    let format = PixelFormat::new(PixelLayout::Rgba, SampleType::U8);
    let mut set = TextureSet::new("s", "S", (size, size)).with_channel(Channel::BaseColor);
    for i in 0..layers {
        let mut image = TiledImage::new(size, size, format).expect("image");
        image.fill([i as f32 / layers as f32, 0.5, 0.5, 1.0]);
        let content = PaintContent::from_images(vec![(Channel::BaseColor, image)]);
        set.layers.push(Layer::new(format!("l{i}"), format!("L{i}"), LayerKind::Paint(content)).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
    }
    let mut doc = Document::new(grid(mesh));
    doc.texture_sets.push(set);
    doc.foreign.push(ForeignFile { path: "foreign/kept.bin".into(), bytes: vec![7; 64 << 20] });
    doc
}

fn rename(doc: &mut OpenDocument, set: &str, layer: &str, name: String) {
    let edit = Edit::SetLayer { layer: LayerRef::new(set, layer), property: LayerProperty::Name(name) };
    doc.edit("Rename", None, |tx| tx.apply(edit)).expect("rename");
}

#[test]
fn edits_leave_the_workers_view_alone() {
    let mut doc = OpenDocument::new(synthetic(256, 3, 8), PathBuf::from("t.mcpaint"), None);
    let maps = doc.maps_view(&[0]);
    let mesh = doc.mesh_view();
    rename(&mut doc, "s", "l1", "renamed".into());
    assert_eq!(doc.doc().texture_sets[0].layers[1].name, "renamed");
    assert_eq!(maps.texture_sets[0].layers[1].name, "L1", "the worker keeps the state it was started with");
    assert_eq!(mesh.mesh.parts[0].triangles.len(), 128);
    assert!(maps.mesh.parts.is_empty() && maps.foreign.is_empty(), "compositing gets no mesh and no kept files");
    assert!(doc.maps_view(&[]).texture_sets[0].layers.is_empty(), "sets not composited keep only their placement");
    assert!(mesh.foreign.is_empty() && mesh.import_report.is_none());
}

/// Time per edit while a maps worker holds the document (the worker is restarted after every edit during a
/// slider drag). `MC_PAINT_BENCH_DOC` names a `.mcpaint` to use instead of a synthetic 2K, 20-layer document.
/// Run with `cargo test --release -p materialcraft-editor edit_during_maps -- --ignored --nocapture`.
#[test]
#[ignore]
fn edit_during_maps_job_bench() {
    let (doc, label) = match std::env::var_os("MC_PAINT_BENCH_DOC") {
        Some(p) => (materialcraft::paint::format::load_path(Path::new(&p)).expect("load"), PathBuf::from(p).display().to_string()),
        None => (synthetic(2048, 20, 700), "synthetic 2K, 20 layers, 980k triangles, 64 MB kept files".to_string()),
    };
    let (index, set, layer) = doc.texture_sets.iter().enumerate().find_map(|(i, s)| Some((i, s.id.clone(), s.layers.first()?.id.clone()))).expect("a layer");
    let mut doc = OpenDocument::new(doc, PathBuf::from("bench.mcpaint"), None);
    let runs = 20;
    let (mut total, mut worst) = (Duration::ZERO, Duration::ZERO);
    for i in 0..runs {
        let held = doc.maps_view(&[index]);
        let start = Instant::now();
        rename(&mut doc, &set, &layer, format!("n{i}"));
        let t = start.elapsed();
        drop(held);
        let snapshot = Instant::now();
        let again = doc.maps_view(&[index]);
        let view = snapshot.elapsed();
        drop(again);
        total += t;
        worst = worst.max(t);
        if i == 0 {
            let all: Vec<usize> = (0..doc.doc().texture_sets.len()).collect();
            let start = Instant::now();
            drop(doc.maps_view(&all));
            eprintln!("taking the maps view: {view:?} for the edited set, {:?} for all sets", start.elapsed());
            let start = Instant::now();
            drop(doc.mesh_view());
            eprintln!("taking the mesh view (once per geometry build): {:?}", start.elapsed());
        }
    }
    eprintln!("{label}: edit while a maps job runs: mean {:?}, worst {worst:?}", total / runs);
}
