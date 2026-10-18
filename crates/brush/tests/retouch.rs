//! Roadmap step 36's gate: clone copies texel for texel with an aligned source, smudge replays bit for bit, and a
//! see-through layer's smudge is painted again when a layer below it changes.

use materialcraft::camera::{Camera, Lens};
use materialcraft::history::PaintHistory;
use materialcraft::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use materialcraft::paint::brush::{BrushPreset, Retouch};
use materialcraft::paint::edit::ops::{NewLayer, create};
use materialcraft::paint::edit::{Edit, LayerRef, Transaction};
use materialcraft::paint::stroke::{Stroke, StrokeData, StrokeTool, Symmetry, ViewKind};
use materialcraft::paint::{Channel, Compositor, Document, LayerKind, NO_FILL_CONTEXT, TextureSet, TiledImage};
use materialcraft::projection::{ViewProjector, ViewScene};
use materialcraft::stamp::TexelIndex;
use materialcraft_brush::{
    BelowSource, CloneSource, DocumentStroke, InputPoint, PaintTarget, PaintValues, ReplayScene, SeeThroughCache, SessionScene, StrokeSetup, TargetPart,
    mode_for, recorded_brush, recorded_values, replay, texel_under,
};

const SIZE: u32 = 256;

fn sphere() -> Mesh {
    let m = materialcraft::render3d::build_mesh(0, 48, 1.0, None, 0.0, 0.0);
    Mesh { materials: vec!["m".into()], parts: vec![MeshPart::from_viewport("sphere", 0, &m)] }
}

fn camera() -> Camera {
    let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    Camera::looking_at([0.4, 0.3, 3.0], [0.0; 3], [0.0, 1.0, 0.0], &lens, [480.0, 360.0])
}

/// A value per texel that its neighbours (and the texels a few dozen away) don't share.
fn noise(x: u32, y: u32, k: u32) -> f32 {
    let h = x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663) ^ k.wrapping_mul(83_492_791);
    (h.wrapping_mul(2_654_435_761) >> 24) as f32 / 255.0
}

fn pattern(img: &mut TiledImage, salt: u32) {
    let all = img.all_tiles();
    img.update_tiles(&all, |r, px| {
        for (i, p) in px.iter_mut().enumerate() {
            let (x, y) = (r.x + i as u32 % r.w, r.y + i as u32 / r.w);
            *p = [noise(x, y, salt), noise(x, y, salt + 1), noise(x, y, salt + 2), 1.0];
        }
    });
}

/// A sphere with one texture set: a painted layer "lower" (base colour and roughness) and, if asked, a
/// see-through layer above it.
fn document(see_through: bool) -> (Document, String, Option<String>) {
    let mut doc = Document { mesh: sphere(), ..Document::default() };
    doc.texture_sets.push(TextureSet::new("s", "S", (SIZE, SIZE)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness));
    let mut tx = Transaction::new(&mut doc);
    let lower = create(&mut tx, "s", NewLayer::Paint, None).unwrap();
    let top = see_through.then(|| create(&mut tx, "s", NewLayer::SeeThrough, Some(&lower)).unwrap());
    tx.commit();
    let LayerKind::Paint(p) = &mut doc.texture_sets[0].layer_mut(&lower).unwrap().kind else { unreachable!() };
    for (i, (_, img)) in p.images.iter_mut().enumerate() {
        pattern(img, i as u32 * 7);
    }
    (doc, lower, top)
}

fn values() -> PaintValues {
    PaintValues { channels: vec![(Channel::BaseColor, [0.0; 4], true), (Channel::Roughness, [0.0; 4], true)], mask: 1.0 }
}

fn setup(doc: &Document, tool: StrokeTool, retouch: Retouch) -> StrokeSetup {
    let preset = BrushPreset { size: 0.15, hardness: 1.0, flow: 1.0, opacity: 1.0, spacing: 0.1, ..BrushPreset::default() };
    StrokeSetup {
        preset,
        tool,
        seed: 3,
        symmetry: Symmetry::default(),
        view: ViewKind::Mesh,
        values: values(),
        model: doc.mesh.bounding_sphere().unwrap(),
        occlusion: true,
        retouch,
    }
}

struct Scene {
    map: SurfaceMap,
    index: TexelIndex,
    view: ViewScene,
}

impl Scene {
    fn of(doc: &Document) -> Scene {
        let map = SurfaceMap::build(&doc.mesh, SurfaceKey::square(&doc.mesh, 0, SIZE)).unwrap();
        let index = TexelIndex::build(&map);
        Scene { map, index, view: ViewScene::new(&doc.mesh, doc.mesh.fit()) }
    }

    fn replay<'a>(&'a self, mesh: &'a Mesh) -> ReplayScene<'a> {
        ReplayScene { mesh, view: &self.view, surface: &self.map, index: &self.index, factors: &[] }
    }
}

/// Paints a stroke from screen `from` to `to` into `layer`; on see-through content, from `cache`'s base. Returns
/// the inverses for one history step.
#[allow(clippy::too_many_arguments)]
fn stroke(
    doc: &mut Document,
    scene: &Scene,
    layer: &str,
    tool: StrokeTool,
    retouch: Retouch,
    from: [f32; 2],
    to: [f32; 2],
    base: Option<materialcraft_brush::Base>,
) -> Vec<Edit> {
    let setup = setup(doc, tool, retouch);
    let mut prepared = DocumentStroke::prepare(doc, PaintTarget { layer: LayerRef::new("s", layer), part: TargetPart::Content }, &values()).unwrap();
    let Document { mesh, texture_sets, .. } = doc;
    let set = &mut texture_sets[0];
    if let Some(base) = base {
        prepared.see_through(set, base);
    }
    let projector = scene.view.projector(mesh, camera()).unwrap();
    let ss = SessionScene { surface: &scene.map, index: &scene.index, projector: Some(&projector as &dyn ViewProjector), factors: &[], projected: None };
    let mut s = prepared.start(setup, &ss, mesh, &|_| true, set).map_err(|b| b.1).unwrap();
    for i in 0..30 {
        let t = i as f32 / 29.0;
        s.add(&ss, &[InputPoint::at([from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t], f64::from(t))], set).unwrap();
    }
    let (ended, result) = s.finish(&ss, set);
    result.unwrap();
    let (inverses, problem) = ended.record(doc);
    assert!(problem.is_none());
    inverses
}

fn image(doc: &Document, layer: &str, channel: Channel) -> TiledImage {
    match &doc.texture_sets[0].layer(layer).unwrap().kind {
        LayerKind::Paint(p) => p.image(channel).unwrap().clone(),
        _ => panic!("not a paint layer"),
    }
}

fn record(doc: &Document, layer: &str) -> Vec<Stroke> {
    match &doc.texture_sets[0].layer(layer).unwrap().kind {
        LayerKind::Paint(p) => match &p.strokes {
            Some(StrokeData::Record(r)) => r.strokes.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Texels that changed from `before` to `after`, and how many of them are exactly `before`'s texel at `offset`.
fn copies(before: &TiledImage, after: &TiledImage, offset: [i32; 2]) -> (usize, usize) {
    let (mut changed, mut exact) = (0, 0);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let now = after.get(x, y);
            if now == before.get(x, y) {
                continue;
            }
            changed += 1;
            let (sx, sy) = (x as i32 + offset[0], y as i32 + offset[1]);
            if sx >= 0 && sy >= 0 && sx < SIZE as i32 && sy < SIZE as i32 && now == before.get(sx as u32, sy as u32) {
                exact += 1;
            }
        }
    }
    (changed, exact)
}

#[test]
fn clone_copies_texel_for_texel_with_an_aligned_source() {
    let (mut doc, lower, _) = document(false);
    let scene = Scene::of(&doc);
    let projector = scene.view.projector(&doc.mesh, camera()).unwrap();
    let mut source = CloneSource::default();
    source.mark("s", texel_under(&projector, [150.0, 230.0], (SIZE, SIZE)).unwrap());
    let start = |p: [f32; 2]| texel_under(&projector, p, (SIZE, SIZE)).unwrap();
    let first = source.offset_for_stroke("s", start([190.0, 140.0])).unwrap();
    assert_ne!(first, [0, 0]);

    let before = image(&doc, &lower, Channel::BaseColor);
    let retouch = Retouch { clone_offset: first, ..Retouch::default() };
    stroke(&mut doc, &scene, &lower, StrokeTool::Clone, retouch, [190.0, 140.0], [215.0, 150.0], None);
    let after = image(&doc, &lower, Channel::BaseColor);
    let (changed, exact) = copies(&before, &after, first);
    assert!(changed > 100, "the stroke cloned something: {changed}");
    assert!(exact * 10 > changed * 7, "under the hard brush the texels are exact copies: {exact} of {changed}");

    // aligned: the next stroke, elsewhere, keeps the offset
    let projector = scene.view.projector(&doc.mesh, camera()).unwrap();
    let second = source.offset_for_stroke("s", texel_under(&projector, [160.0, 120.0], (SIZE, SIZE)).unwrap()).unwrap();
    assert_eq!(second, first);
    stroke(&mut doc, &scene, &lower, StrokeTool::Clone, Retouch { clone_offset: second, ..Retouch::default() }, [160.0, 120.0], [185.0, 110.0], None);
    let (changed, exact) = copies(&after, &image(&doc, &lower, Channel::BaseColor), second);
    assert!(changed > 100 && exact * 10 > changed * 7, "{exact} of {changed}");
    let tools: Vec<StrokeTool> = record(&doc, &lower).iter().map(|s| s.tool).collect();
    assert_eq!(tools, [StrokeTool::Clone, StrokeTool::Clone]);
    assert_eq!(recorded_brush(&record(&doc, &lower)[1]).unwrap().retouch.clone_offset, first, "the offset is recorded");
}

#[test]
fn smudge_replays_bit_for_bit() {
    let (mut doc, lower, _) = document(false);
    let scene = Scene::of(&doc);
    let clean = doc.clone();
    let retouch = Retouch { strength: 0.8, ..Retouch::default() };
    stroke(&mut doc, &scene, &lower, StrokeTool::Smudge, retouch, [130.0, 150.0], [215.0, 200.0], None);
    let live = image(&doc, &lower, Channel::BaseColor);
    let (changed, _) = copies(&image(&clean, &lower, Channel::BaseColor), &live, [0, 0]);
    assert!(changed > 300, "the smudge moved paint: {changed}");
    let strokes = record(&doc, &lower);
    assert_eq!(strokes.len(), 1);
    assert_eq!(strokes[0].tool, StrokeTool::Smudge);
    let replayed = || {
        let mut d = clean.clone();
        let target = PaintTarget { layer: LayerRef::new("s", &lower), part: TargetPart::Content };
        let brush = recorded_brush(&strokes[0]).unwrap();
        let set = &mut d.texture_sets[0];
        let resolved = target.resolve(set, &recorded_values(&brush)).unwrap();
        let mut targets = target.channel_targets(set, &resolved, mode_for(strokes[0].tool, &brush.retouch)).unwrap();
        replay(&strokes[0], &scene.replay(&clean.mesh), &mut targets).unwrap();
        drop(targets);
        (image(&d, &lower, Channel::BaseColor).to_raw(), image(&d, &lower, Channel::Roughness).to_raw())
    };
    let a = replayed();
    assert_eq!(a, replayed(), "two replays give the same bits");
    let live = live.to_raw();
    let differing = a.0.iter().zip(&live).filter(|(x, y)| x != y).count();
    assert!(differing * 100 < live.len(), "replay differs from the live stroke in {differing} of {} bytes", live.len());
}

#[test]
fn see_through_smudge_is_painted_again_when_a_lower_layer_changes() {
    let (mut doc, lower, top) = document(true);
    let top = top.unwrap();
    let scene = Scene::of(&doc);
    let mut cache = SeeThroughCache::default();
    let below = BelowSource { effects: materialcraft::generators::builtin(), fills: &NO_FILL_CONTEXT };
    let refresh = |doc: &mut Document, cache: &mut SeeThroughCache| {
        let Document { mesh, texture_sets, .. } = doc;
        cache.refresh(&mut texture_sets[0], below, &scene.replay(mesh))
    };
    assert_eq!(refresh(&mut doc, &mut cache).repainted, vec![top.clone()], "a layer seen for the first time is painted");
    assert_eq!(image(&doc, &top, Channel::BaseColor).allocated_tiles(), 0, "no strokes, no pixels");

    // smudge on the see-through layer: the layer below stays, the composite shows the smudge
    let lower_before = image(&doc, &lower, Channel::BaseColor);
    let composite = |doc: &Document| Compositor::new(&doc.texture_sets[0]).composite_image(Channel::BaseColor).unwrap().0;
    let plain = composite(&doc);
    let clean = doc.clone();
    let base = cache.base(&doc.texture_sets[0], &top).cloned();
    let retouch = Retouch { strength: 0.8, ..Retouch::default() };
    let inverses = stroke(&mut doc, &scene, &top, StrokeTool::Smudge, retouch, [130.0, 150.0], [215.0, 200.0], base);
    assert_eq!(image(&doc, &lower, Channel::BaseColor), lower_before, "the layer below is left alone");
    let smudged = composite(&doc);
    let (changed, _) = copies(&plain, &smudged, [0, 0]);
    assert!(changed > 300, "the composite shows the smudge: {changed}");
    let st = image(&doc, &top, Channel::BaseColor);
    assert!(st.allocated_tiles() > 0 && st.allocated_tiles() < st.tile_count(), "pixels only where the stroke went");
    assert!(refresh(&mut doc, &mut cache).repainted.is_empty(), "nothing below changed: the cache holds");

    // one undo step takes it back
    let mut history = PaintHistory::default();
    history.record_all(&doc, "Smudge", None, inverses);
    history.close();
    history.undo(&mut doc).unwrap();
    assert_eq!(doc, clean);
    history.redo(&mut doc).unwrap();

    // painted again on the same composite, the layer matches the live stroke within rounding
    let live = image(&doc, &top, Channel::BaseColor).to_raw();
    let fresh = |doc: &mut Document| {
        let mut c = SeeThroughCache::default();
        refresh(doc, &mut c);
        image(doc, &top, Channel::BaseColor)
    };
    let again = fresh(&mut doc.clone()).to_raw();
    let differing = again.iter().zip(&live).filter(|(x, y)| x != y).count();
    assert!(differing * 100 < live.len(), "repaint differs from the live stroke in {differing} of {} bytes", live.len());

    // a change below: the smudge is painted again on the new composite
    let LayerKind::Paint(p) = &mut doc.texture_sets[0].layer_mut(&lower).unwrap().kind else { unreachable!() };
    pattern(p.image_mut(Channel::BaseColor).unwrap(), 99);
    let st_before = image(&doc, &top, Channel::BaseColor);
    assert_eq!(refresh(&mut doc, &mut cache).repainted, vec![top.clone()]);
    let st_after = image(&doc, &top, Channel::BaseColor);
    assert_ne!(st_after.to_raw(), st_before.to_raw(), "the smudge follows the new layer below");
    assert_eq!(st_after.to_raw(), fresh(&mut doc.clone()).to_raw(), "the same bits as painting it from scratch");
    let new_plain = Compositor::new(&doc.texture_sets[0]);
    let lower_now = image(&doc, &lower, Channel::BaseColor);
    // where the stroke did not go, the new layer below shows through
    let untouched = (0..SIZE).flat_map(|y| (0..SIZE).map(move |x| (x, y))).find(|&(x, y)| st_after.tile(x / 128, y / 128).is_none());
    if let Some((x, y)) = untouched {
        let c = new_plain.composite_image(Channel::BaseColor).unwrap().0.get(x, y);
        assert_eq!(c, lower_now.get(x, y));
    }
    assert!(refresh(&mut doc, &mut cache).repainted.is_empty(), "and then it holds again");
}
