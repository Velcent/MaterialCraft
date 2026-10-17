//! Native effect kinds in the compositor: levels, colour selection, polygon fill, anchors and references, and
//! effect layers.

use super::*;
use crate::mesh::{Mesh, SurfaceKey, SurfaceMap};
use crate::paint::effect::{Anchor, ColorSelection, Levels, Pick, PickMode, PolygonFill, Reference, Selection};
use crate::paint::{BakedKind, BakedMap, ContentEffect, EffectKind, Fill, GreyBlend, Mask, MaskEffect, PaintContent, PixelFormat};

const R: Channel = Channel::Roughness;

fn set(w: u32, h: u32) -> TextureSet {
    TextureSet::new("s", "S", (w, h)).with_channel(R)
}

fn fill_layer(id: &str, v: f32) -> Layer {
    Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(R, [v, v, v, 1.0])]))).with_blend(R, BlendMode::Normal, 1.0)
}

fn effect(kind: EffectKind<crate::paint::effect::Content>) -> ContentEffect {
    ContentEffect::new(Vec::new(), kind).with_blend(R, BlendMode::Normal, 1.0)
}

fn grey(kind: EffectKind<crate::paint::effect::Grey>) -> MaskEffect {
    MaskEffect::new(GreyBlend::default(), kind)
}

fn at(c: &Compositor, x: usize, y: usize) -> f32 {
    let (tex, _) = c.composite_tex(R);
    tex.px[y * tex.w + x][0]
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn invert() -> Levels {
    Levels { output_min: [1.0; 3], output_max: [0.0; 3], ..Levels::default() }
}

#[test]
fn levels_transform_content_and_masks() {
    let mut s = set(2, 2);
    s.layers.push(fill_layer("base", 0.25).with_effect(effect(EffectKind::Levels(invert()))));
    assert!(close(at(&Compositor::new(&s), 0, 0), 0.75));
    assert!(Compositor::new(&s).warnings(R).is_empty());
    // A mask of 0.25 inverted to 0.75 shows 75 % of the top layer over the bottom one.
    let mut s = set(2, 2);
    s.layers.push(fill_layer("below", 0.0));
    let mut top = fill_layer("top", 1.0);
    top.mask = Some(Mask::new(0.0).with_effect(grey(EffectKind::Fill(0.25))).with_effect(grey(EffectKind::Levels(invert()))));
    s.layers.push(top);
    assert!(close(at(&Compositor::new(&s), 1, 1), 0.75));
}

#[test]
fn colour_selection_reads_the_baked_id_map() {
    let mut s = set(2, 1);
    let mut id = TiledImage::new(2, 1, PixelFormat::from_id("rgba8").unwrap()).unwrap();
    id.fill([1.0, 0.0, 0.0, 1.0]);
    id.update_tiles(&id.all_tiles(), |r, px| {
        for (i, p) in px.iter_mut().enumerate() {
            if r.x as usize + i == 1 {
                *p = [0.0, 0.0, 1.0, 1.0];
            }
        }
    });
    s.layers.push(fill_layer("below", 0.0));
    let mut top = fill_layer("top", 1.0);
    let select = ColorSelection { map: BakedKind::Id, colors: vec![[0.0, 0.0, 1.0]], tolerance: 0.1, hardness: 1.0 };
    top.mask = Some(Mask::new(0.0).with_effect(grey(EffectKind::ColorSelection(select))));
    s.layers.push(top);
    // Without the map the selection is not computable and left out.
    assert!(close(at(&Compositor::new(&s), 1, 0), 0.0));
    assert_eq!(Compositor::new(&s).warnings(R).len(), 1);
    s.baked.push(BakedMap { kind: BakedKind::Id, image: id });
    let c = Compositor::new(&s);
    assert_eq!((at(&c, 0, 0), at(&c, 1, 0)), (0.0, 1.0));
    assert!(c.warnings(R).is_empty());
}

fn quad_surface(size: u32) -> SurfaceMap {
    let mesh = Mesh { materials: vec!["m".into()], parts: vec![crate::mesh::test_util::quad()] };
    SurfaceMap::build(&mesh, SurfaceKey { material: 0, uv_set: 0, width: size, height: size, gutter: 0, mesh_revision: 0 }).unwrap()
}

#[test]
fn polygon_fill_writes_the_picked_triangles() {
    // Triangle 0 of the quad covers the lower-right half of UV space (u + v > 1), triangle 1 the upper left.
    let surface = quad_surface(4);
    let fills = FillContext::with_sources(&NoFillSources).with_surface(&surface, None);
    let selection = Selection { mode: PickMode::Triangle, picks: vec![Pick { part: "quad".into(), triangles: vec![0] }] };
    let mut s = set(4, 4);
    s.layers.push(fill_layer("below", 0.0));
    let mut top = fill_layer("top", 1.0);
    top.mask = Some(Mask::new(0.0).with_effect(grey(EffectKind::PolygonFill(PolygonFill { selection: selection.clone(), values: 1.0 }))));
    s.layers.push(top);
    // Without a surface map: not computable.
    assert_eq!(Compositor::new(&s).warnings(R).len(), 1);
    let c = Compositor::new(&s).with_fills(&fills);
    assert!(c.warnings(R).is_empty());
    assert_eq!((at(&c, 3, 2), at(&c, 0, 0)), (1.0, 0.0));
    // In content: the fill's values on the picked triangles only.
    let mut s = set(4, 4);
    let values = Fill::uniform(vec![(R, [0.5, 0.5, 0.5, 1.0])]);
    s.layers.push(fill_layer("base", 0.1).with_effect(effect(EffectKind::PolygonFill(PolygonFill { selection, values }))));
    let c = Compositor::new(&s).with_fills(&fills);
    assert!(close(at(&c, 3, 2), 0.5));
    assert!(close(at(&c, 0, 0), 0.1));
}

#[test]
fn references_read_the_stack_up_to_the_anchor() {
    let mut s = set(2, 2);
    // The source layer: 0.2, anchored, then raised to 0.9 above the anchor.
    let source = fill_layer("src", 0.2)
        .with_effect(effect(EffectKind::Anchor(Anchor { id: "a".into() })))
        .with_effect(effect(EffectKind::Fill(Fill::uniform(vec![(R, [0.9, 0.9, 0.9, 1.0])]))));
    s.layers.push(source);
    let reader = Layer::new("rd", "Reader", LayerKind::Paint(PaintContent::default()))
        .with_blend(R, BlendMode::Normal, 1.0)
        .with_effect(effect(EffectKind::Reference(Reference::new("a"))));
    s.layers.push(reader);
    assert!(close(at(&Compositor::new(&s), 0, 0), 0.2));
    // Through levels, and from a mask.
    s.layers[1].effects[0].kind = EffectKind::Reference(Reference { levels: Some(invert()), ..Reference::new("a") });
    assert!(close(at(&Compositor::new(&s), 0, 0), 0.8));
    let mut masked = fill_layer("top", 1.0);
    masked.mask = Some(Mask::new(0.0).with_effect(grey(EffectKind::Reference(Reference::new("a")))));
    s.layers.push(masked);
    assert!(close(at(&Compositor::new(&s), 0, 0), 0.8 + 0.2 * 0.2));
    // A missing anchor is reported; a reference reading itself stops.
    s.layers[2].mask = Some(Mask::new(0.0).with_effect(grey(EffectKind::Reference(Reference::new("gone")))));
    assert_eq!(Compositor::new(&s).warnings(R).len(), 1);
    let mut s = set(1, 1);
    let looped = fill_layer("loop", 0.3)
        .with_effect(effect(EffectKind::Anchor(Anchor { id: "l".into() })))
        .with_effect(effect(EffectKind::Reference(Reference::new("l"))));
    s.layers.push(looped);
    let mut l2 = fill_layer("l2", 0.5);
    l2.effects.push(effect(EffectKind::Reference(Reference::new("l"))));
    l2.effects.push(effect(EffectKind::Anchor(Anchor { id: "m".into() })));
    s.layers.push(l2);
    assert!(close(at(&Compositor::new(&s), 0, 0), 0.3));
}

#[test]
fn effect_layers_adjust_the_composite_below() {
    let mut s = set(2, 2);
    s.layers.push(fill_layer("below", 0.25));
    let adjust = Layer::new("fx", "Levels", LayerKind::Paint(PaintContent::default()))
        .with_blend(R, BlendMode::Passthrough, 0.5)
        .with_effect(effect(EffectKind::Levels(invert())));
    s.layers.push(adjust);
    // Half of the inverted 0.25 over 0.25.
    assert!(close(at(&Compositor::new(&s), 0, 0), 0.5));
    s.layers.push(fill_layer("above", 1.0).with_blend(R, BlendMode::Multiply, 1.0));
    assert!(close(at(&Compositor::new(&s), 1, 1), 0.5));
}
