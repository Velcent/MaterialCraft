//! The material picker (PT-18): a temporary tool that reads every channel's stored value under the cursor into the
//! brush's painted values, then hands back to the tool before it. It shares the colour eyedropper's stored-value
//! lookup (ADR 0008 mc2-q-pt-08c): the view projector's hit → [`TexelProbe`] → the composite at that texel.

use super::BrushTools;
use materialcraft::paint::probe::{StoredValue, TexelProbe};
use materialcraft::projection::ViewProjector;

/// Every channel's composited value at the texel under view position `at`.
pub fn pick(probe: &TexelProbe, view: &dyn ViewProjector, at: [f32; 2]) -> Result<Vec<StoredValue>, &'static str> {
    let hit = view.pick(at).ok_or("There is no surface under the material picker.")?;
    let texel = probe.locate(&hit).ok_or("No texture set paints the surface under the material picker.")?;
    Ok(probe.channels(texel))
}

impl BrushTools {
    /// Takes picked values as the values painted from now on: each picked channel is switched on and paints the
    /// picked colour (fully: the coverage under the cursor is not the material's).
    pub fn take_material(&mut self, picked: &[StoredValue]) {
        for v in picked {
            let entry = self.value_mut(v.channel);
            entry.value = [v.value[0], v.value[1], v.value[2], 1.0];
            entry.enabled = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::camera::{Camera, Lens};
    use materialcraft::mesh::{Fit, Mesh, MeshPart};
    use materialcraft::paint::image::Rect;
    use materialcraft::paint::stack::Compositor;
    use materialcraft::paint::{BlendMode, Channel, Document, Fill, Layer, LayerKind, PaintContent, PixelFormat, TextureSet, TiledImage};
    use materialcraft::projection::ViewScene;

    const SIZE: u32 = 32;

    /// A plane with a paint layer of per-texel colours and roughness, and a half-strength fill over it.
    fn document() -> Document {
        let plane = materialcraft::render3d::build_mesh(2, 8, 1.0, None, 0.0, 0.0);
        let mut doc = Document::new(Mesh { materials: vec!["m".into()], parts: vec![MeshPart::from_viewport("plane", 0, &plane)] });
        let mut set = TextureSet::new("s", "S", (SIZE, SIZE)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
        let mut base = TiledImage::new(SIZE, SIZE, PixelFormat::from_id("rgba8").unwrap()).unwrap();
        let mut rough = TiledImage::new(SIZE, SIZE, PixelFormat::from_id("gray16").unwrap()).unwrap();
        for y in 0..SIZE {
            for x in 0..SIZE {
                base.set(x, y, [x as f32 / SIZE as f32, y as f32 / SIZE as f32, ((x * 7 + y) % 13) as f32 / 12.0, 1.0]);
                let r = ((x ^ y) % 17) as f32 / 16.0;
                rough.set(x, y, [r, r, r, 1.0]);
            }
        }
        let mut c = PaintContent::default();
        c.set_image(Channel::BaseColor, base);
        c.set_image(Channel::Roughness, rough);
        set.layers.push(Layer::new("paint", "Paint", LayerKind::Paint(c)).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
        set.layers.push(
            Layer::new("fill", "Fill", LayerKind::Fill(Fill::uniform(vec![(Channel::BaseColor, [0.9, 0.5, 0.3, 1.0]), (Channel::Roughness, [0.2; 4])])))
                .with_blend(Channel::BaseColor, BlendMode::Multiply, 0.5)
                .with_blend(Channel::Roughness, BlendMode::Normal, 0.5),
        );
        doc.texture_sets.push(set);
        doc
    }

    /// The gate: what the picker takes equals the full composite of the set at the texel under the cursor.
    #[test]
    fn the_picker_takes_the_composite_at_the_texel_under_the_cursor() {
        let doc = document();
        let scene = ViewScene::new(&doc.mesh, Fit::IDENTITY);
        let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.05, far: 50.0 };
        let cam = Camera::looking_at([0.3, 2.5, 0.4], [0.0; 3], [0.0, 0.0, -1.0], &lens, [300.0, 200.0]);
        let view = scene.projector(&doc.mesh, cam).unwrap();
        let probe = TexelProbe::new(&doc);
        let set = &doc.texture_sets[0];
        let composite = |c: Channel| {
            let mut px = vec![[0.0f32; 4]; (SIZE * SIZE) as usize];
            Compositor::new(set).composite_rect(c, Rect { x: 0, y: 0, w: SIZE, h: SIZE }, &mut px);
            px
        };
        let (base, rough) = (composite(Channel::BaseColor), composite(Channel::Roughness));
        let mut checked = 0;
        for at in [[150.0, 100.0], [140.0, 95.0], [160.0, 106.0], [145.0, 108.0]] {
            let Some(hit) = view.pick(at) else { continue };
            let (x, y) = (((hit.uv[0] * SIZE as f32) as u32).min(SIZE - 1), ((hit.uv[1] * SIZE as f32) as u32).min(SIZE - 1));
            let picked = pick(&probe, &view, at).unwrap();
            let i = (y * SIZE + x) as usize;
            assert_eq!(picked.iter().map(|v| v.channel).collect::<Vec<_>>(), vec![Channel::BaseColor, Channel::Roughness]);
            assert_eq!(picked[0].value, base[i], "base colour at {at:?}");
            assert_eq!(picked[1].value, rough[i], "roughness at {at:?}");
            let mut brush = BrushTools::default();
            brush.value_mut(Channel::Roughness).enabled = false;
            brush.take_material(&picked);
            let values = brush.values_for(set);
            assert_eq!(values.value(Channel::BaseColor), Some([base[i][0], base[i][1], base[i][2], 1.0]));
            assert_eq!(values.value(Channel::Roughness), Some([rough[i][0], rough[i][1], rough[i][2], 1.0]), "switched on again");
            checked += 1;
        }
        assert_eq!(checked, 4, "every pick hit the plane");
        assert!(pick(&probe, &view, [1.0, 1.0]).is_err(), "off the mesh");
    }
}
