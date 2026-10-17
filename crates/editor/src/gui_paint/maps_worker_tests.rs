//! The maps worker composites fills projected from a volume with the set's surface map, as the core does with a full
//! fill context, and keeps the surface map across runs.

use super::*;
use crate::gui_paint::document_state::OpenDocument;
use materialcraft::material_fill::{FolderResolver, MaterialCache};
use materialcraft::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use materialcraft::paint::{BlendMode, Channel, Fill, FillSource, ImageFill, Layer, LayerKind, PixelFormat, PixelLayout, SampleType, TextureSet, TiledImage};
use materialcraft::projection::{Projection, ProjectionMode, SceneFrame};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const SIZE: u32 = 32;

/// A bumped grid (x, y 0…1, z up to 0.3), so the tri-planar planes blend.
fn bumped_grid(n: u32) -> Mesh {
    let mut part = MeshPart { name: "grid".into(), ..MeshPart::default() };
    let pi = std::f32::consts::PI;
    for y in 0..=n {
        for x in 0..=n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            let z = 0.3 * (pi * u).sin() * (pi * v).sin();
            let (dx, dy) = (0.3 * pi * (pi * u).cos() * (pi * v).sin(), 0.3 * pi * (pi * u).sin() * (pi * v).cos());
            let len = (dx * dx + dy * dy + 1.0).sqrt();
            part.positions.push([u, v, z]);
            part.normals.push([-dx / len, -dy / len, 1.0 / len]);
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

/// A set with a grey base and a tri-planar image fill of a colour ramp on top.
fn tri_planar_document() -> Document {
    let mut ramp = TiledImage::new(16, 16, PixelFormat::new(PixelLayout::Rgba, SampleType::U8)).expect("image");
    for y in 0..16 {
        for x in 0..16 {
            ramp.set(x, y, [x as f32 / 15.0, y as f32 / 15.0, 0.5, 1.0]);
        }
    }
    let mut image = ImageFill::default();
    image.set_image(Channel::BaseColor, ramp);
    let projection = Projection { mode: ProjectionMode::default_of("tri_planar").expect("tri-planar"), ..Projection::default() };
    let fill = Fill::new(FillSource::Image(image)).with_projection(projection);
    let mut set = TextureSet::new("s", "S", (SIZE, SIZE)).with_channel(Channel::BaseColor);
    let base = Fill::uniform(vec![(Channel::BaseColor, [0.5, 0.5, 0.5, 1.0])]);
    set.layers.push(Layer::new("base", "Base", LayerKind::Fill(base)).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
    set.layers.push(Layer::new("tri", "Tri-planar", LayerKind::Fill(fill)).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
    let mut doc = Document::new(bumped_grid(8));
    doc.texture_sets.push(set);
    doc
}

fn materials() -> MaterialFills {
    MaterialFills::new(Arc::new(FolderResolver::new([PathBuf::from("/nonexistent")])), Arc::new(MaterialCache::default()))
}

/// Runs a maps worker over set `set` to the end.
fn run_set(doc: &mut OpenDocument, set: usize, surfaces: Option<Arc<FillSurfaces>>) -> SetMaps {
    let mut worker = MapsWorker::start(doc.maps_view(&[set]), vec![set], ChannelView::Material, None, materials(), None, surfaces);
    let start = Instant::now();
    let mut out = Vec::new();
    while !worker.is_finished() || out.is_empty() {
        out.extend(worker.poll());
        assert!(start.elapsed() < Duration::from_secs(60), "the maps worker did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
    out.pop().expect("the set's maps")
}

fn run(doc: &mut OpenDocument, surfaces: Option<Arc<FillSurfaces>>) -> SetMaps {
    run_set(doc, 0, surfaces)
}

/// Needs-mesh warnings among a run's log entries.
fn needs_mesh(maps: &SetMaps) -> usize {
    maps.warnings.iter().filter(|e| e.text.contains("needs the mesh")).count()
}

/// A real document's sets with and without surface maps: needs-mesh warnings and the time of a recomposite (the
/// first one with surfaces builds them, later ones reuse them). `MC_FILLS_DOC` names the `.mcpaint`. Run with
/// `cargo test --release -p materialcraft-editor real_document_fills -- --ignored --nocapture`.
#[test]
#[ignore]
fn real_document_fills() {
    let path = std::env::var_os("MC_FILLS_DOC").expect("MC_FILLS_DOC");
    let loaded = materialcraft::paint::format::load_path(std::path::Path::new(&path)).expect("load");
    let mut doc = OpenDocument::new(loaded, PathBuf::from(&path), None);
    let surfaces = Arc::new(FillSurfaces::new(doc.shared_mesh()));
    for set in 0..doc.doc().texture_sets.len() {
        let name = doc.doc().texture_sets[set].name.clone();
        let needs = doc.doc().texture_sets[set].needs_surface();
        let timed = |doc: &mut OpenDocument, s: Option<Arc<FillSurfaces>>| {
            let start = Instant::now();
            let maps = run_set(doc, set, s);
            (maps, start.elapsed())
        };
        let (plain, t_plain) = timed(&mut doc, None);
        let (first, t_first) = timed(&mut doc, Some(surfaces.clone()));
        let (again, t_again) = timed(&mut doc, Some(surfaces.clone()));
        eprintln!(
            "{name}: needs surface {needs}; warnings without/with surfaces {}/{}, needs-mesh {}/{}; recomposite without {t_plain:?}, first with {t_first:?}, again {t_again:?}; base colour differs: {}",
            plain.warnings.len(),
            first.warnings.len(),
            needs_mesh(&plain),
            needs_mesh(&first),
            plain.maps.base != first.maps.base
        );
        assert_eq!(needs_mesh(&first), 0);
        assert_eq!(first.maps.base, again.maps.base);
    }
}

#[test]
fn tri_planar_fills_composite_as_the_core_does() {
    let mut doc = OpenDocument::new(tri_planar_document(), PathBuf::from("t.mcpaint"), None);
    let ts = &doc.doc().texture_sets[0];
    assert!(ts.needs_surface());
    // the core with a full fill context: its own surface map and the scene frame
    let mesh = &doc.doc().mesh;
    let key = SurfaceKey { material: 0, uv_set: 0, width: SIZE, height: SIZE, gutter: 0, mesh_revision: 0 };
    let surface = SurfaceMap::build(mesh, key).expect("surface map");
    let sources = materials();
    let full = FillContext::with_sources(&sources).with_surface(&surface, SceneFrame::of_mesh(mesh));
    let opts = OutputOptions::for_document(doc.doc());
    let core = MaterialMaps::build_with(ts, &opts, generators::builtin(), &full);
    assert!(core.warnings.is_empty(), "{:?}", core.warnings);
    let expected = ChannelView::Material.prepare(&core, ts);
    let without = ChannelView::Material.prepare(&MaterialMaps::build_with(ts, &opts, generators::builtin(), &FillContext::with_sources(&sources)), ts);
    assert_ne!(expected.base, without.base, "the fill shows only with the surface");

    let surfaces = Arc::new(FillSurfaces::new(doc.shared_mesh()));
    let maps = run(&mut doc, Some(surfaces.clone()));
    assert!(maps.warnings.is_empty(), "no needs-mesh warning: {:?}", maps.warnings.iter().map(|e| &e.text).collect::<Vec<_>>());
    assert_eq!(maps.maps.base, expected.base);

    // later runs reuse the map the first one built; the UI thread sees it too
    let built = surfaces.cached(&doc.doc().texture_sets[0], (SIZE, SIZE)).expect("kept");
    let again = run(&mut doc, Some(surfaces.clone()));
    assert_eq!(again.maps.base, expected.base);
    let still = surfaces.cached(&doc.doc().texture_sets[0], (SIZE, SIZE)).expect("kept");
    assert!(Arc::ptr_eq(&built.map, &still.map), "the surface map is not rebuilt per run");
    assert!(still.frame.is_some());

    // without surfaces the fill is left out and reported
    let plain = run(&mut doc, None);
    assert_eq!(plain.maps.base, without.base);
    assert!(!plain.warnings.is_empty());
}

#[test]
fn the_mesh_is_shared_once() {
    let mut doc = OpenDocument::new(tri_planar_document(), PathBuf::from("t.mcpaint"), None);
    let (a, b) = (doc.shared_mesh(), doc.shared_mesh());
    assert!(Arc::ptr_eq(&a.mesh, &b.mesh) && a.revision == b.revision);
    assert!(doc.maps_view(&[0]).mesh.parts.is_empty(), "the maps view copies no mesh");
}
