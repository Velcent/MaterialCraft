//! Native effects through the whole import: levels, effect layers, anchor references, several fills in one layer,
//! colour selection and polygon fill, each with its verdict and its composite.

use super::fixtures::*;
use crate::model::*;
use crate::translate::layers::decal::geometry::SetGeometry;
use crate::translate::layers::{BlendTable, ImportedStack, LayerImport, NoImages};
use materialcraft::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use materialcraft::paint::{
    BakedKind, BakedMap, Channel, Compositor, FillContext, NoFillSources, PixelFormat, TextureSet, TiledImage,
};

fn invert() -> Levels {
    Levels {
        input_minimum: [0.0; 3],
        input_maximum: [1.0; 3],
        gamma: [1.0; 3],
        output_minimum: [1.0; 3],
        output_maximum: [0.0; 3],
        clamp: 1,
    }
}

fn levels(channels: u64) -> PainterEffect {
    PainterEffect::Levels(LevelsEffect {
        header: header(channels, vec![]),
        levels: Some(invert()),
    })
}

fn anchor(uid: u64) -> PainterEffect {
    let mut h = header(0, vec![]);
    h.uid = Some(uid);
    PainterEffect::Anchor(h)
}

fn reference(anchor_uid: u64, read: u64) -> PainterSource {
    PainterSource::Reference(ReferenceSource {
        uid: None,
        channels: ChannelMask(0),
        anchor_uid: Some(anchor_uid),
        anchor_label: String::new(),
        levels: None,
        bindings: vec![(read, 0)],
        mode: 2,
    })
}

fn masked(effects: Vec<PainterEffect>) -> PainterLayer {
    let mut l = layer(9, "Top", vec![fill(vec![uniform(ROUGH, [1.0; 3])])], vec![]);
    l.mask = Some(PainterMask {
        enabled: true,
        initial: MaskInitial::Black,
        effects,
    });
    l
}

fn mask_fill(source: PainterSource) -> PainterEffect {
    PainterEffect::Fill(FillEffect {
        header: header(0, vec![blend(0, 2, 1.0)]),
        sources: vec![source],
        projection: 0,
        filtering: 2,
        placement: Default::default(),
        uv_transform: None,
    })
}

fn base(v: f32) -> PainterLayer {
    layer(1, "Base", vec![fill(vec![uniform(ROUGH, [v; 3])])], vec![])
}

fn rough(stack: &ImportedStack) -> f32 {
    composite(stack, Channel::Roughness, 1, 1)[0]
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn levels_in_content_and_masks_are_exact() {
    let content = import(vec![layer(
        1,
        "L",
        vec![fill(vec![uniform(ROUGH, [0.25; 3])]), levels(ROUGH)],
        vec![],
    )]);
    assert!(content.verdict.channel(Channel::Roughness).is_exact());
    assert!(close(rough(&content), 0.75));
    let quarter = mask_fill(uniform(0, [0.25; 3]));
    let mask = import(vec![base(0.0), masked(vec![quarter, levels(0)])]);
    assert!(mask.verdict.all_exact());
    assert!(close(rough(&mask), 0.75));
}

#[test]
fn a_pass_through_layer_with_levels_adjusts_what_is_below() {
    let mut adjust = layer(
        2,
        "Invert",
        vec![
            PainterEffect::Group(EffectGroup {
                header: header(0, vec![]),
                effects: vec![],
            }),
            levels(ROUGH),
        ],
        vec![blend(ALL, 25, 1.0)],
    );
    adjust.kind = LayerKind::Paint;
    let stack = import(vec![base(0.25), adjust]);
    assert!(stack.verdict.channel(Channel::Roughness).is_exact());
    assert!(close(rough(&stack), 0.75));
}

#[test]
fn references_read_anchors_in_content_and_masks() {
    let source = layer(
        3,
        "Source",
        vec![fill(vec![uniform(ROUGH, [0.4; 3])]), anchor(77)],
        vec![],
    );
    let stack = import(vec![base(0.0), source, masked(vec![mask_fill(reference(77, ROUGH))])]);
    assert!(stack.verdict.all_exact(), "{:?}", stack.verdict.channels());
    // the mask reads 0.4 from the anchor: 0.4 × 1 + 0.6 × 0.4
    assert!(close(rough(&stack), 0.4 + 0.6 * 0.4));
    let missing = import(vec![base(0.0), masked(vec![mask_fill(reference(78, ROUGH))])]);
    assert!(!missing.verdict.channel(Channel::Roughness).is_computable());
    // an anchor over something not computable passes that on
    let opaque = layer(
        3,
        "Source",
        vec![fill(vec![procedural(ROUGH, "moss")]), anchor(77)],
        vec![],
    );
    let stack = import(vec![opaque, masked(vec![mask_fill(reference(77, ROUGH))])]);
    assert!(!stack.verdict.channel(Channel::Roughness).is_computable());
}

#[test]
fn a_fill_over_levels_in_one_layer_stacks() {
    let half = PainterEffect::Fill(FillEffect {
        header: header(ALL, vec![blend(ROUGH, 2, 0.5)]),
        sources: vec![uniform(ROUGH, [0.5; 3])],
        projection: 0,
        filtering: 2,
        placement: Default::default(),
        uv_transform: None,
    });
    let content = vec![fill(vec![uniform(ROUGH, [0.25; 3])]), levels(ROUGH), half];
    let stack = import(vec![layer(1, "L", content, vec![])]);
    assert!(stack.verdict.channel(Channel::Roughness).is_exact());
    assert!(close(rough(&stack), 0.625));
}

#[test]
fn colour_selection_selects_on_the_id_map() {
    let select = PainterEffect::ColorSelection(ColorSelectionEffect {
        header: header(0, vec![blend(0, 2, 1.0)]),
        colors: vec![[0.0, 0.0, 1.0]],
        tolerance: 0.1,
        hardness: 1.0,
        bitmap_url: String::new(),
    });
    let stack = import(vec![base(0.0), masked(vec![select])]);
    assert!(stack.verdict.all_exact());
    let mut set = TextureSet::new("t", "T", SIZE);
    set.channels = channels().specs().to_vec();
    set.layers = stack.layers.clone();
    let mut id = TiledImage::new(SIZE.0, SIZE.1, PixelFormat::from_id("rgba8").unwrap()).unwrap();
    id.fill([0.0, 0.0, 1.0, 1.0]);
    set.baked.push(BakedMap {
        kind: BakedKind::Id,
        image: id,
    });
    let (tex, warnings) = Compositor::new(&set).composite_tex(Channel::Roughness);
    assert!(warnings.is_empty());
    assert!(close(tex.px[0][0], 1.0));
}

/// One quad over the whole UV square, two triangles (one source polygon).
pub(super) fn quad() -> Mesh {
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

#[test]
fn polygon_fill_picks_are_resolved_on_the_mesh() {
    let click = |q: [f32; 2], color: f32| DecalHit {
        uid: None,
        shape: HitShape::Point(q),
        color: [color; 3],
        granularity: HitGranularity::Triangle,
        view: PainterView::Uv,
        proj_view_inv: [0.0; 16],
        duplicate_count: 0,
        duplicate_matrix: [0.0; 16],
        paint_through: false,
    };
    // Painter UV (V up): triangle 0 is the lower right half; white on it, then black on the other, then white again.
    let decal = PainterEffect::PolygonFill(PolygonFillEffect {
        header: header(ALL, vec![]),
        hits: vec![click([0.9, 0.1], 1.0), click([0.1, 0.9], 0.0), click([0.1, 0.9], 1.0)],
        unread: 0,
    });
    let group = PainterEffect::Group(EffectGroup {
        header: header(0, vec![]),
        effects: vec![decal],
    });
    let layers = vec![base(0.0), masked(vec![group])];
    let mesh = quad();
    let geometry = SetGeometry::new(&mesh, &[], &[0]);
    let table = BlendTable::painter();
    let import = LayerImport {
        table: &table,
        projections: &crate::translate::layers::ProjectionTable::painter(),
        images: &NoImages,
        plan: &crate::translate::replacements::ProceduralPlan::default(),
    };
    let stack = import.import_on(&set(layers.clone()), &channels(), SIZE, Some(&geometry), None);
    assert!(stack.verdict.all_exact(), "{:?}", stack.verdict.channels());
    let without = import.import(&set(layers), &channels(), SIZE);
    assert!(
        !without.verdict.channel(Channel::Roughness).is_computable(),
        "no mesh, no picks"
    );
    let surface = SurfaceMap::build(
        &mesh,
        SurfaceKey {
            material: 0,
            uv_set: 0,
            width: SIZE.0,
            height: SIZE.1,
            gutter: 0,
            mesh_revision: 0,
        },
    )
    .unwrap();
    let fills = FillContext::with_sources(&NoFillSources).with_surface(&surface, None);
    let mut set = TextureSet::new("t", "T", SIZE);
    set.channels = channels().specs().to_vec();
    set.layers = stack.layers.clone();
    let (tex, warnings) = Compositor::new(&set)
        .with_fills(&fills)
        .composite_tex(Channel::Roughness);
    assert!(warnings.is_empty(), "{warnings:?}");
    // texel (6, 6) is in the lower right half (glTF V down), (1, 1) in the upper left; both picked white in the end
    assert!(close(tex.px[6 * 8 + 6][0], 1.0));
    assert!(close(tex.px[8 + 1][0], 1.0));
}

#[test]
fn content_filters_follow_the_replacement_plan() {
    use crate::translate::replacements::{Choice, ProceduralPlan};
    let invert = PainterEffect::Filter(FilterEffect {
        header: header(ROUGH, vec![]),
        procedural: Some(Procedural {
            uid: None,
            url: "resource://shelf/invert?version=1.sbsar".into(),
            tweaks: vec![],
            outputs: vec![],
            inputs: vec![],
        }),
        inputs: vec![],
    });
    let layers = vec![layer(
        1,
        "L",
        vec![fill(vec![uniform(ROUGH, [0.25; 3])]), invert],
        vec![],
    )];
    let table = BlendTable::painter();
    let import = |plan: &ProceduralPlan| {
        LayerImport {
            table: &table,
            projections: &crate::translate::layers::ProjectionTable::painter(),
            images: &NoImages,
            plan,
        }
        .import(&set(layers.clone()), &channels(), SIZE)
    };
    let kept = import(&ProceduralPlan::default());
    assert!(!kept.verdict.channel(Channel::Roughness).is_computable());
    let replaced = import(&ProceduralPlan::all(Choice::Replace));
    let status = replaced.verdict.channel(Channel::Roughness);
    assert!(status.is_exact_or_replaced() && !status.is_exact(), "{status:?}");
    let l = &replaced.layers[0];
    assert!(
        matches!(&l.effects[0].kind, materialcraft::paint::EffectKind::Filter(p) if !p.id.starts_with("substance:"))
    );
    let b = l.effects[0].blend_for(Channel::Roughness).unwrap();
    assert_eq!(
        b.mode,
        materialcraft::paint::BlendMode::Normal,
        "filters without a stored list replace their input"
    );
}
