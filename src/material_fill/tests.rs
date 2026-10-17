use super::*;
use crate::paint::format::{load, save};
use crate::paint::{BlendMode, Channel, CompositeWarning, Compositor, Document, Fill, FillContext, FillSource, Layer, LayerKind, TextureSet};
use crate::projection::{Filter, ImagePlacement, Projection};
use std::io::Cursor;
use std::sync::Arc;

/// Base colour from an exposed grey level, roughness a horizontal ramp, no metallic output.
const GRAPH: &str = r#"size 16
node 1 color r=0.5 g=0.5 b=0.5
node 2 gradient mode=0 angle=0
output basecolor 1
output roughness 2
expose "Level" 1 r 0 1
"#;

const R: Channel = Channel::Roughness;

fn material(params: Vec<(String, String)>) -> Fill {
    Fill::new(FillSource::Material { path: "materials/test.mca".into(), params })
}

fn set_with(fill: Fill, size: u32) -> TextureSet {
    let mut s = TextureSet::new("s", "S", (size, size)).with_channel(R).with_channel(Channel::BaseColor).with_channel(Channel::Metallic);
    let below = Fill::uniform(vec![(R, [0.9; 4]), (Channel::Metallic, [0.25; 4])]);
    s.layers.push(Layer::new("base", "Base", LayerKind::Fill(below)).with_blend(R, BlendMode::Normal, 1.0).with_blend(
        Channel::Metallic,
        BlendMode::Normal,
        1.0,
    ));
    let mut layer = Layer::new("mat", "Material", LayerKind::Fill(fill));
    for c in [R, Channel::BaseColor, Channel::Metallic] {
        layer = layer.with_blend(c, BlendMode::Normal, 1.0);
    }
    s.layers.push(layer);
    s
}

fn fills(resolver: impl MaterialResolver + 'static) -> MaterialFills {
    MaterialFills::new(Arc::new(resolver), Arc::new(MaterialCache::default()))
}

fn known() -> MaterialFills {
    fills(MemoryResolver::new().with("materials/test.mca", GRAPH))
}

/// Row 0 of `channel` composited with `sources`.
fn row(s: &TextureSet, sources: &MaterialFills, channel: Channel) -> Vec<f32> {
    let context = FillContext::with_sources(sources);
    let (tex, _) = Compositor::new(s).with_fills(&context).composite_tex(channel);
    (0..tex.w).map(|x| tex.px[x][0]).collect()
}

fn warnings(s: &TextureSet, sources: &MaterialFills, channel: Channel) -> Vec<CompositeWarning> {
    let context = FillContext::with_sources(sources);
    Compositor::new(s).with_fills(&context).warnings(channel)
}

#[test]
fn a_material_fill_sets_the_channels_it_has_outputs_for() {
    let s = set_with(material(vec![]), 16);
    let f = known();
    // texture space at the render's size: the graph's own values, texel for texel
    let graph = crate::graph::Graph::parse(GRAPH).unwrap().evaluate(Some(16)).unwrap();
    let expected: Vec<f32> = graph["roughness"].px[..16].iter().map(|p| p[0]).collect();
    let ramp = row(&s, &f, R);
    assert!(ramp.iter().zip(&expected).all(|(a, b)| (a - b).abs() < 1e-4), "{ramp:?} vs {expected:?}");
    assert!(ramp.iter().any(|v| (v - ramp[0]).abs() > 0.5), "{ramp:?}");
    assert!(row(&s, &f, Channel::BaseColor).iter().all(|v| (v - 0.5).abs() < 2e-3));
    // no metallic output: the layer below shows
    assert!(row(&s, &f, Channel::Metallic).iter().all(|v| (v - 0.25).abs() < 1e-3));
    assert!(warnings(&s, &f, R).is_empty());
}

#[test]
fn the_projection_tiles_the_material() {
    let tiled = Projection { filter: Filter::Bilinear, ..Projection::uv(ImagePlacement { tiling: [2.0, 1.0], ..ImagePlacement::default() }) };
    let s = set_with(material(vec![]).with_projection(tiled), 16);
    let r = row(&s, &known(), R);
    // the material twice across: period 8 texels, not constant
    assert!((0..8).all(|x| (r[x] - r[x + 8]).abs() < 1e-4), "{r:?}");
    assert!(r.iter().any(|v| (v - r[0]).abs() > 0.3), "{r:?}");
    let untiled = row(&set_with(material(vec![]), 16), &known(), R);
    assert_ne!(r, untiled);
}

#[test]
fn parameters_override_exposed_values_and_change_the_key() {
    let f = known();
    let s = set_with(material(vec![("Level".into(), "0.2".into())]), 16);
    assert!(row(&s, &f, Channel::BaseColor).iter().all(|v| (v - 0.2).abs() < 2e-3));
    let a = f.material("materials/test.mca", &[]).unwrap().key(16);
    let b = f.material("materials/test.mca", &[("Level".into(), "0.2".into())]).unwrap().key(16);
    assert_ne!(a, b);
    assert_ne!(a, f.material("materials/test.mca", &[]).unwrap().key(32));
    // a parameter the graph does not expose, or a value that is no number: not computable
    for bad in [("Nope", "1"), ("Level", "lots")] {
        let s = set_with(material(vec![(bad.0.into(), bad.1.into())]), 16);
        assert_eq!(warnings(&s, &f, R), vec![CompositeWarning::MaterialFillSkipped { layer: "mat".into() }]);
        assert!(row(&s, &f, R).iter().all(|v| (v - 0.9).abs() < 1e-3));
    }
}

#[test]
fn unknown_references_and_broken_graphs_are_not_computable() {
    let s = set_with(material(vec![]), 16);
    for f in [fills(MemoryResolver::new()), fills(MemoryResolver::new().with("materials/test.mca", "node 1 nonsense\noutput basecolor 1\n"))] {
        assert_eq!(warnings(&s, &f, R), vec![CompositeWarning::MaterialFillSkipped { layer: "mat".into() }]);
        assert!(row(&s, &f, R).iter().all(|v| (v - 0.9).abs() < 1e-3));
    }
}

#[test]
fn renders_are_cached_by_key_and_evicted_over_budget() {
    let f = known();
    let a = f.render("materials/test.mca", &[], 16).unwrap();
    assert!(Arc::ptr_eq(&a, &f.render("materials/test.mca", &[], 16).unwrap()));
    assert_eq!(f.cache().len(), 1);
    assert!(f.cache().bytes() > 0);
    assert_eq!(a.channels().collect::<Vec<_>>(), vec![Channel::BaseColor, Channel::Roughness]);
    // a cache too small for two renders keeps the newest only
    let small = MaterialFills::new(Arc::new(MemoryResolver::new().with("materials/test.mca", GRAPH)), Arc::new(MaterialCache::new(1)));
    let first = small.render("materials/test.mca", &[], 16).unwrap();
    small.render("materials/test.mca", &[], 32).unwrap();
    assert_eq!(small.cache().len(), 1);
    assert!(small.cache().get(small.material("materials/test.mca", &[]).unwrap().key(16)).is_none());
    assert_eq!(first.size(), 16);
}

#[test]
fn a_material_fill_round_trips_through_the_format() {
    let mut doc = Document::default();
    doc.texture_sets.push(set_with(material(vec![("Level".into(), "0.3".into())]).with_projection(Projection::uv(ImagePlacement::default())), 16));
    let mut bytes = Vec::new();
    save(&doc, &mut bytes).unwrap();
    let back = load(Cursor::new(&bytes)).unwrap();
    assert_eq!(back.texture_sets[0].layers, doc.texture_sets[0].layers);
    let s = &back.texture_sets[0];
    assert!(row(s, &known(), Channel::BaseColor).iter().all(|v| (v - 0.3).abs() < 2e-3));
    // without the material the document still loads; the fill is reported, not guessed
    assert_eq!(warnings(s, &fills(MemoryResolver::new()), R), vec![CompositeWarning::MaterialFillSkipped { layer: "mat".into() }]);
}

#[test]
fn folder_resolver_finds_files_under_its_folders_only() {
    let dir = std::env::temp_dir().join(format!("mc-material-fill-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("materials")).unwrap();
    std::fs::write(dir.join("materials/test.mca"), GRAPH).unwrap();
    let r = FolderResolver::new([dir.join("missing"), dir.clone()]);
    assert_eq!(r.resolve("materials/test.mca").unwrap(), GRAPH);
    assert!(matches!(r.resolve("materials/other.mca"), Err(ResolveError::NotFound { .. })));
    assert!(r.locate("../materials/test.mca").is_none());
    assert!(r.locate("").is_none());
    let absolute = dir.join("materials/test.mca");
    assert_eq!(r.locate(absolute.to_str().unwrap()), Some(absolute));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Gate of roadmap step 14: `cargo test --release -p materialcraft material_fill_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn material_fill_bench_2k() {
    use crate::paint::{MaterialMaps, OutputOptions};
    use std::time::Instant;
    let f = fills(MemoryResolver::new().with("materials/brick.mca", crate::graph::EXAMPLE));
    let fill = Fill::new(FillSource::Material { path: "materials/brick.mca".into(), params: vec![] })
        .with_projection(Projection::uv(ImagePlacement { tiling: [2.0, 2.0], ..ImagePlacement::default() }));
    let mut s = TextureSet::new("s", "S", (2048, 2048));
    let channels = [Channel::BaseColor, Channel::Roughness, Channel::Metallic, Channel::Normal, Channel::Height, Channel::AmbientOcclusion];
    let mut layer = Layer::new("brick", "Brick", LayerKind::Fill(fill));
    for c in channels {
        s = s.with_channel(c);
        layer = layer.with_blend(c, BlendMode::Normal, 1.0);
    }
    s.layers.push(layer);
    let context = FillContext::with_sources(&f);
    let t = Instant::now();
    f.render("materials/brick.mca", &[], MaterialFills::render_size(&s)).unwrap();
    println!("first render (graph at 2K + levels): {:?}, {} MiB", t.elapsed(), f.cache().bytes() >> 20);
    let t = Instant::now();
    let maps = MaterialMaps::build_with_fills(&s, &OutputOptions::default(), &context);
    let all = t.elapsed();
    assert!(maps.warnings.is_empty(), "{:?}", maps.warnings);
    println!("composite all maps after the first render: {all:?}");
    for c in [Channel::BaseColor, Channel::Roughness] {
        let best = (0..5)
            .map(|_| {
                let t = Instant::now();
                let (_, w) = Compositor::new(&s).with_fills(&context).composite_tex(c);
                assert!(w.is_empty());
                t.elapsed()
            })
            .min();
        println!("cached re-composite of {} (best of 5): {best:?}", c.id());
    }
}
