//! A small document built in code, for trying Paint mode without files (`--paint-demo`): a red
//! plastic sphere and a blue metal torus with painted height stripes, each its own texture set.

use materialcraft::mesh::{Mesh, MeshPart};
use materialcraft::paint::{BlendMode, Channel, ChannelSpec, Document, Fill, Layer, LayerKind, PaintContent, TextureSet, TiledImage};
use materialcraft::render3d::build_mesh;
use materialcraft::tex::Tex;

const RESOLUTION: u32 = 512;
const SHAPE_SPHERE: u32 = 0;
const SHAPE_TORUS: u32 = 12;

pub fn document() -> Document {
    let mesh = Mesh { materials: vec!["Ball".into(), "Ring".into()], parts: vec![part("Ball", 0, SHAPE_SPHERE, -1.1), part("Ring", 1, SHAPE_TORUS, 1.1)] };
    let mut doc = Document::new(mesh);
    doc.texture_sets.push(uniform_set("ball", "Ball", 0, [0.8, 0.2, 0.2, 1.0], 0.4, 0.0));
    let mut ring = uniform_set("ring", "Ring", 1, [0.2, 0.5, 0.9, 1.0], 0.25, 1.0).with_channel(Channel::Height);
    ring.layers.push(stripes());
    doc.texture_sets.push(ring);
    doc
}

/// A built-in shape, shifted along x.
fn part(name: &str, material: usize, shape: u32, x: f32) -> MeshPart {
    let mut p = MeshPart::from_viewport(name, material, &build_mesh(shape, 96, 1.0, None, 0.0, 0.0));
    p.positions.iter_mut().for_each(|v| v[0] += x);
    p
}

fn uniform_set(id: &str, name: &str, slot: usize, base: [f32; 4], rough: f32, metal: f32) -> TextureSet {
    let mut set = TextureSet::new(id, name, (RESOLUTION, RESOLUTION))
        .with_channel(Channel::BaseColor)
        .with_channel(Channel::Roughness)
        .with_channel(Channel::Metallic)
        .with_channel(Channel::Normal);
    set.material_slot = slot;
    let values = vec![(Channel::BaseColor, base), (Channel::Roughness, [rough; 4]), (Channel::Metallic, [metal; 4])];
    let mut fill = Layer::new(format!("{id}_fill"), "Fill", LayerKind::Fill(Fill::uniform(values)));
    for c in [Channel::BaseColor, Channel::Roughness, Channel::Metallic] {
        fill = fill.with_blend(c, BlendMode::Normal, 1.0);
    }
    set.layers.push(fill);
    set
}

/// A paint layer with raised stripes in the height channel.
fn stripes() -> Layer {
    let n = RESOLUTION as usize;
    let tex = Tex::from_fn(n, n, |u, _| {
        let h = if (u * 16.0).fract() < 0.5 { 1.0 } else { 0.0 };
        [h, h, h, 1.0]
    });
    let format = ChannelSpec::new(Channel::Height).layer_format();
    let image = TiledImage::from_tex(&tex, format).expect("demo image");
    let content = PaintContent::from_images(vec![(Channel::Height, image)]);
    Layer::new("stripes", "Stripes", LayerKind::Paint(content)).with_blend(Channel::Height, BlendMode::Normal, 1.0)
}
