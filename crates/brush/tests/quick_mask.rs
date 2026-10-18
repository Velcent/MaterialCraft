//! The quick mask painted with a brush stroke, then honoured by a document stroke (PT-42).

use materialcraft::camera::{Camera, Lens};
use materialcraft::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use materialcraft::paint::brush::BrushPreset;
use materialcraft::paint::edit::ops::{NewLayer, create};
use materialcraft::paint::edit::{LayerRef, Transaction};
use materialcraft::paint::stroke::{StrokeTool, Symmetry, ViewKind};
use materialcraft::paint::{Channel, Document, TextureSet, TiledImage};
use materialcraft::projection::{ViewProjector, ViewScene};
use materialcraft::stamp::{TexelFactor, TexelIndex};
use materialcraft_brush::{DocumentStroke, InputPoint, PaintTarget, PaintValues, QuickMask, QuickMaskStroke, SessionScene, StrokeSetup, TargetPart};

const SIZE: u32 = 128;

fn document() -> (Document, String) {
    let m = materialcraft::render3d::build_mesh(0, 32, 1.0, None, 0.0, 0.0);
    let mut doc = Document { mesh: Mesh { materials: vec!["m".into()], parts: vec![MeshPart::from_viewport("sphere", 0, &m)] }, ..Document::default() };
    doc.texture_sets.push(TextureSet::new("s", "S", (SIZE, SIZE)).with_channel(Channel::Roughness));
    let mut tx = Transaction::new(&mut doc);
    let id = create(&mut tx, "s", NewLayer::Paint, None).unwrap();
    tx.commit();
    (doc, id)
}

fn camera() -> Camera {
    let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    Camera::looking_at([0.0, 0.0, 3.0], [0.0; 3], [0.0, 1.0, 0.0], &lens, [400.0, 300.0])
}

fn setup(doc: &Document, size: f32) -> StrokeSetup {
    StrokeSetup {
        preset: BrushPreset { size, flow: 1.0, opacity: 1.0, ..BrushPreset::default() },
        tool: StrokeTool::Paint,
        seed: 1,
        symmetry: Symmetry::default(),
        view: ViewKind::Mesh,
        values: PaintValues { channels: vec![(Channel::Roughness, [1.0; 4], true)], mask: 1.0 },
        model: doc.mesh.bounding_sphere().unwrap(),
        occlusion: true,
        retouch: Default::default(),
    }
}

fn points() -> Vec<InputPoint> {
    (0..20).map(|i| InputPoint::at([150.0 + 5.0 * i as f32, 150.0], f64::from(i) * 0.01)).collect()
}

/// Paints the horizontal stroke into the paint layer's roughness through `factors`; returns the image.
fn paint_document(mut doc: Document, layer: &str, surface: (&SurfaceMap, &TexelIndex), view: &ViewScene, factors: &[&dyn TexelFactor]) -> TiledImage {
    let target = PaintTarget { layer: LayerRef::new("s", layer), part: TargetPart::Content };
    let values = PaintValues { channels: vec![(Channel::Roughness, [1.0; 4], true)], mask: 1.0 };
    let prepared = DocumentStroke::prepare(&mut doc, target, &values).unwrap();
    let setup = setup(&doc, 0.15);
    let Document { mesh, texture_sets, .. } = &mut doc;
    let projector = view.projector(mesh, camera()).unwrap();
    let scene = SessionScene { surface: surface.0, index: surface.1, projector: Some(&projector as &dyn ViewProjector), factors };
    let set = &mut texture_sets[0];
    let mut stroke = prepared.start(setup, &scene, mesh, &|_| true, set).map_err(|b| b.1).unwrap();
    stroke.add(&scene, &points(), set).unwrap();
    stroke.finish(&scene, set).1.unwrap();
    set.layer(layer).unwrap().images()[0].1.clone()
}

#[test]
fn a_painted_quick_mask_holds_back_document_strokes() {
    let (doc, layer) = document();
    let surface = SurfaceMap::build(&doc.mesh, SurfaceKey::square(&doc.mesh, 0, SIZE)).unwrap();
    let index = TexelIndex::build(&surface);
    let view = ViewScene::new(&doc.mesh, doc.mesh.fit());
    let projector = view.projector(&doc.mesh, camera()).unwrap();
    let scene = SessionScene { surface: &surface, index: &index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };

    // editing: a wider stroke along the same line protects it
    let mut mask = QuickMask::default();
    mask.toggle();
    let mut s = QuickMaskStroke::begin(&mut mask, "s", (SIZE, SIZE), setup(&doc, 0.4), &scene, &doc.mesh, &|_| true).unwrap();
    s.add(&mut mask, &scene, &points()).unwrap();
    s.finish(&mut mask, &scene).unwrap();
    let protected = (0..SIZE * SIZE).filter(|i| mask.protection("s", i % SIZE, i / SIZE) > 0.999).count();
    assert!(protected > 50, "{protected}");
    assert!(doc.texture_sets[0].layer(&layer).unwrap().images()[0].1.allocated_tiles() == 0, "the document is untouched");

    let free = paint_document(doc.clone(), &layer, (&surface, &index), &view, &[]);
    let painted = |img: &TiledImage| (0..SIZE * SIZE).filter(|i| img.get(i % SIZE, i / SIZE)[3] > 0.01).count();
    assert!(painted(&free) > 20);

    mask.apply();
    let factor = mask.factor("s", (SIZE, SIZE)).unwrap();
    let held = paint_document(doc.clone(), &layer, (&surface, &index), &view, &[&factor]);
    for i in 0..SIZE * SIZE {
        let (x, y) = (i % SIZE, i / SIZE);
        if mask.protection("s", x, y) > 0.999 {
            assert!(held.get(x, y)[3] < 1e-3, "protected texel {x},{y} got paint");
        }
    }
    assert!(painted(&held) < painted(&free) / 4, "{} vs {}", painted(&held), painted(&free));
}
