//! Projection painting into a document (PJ-26): the stroke lays the projected image's values into the channels the
//! projection provides, leaves the others alone, is recorded as a projection stroke and is not replayed as a plain
//! one.

use materialcraft::camera::{Camera, Lens};
use materialcraft::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use materialcraft::paint::brush::BrushPreset;
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::edit::Transaction;
use materialcraft::paint::edit::ops::{NewLayer, create};
use materialcraft::paint::stroke::{StrokeData, StrokeTool, Symmetry, ViewKind};
use materialcraft::paint::{Channel, Document, LayerKind, TextureSet};
use materialcraft::projection::{Filter, ImageLevels, LevelChain, Repeat, ScreenImageTransform, ViewProjector, ViewScene};
use materialcraft::stamp::{ProjectedPaint, TexelIndex};
use materialcraft::tex::Tex;
use materialcraft_brush::{BrushError, DocumentStroke, InputPoint, PaintTarget, PaintValues, ReplayScene, SessionScene, StrokeSetup, TargetPart, replay};

const SIZE: u32 = 128;

fn document() -> (Document, String) {
    let m = materialcraft::render3d::build_mesh(0, 32, 1.0, None, 0.0, 0.0);
    let mesh = Mesh { materials: vec!["m".into()], parts: vec![MeshPart::from_viewport("sphere", 0, &m)] };
    let mut doc = Document { mesh, ..Document::default() };
    doc.texture_sets.push(TextureSet::new("s", "S", (SIZE, SIZE)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness));
    let mut tx = Transaction::new(&mut doc);
    let id = create(&mut tx, "s", NewLayer::Paint, None).unwrap();
    tx.commit();
    (doc, id)
}

fn camera() -> Camera {
    let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    Camera::looking_at([0.0, 0.0, 3.0], [0.0; 3], [0.0, 1.0, 0.0], &lens, [320.0, 240.0])
}

#[test]
fn a_projection_stroke_paints_the_images_channels_only_and_is_recorded_as_one() {
    let (mut doc, layer) = document();
    let map = SurfaceMap::build(&doc.mesh, SurfaceKey::square(&doc.mesh, 0, SIZE)).unwrap();
    let index = TexelIndex::build(&map);
    let view = ViewScene::new(&doc.mesh, doc.mesh.fit());
    let target = PaintTarget { layer: LayerRef::new("s", &layer), part: TargetPart::Content };
    // the projection provides base colour only: roughness is switched off for the stroke
    let values = PaintValues { channels: vec![(Channel::BaseColor, [1.0; 4], true), (Channel::Roughness, [0.3; 4], false)], mask: 1.0 };
    let tex = Tex { w: 1, h: 2, px: vec![[0.2, 0.4, 0.6, 1.0], [0.8, 0.6, 0.4, 1.0]] };
    let chain = LevelChain::default();
    let placement = ScreenImageTransform { centre: [160.0, 120.0], size: [120.0, 80.0], rotation: 0.0, repeat: Repeat::None };

    let prepared = DocumentStroke::prepare(&mut doc, target.clone(), &values).unwrap();
    let Document { mesh, texture_sets, .. } = &mut doc;
    let projector = view.projector(mesh, camera()).unwrap();
    let projected = ProjectedPaint::new(&map, &projector, placement, Filter::Nearest).with_channel(Channel::BaseColor, ImageLevels::new(&tex, &chain));
    let scene = SessionScene { surface: &map, index: &index, projector: Some(&projector as &dyn ViewProjector), factors: &[], projected: Some(&projected) };
    let setup = StrokeSetup {
        preset: BrushPreset { size: 400.0, size_space: materialcraft::paint::brush::SizeSpace::Screen, flow: 1.0, opacity: 1.0, ..BrushPreset::default() },
        tool: StrokeTool::Projection,
        seed: 1,
        symmetry: Symmetry::default(),
        view: ViewKind::Mesh,
        values: values.clone(),
        model: mesh.bounding_sphere().unwrap(),
        occlusion: true,
        retouch: Default::default(),
    };
    let set = &mut texture_sets[0];
    let mut stroke = prepared.start(setup, &scene, mesh, &|_| true, set).map_err(|b| b.1).unwrap();
    stroke.add(&scene, &[InputPoint::at([160.0, 120.0], 0.0)], set).unwrap();
    let (ended, result) = stroke.finish(&scene, set);
    result.unwrap();
    let (_, problem) = ended.record(&mut doc);
    assert!(problem.is_none());

    let LayerKind::Paint(p) = &doc.texture_sets[0].layer(&layer).unwrap().kind else { panic!("not a paint layer") };
    assert!(p.image(Channel::Roughness).is_none_or(|r| (0..SIZE).all(|y| (0..SIZE).all(|x| r.get(x, y)[3] == 0.0))), "roughness untouched");
    let base = p.image(Channel::BaseColor).unwrap();
    let (mut top, mut bottom) = (0, 0);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = base.get(x, y);
            if px[3] > 0.999 {
                let rgb = [px[0], px[1], px[2]];
                let near = |c: [f32; 4]| (0..3).all(|i| (rgb[i] - c[i]).abs() < 0.01);
                assert!(near(tex.px[0]) || near(tex.px[1]), "({x}, {y}) {px:?} is one of the image's values");
                top += usize::from(near(tex.px[0]));
                bottom += usize::from(near(tex.px[1]));
            }
        }
    }
    assert!(top > 50 && bottom > 50, "both halves of the image landed: {top} {bottom}");

    let Some(StrokeData::Record(record)) = p.strokes.clone() else { panic!("no stroke record") };
    let stroke = &record.strokes[0];
    assert_eq!(stroke.tool, StrokeTool::Projection);
    let mut d = doc.clone();
    let set = &mut d.texture_sets[0];
    let resolved = target.resolve(set, &values).unwrap();
    let mut targets = target.channel_targets(set, &resolved, materialcraft::stamp::StampMode::Paint).unwrap();
    let scene = ReplayScene { mesh: &doc.mesh, view: &view, surface: &map, index: &index, factors: &[] };
    assert!(matches!(replay(stroke, &scene, &mut targets), Err(BrushError::NotReplayable(_))));
}
