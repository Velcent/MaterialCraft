//! Painted masks replayed at import: 2D-view and 3D-view strokes land where Painter put them, the mask becomes
//! computable ("replaced"), and what cannot be replayed keeps the mask not computable with the reason.

use super::fixtures::*;
use crate::model::*;
use crate::translate::layers::decal::geometry::SetGeometry;
use crate::translate::layers::replay::{REPLAYED, ReplayChoice};
use crate::translate::layers::{BlendTable, ImportedStack, LayerImport, NoImages};
use materialcraft::camera::{Camera, Lens};
use materialcraft::mesh::{Mesh, MeshPart};
use materialcraft::paint::{Channel, EffectKind};

const RES: (u32, u32) = (64, 64);

/// One quad over the whole UV square (glTF V down: position y 1 is V 0).
fn quad() -> Mesh {
    Mesh {
        materials: vec!["m".into()],
        parts: vec![MeshPart {
            name: "quad".into(),
            material: 0,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            tangents: vec![],
            uv_sets: vec![vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            faces: Default::default(),
        }],
    }
}

fn shape_alpha(hardness: f32) -> PainterSource {
    PainterSource::Procedural(ProceduralSource {
        uid: None,
        channels: ChannelMask(u64::MAX),
        mode: 0,
        procedural: Some(Procedural {
            uid: None,
            url: "resource://tool/shape/shape?version=1.sbsar".into(),
            tweaks: vec![Tweak {
                identifier: "hardness".into(),
                value: TweakValue::Floats(vec![hardness]),
            }],
            outputs: vec![],
            inputs: vec![],
        }),
    })
}

fn stroke(view: PainterView, points: &[[f32; 2]], camera: Option<&Camera>) -> PainterStroke {
    PainterStroke {
        uid: None,
        points: points
            .iter()
            .enumerate()
            .map(|(i, p)| StrokeKey {
                position: *p,
                time: i as f32 * 0.02,
            })
            .collect(),
        pressure: vec![],
        tilt_keys: 0,
        complete: true,
        view_matrix: camera.map_or([0.0; 16], |c| c.view),
        projection_matrix: camera.map_or([0.0; 16], |c| c.projection),
        view,
        duplicate_count: 0,
        duplicate_matrix: [0.0; 16],
        random_seed: 7,
        lazy_distance: 0.0,
    }
}

fn paint(strokes: Vec<PainterStroke>, size: f32, alpha: PainterSource) -> PainterEffect {
    PainterEffect::Paint(PaintEffect {
        header: header(0, vec![]),
        tool: PaintTool::Brush,
        opacity: 1.0,
        strokes,
        path_strokes: 0,
        brush_uid: Some(5),
        brush: Some(PainterBrush {
            size,
            ..PainterBrush::default()
        }),
        stamp_blending: 0,
        colors: vec![uniform(0, [1.0; 3])],
        alpha: vec![alpha],
        stencil: None,
    })
}

/// A roughness 0 base under a roughness 1 layer whose black mask is painted with `effect`.
fn layers(effect: PainterEffect) -> Vec<PainterLayer> {
    let base = layer(1, "Base", vec![fill(vec![uniform(ROUGH, [0.0; 3])])], vec![]);
    let mut top = layer(9, "Top", vec![fill(vec![uniform(ROUGH, [1.0; 3])])], vec![]);
    top.mask = Some(PainterMask {
        enabled: true,
        initial: MaskInitial::Black,
        effects: vec![PainterEffect::Group(EffectGroup {
            header: header(0, vec![]),
            effects: vec![effect],
        })],
    });
    vec![base, top]
}

fn import(layers: Vec<PainterLayer>, replay: ReplayChoice) -> ImportedStack {
    let mesh = quad();
    let geometry = SetGeometry::new(&mesh, &[], &[0]);
    let table = BlendTable::painter();
    let plan = crate::translate::replacements::ProceduralPlan::default();
    let projections = crate::translate::layers::ProjectionTable::painter();
    LayerImport {
        table: &table,
        projections: &projections,
        images: &NoImages,
        plan: &plan,
        replay,
        replay_cache: None,
    }
    .import_on(&set(layers), &channels(), RES, Some(&geometry), None)
}

/// The mask value our replay painted at texel (x, y) (grey × coverage of the mask's paint effects).
fn painted(stack: &ImportedStack, x: u32, y: u32) -> f32 {
    let mask = stack.layers[1].mask.as_ref().unwrap();
    mask.effects
        .iter()
        .filter_map(|e| match &e.kind {
            EffectKind::Paint(p) => Some(p.image.get(x, y)),
            _ => None,
        })
        .map(|p| p[0] * p[3])
        .fold(0.0, f32::max)
}

#[test]
fn a_2d_view_stroke_paints_where_painter_put_it() {
    // Painter's 2D view (V down, as ours) 0.25, 0.25: the upper left quarter, texel (16, 16)
    let effect = paint(
        vec![stroke(PainterView::Uv, &[[0.2, 0.25], [0.3, 0.25]], None)],
        0.15,
        shape_alpha(1.0),
    );
    let stack = import(layers(effect.clone()), ReplayChoice::Replay);
    let status = stack.verdict.channel(Channel::Roughness);
    assert!(status.is_exact_or_replaced() && !status.is_exact(), "{status:?}");
    assert!(status.reasons()[0].starts_with(REPLAYED), "{status:?}");
    assert!(painted(&stack, 16, 16) > 0.99, "under the stroke");
    assert!(
        painted(&stack, 16, 48) < 0.01 && painted(&stack, 48, 16) < 0.01,
        "away from it"
    );
    // and the composite shows the masked layer there
    let mut set = materialcraft::paint::TextureSet::new("t", "T", RES);
    set.channels = channels().specs().to_vec();
    set.layers = stack.layers.clone();
    let (tex, _) = materialcraft::paint::Compositor::new(&set).composite_tex(Channel::Roughness);
    assert!(
        tex.px[16 * 64 + 16][0] > 0.99 && tex.px[48 * 64 + 16][0] < 0.01,
        "{:?}",
        tex.px[16 * 64 + 16]
    );
    // the record is kept next to the pixels
    let mask = stack.layers[1].mask.as_ref().unwrap();
    assert!(
        mask.effects
            .iter()
            .any(|e| matches!(&e.kind, EffectKind::Paint(p) if p.strokes.is_some()))
    );
    // without replay the mask stays empty and not computable
    let kept = import(layers(effect), ReplayChoice::Keep);
    assert!(!kept.verdict.channel(Channel::Roughness).is_computable());
}

#[test]
fn a_3d_view_stroke_goes_through_its_stored_camera() {
    let lens = Lens::Perspective {
        fov_y: 0.6,
        near: 0.05,
        far: 20.0,
    };
    let camera = Camera::looking_at([0.5, 0.5, 3.0], [0.5, 0.5, 0.0], [0.0, 1.0, 0.0], &lens, [1.0, 1.0]);
    // the middle of the screen (Painter's 0 … 1, y up) is the middle of the quad; a little right and up from it is
    // right and up on the quad (x 0.6, y 0.6 → texel (38, 25) with V down)
    let at = camera.project([0.6, 0.6, 0.0]).unwrap();
    let ndc = camera.screen_to_ndc(at.screen);
    let screen01 = [(ndc[0] + 1.0) / 2.0, (ndc[1] + 1.0) / 2.0];
    let effect = paint(
        vec![stroke(PainterView::Mesh, &[screen01], Some(&camera))],
        0.05,
        shape_alpha(1.0),
    );
    let stack = import(layers(effect), ReplayChoice::Replay);
    assert!(stack.verdict.channel(Channel::Roughness).is_exact_or_replaced());
    assert!(painted(&stack, 38, 25) > 0.99, "under the click");
    assert!(painted(&stack, 25, 38) < 0.01, "mirrored place stays black");
}

#[test]
fn a_paint_layer_gets_its_brush_work_as_pixels() {
    let mut brush = paint(
        vec![stroke(PainterView::Uv, &[[0.2, 0.25], [0.3, 0.25]], None)],
        0.15,
        shape_alpha(1.0),
    );
    if let PainterEffect::Paint(p) = &mut brush {
        p.header = header(ALL, vec![]);
        p.colors = vec![uniform(ROUGH, [0.8; 3])];
    }
    let base = layer(1, "Base", vec![fill(vec![uniform(ROUGH, [0.0; 3])])], vec![]);
    let mut painted = layer(
        2,
        "Paint",
        vec![PainterEffect::Group(EffectGroup {
            header: header(ALL, vec![blend(ALL, 25, 1.0)]),
            effects: vec![brush],
        })],
        vec![],
    );
    painted.kind = crate::model::LayerKind::Paint;
    let stack = import(vec![base.clone(), painted.clone()], ReplayChoice::Replay);
    let status = stack.verdict.channel(Channel::Roughness);
    assert!(status.is_exact_or_replaced() && !status.is_exact(), "{status:?}");
    let materialcraft::paint::LayerKind::Paint(content) = &stack.layers[1].kind else {
        panic!("not a paint layer")
    };
    let image = content.image(Channel::Roughness).expect("roughness pixels");
    assert!(content.strokes.is_some());
    let (under, away) = (image.get(16, 16), image.get(48, 48));
    assert!((under[0] - 0.8).abs() < 0.01 && under[3] > 0.99, "{under:?}");
    assert!(away[3] < 0.01, "{away:?}");
    let kept = import(vec![base, painted], ReplayChoice::Keep);
    assert!(!kept.verdict.channel(Channel::Roughness).is_computable());
}

#[test]
fn strokes_that_cannot_be_replayed_keep_the_mask_not_computable() {
    let other_alpha = PainterSource::Procedural(ProceduralSource {
        uid: None,
        channels: ChannelMask(u64::MAX),
        mode: 0,
        procedural: Some(Procedural {
            uid: None,
            url: "resource://shelf/drips?version=1.sbsar".into(),
            tweaks: vec![],
            outputs: vec![],
            inputs: vec![],
        }),
    });
    let effect = paint(vec![stroke(PainterView::Uv, &[[0.5, 0.5]], None)], 0.1, other_alpha);
    let stack = import(layers(effect), ReplayChoice::Replay);
    let status = stack.verdict.channel(Channel::Roughness);
    assert!(!status.is_computable());
    assert!(
        status.reasons().iter().any(|r| r.contains("brush alpha \"drips\"")),
        "{status:?}"
    );
}
