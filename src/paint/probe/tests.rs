use super::*;
use crate::camera::{Camera, Lens};
use crate::mesh::test_util::{quad, seamed_cube};
use crate::mesh::{Fit, Mesh};
use crate::paint::{BlendMode, Fill, Layer, LayerKind, PaintContent, PixelFormat, TiledImage};
use crate::projection::{ViewProjector, ViewScene};

const BASE: Channel = Channel::BaseColor;
const ROUGH: Channel = Channel::Roughness;

/// A paint layer whose base colour and roughness vary per texel, so a wrong texel shows.
fn varied_paint(w: u32, h: u32, seed: u32) -> Layer {
    let mut base = TiledImage::new(w, h, PixelFormat::from_id("rgba8").unwrap()).unwrap();
    let mut rough = TiledImage::new(w, h, PixelFormat::from_id("graya16").unwrap()).unwrap();
    for y in 0..h {
        for x in 0..w {
            let k = (x * 7 + y * 13 + seed) % 31;
            let a = if (x + y) % 5 == 0 { 0.0 } else { 0.4 + 0.6 * ((x * y) % 3) as f32 / 2.0 };
            base.set(x, y, [x as f32 / w as f32, y as f32 / h as f32, k as f32 / 31.0, a]);
            let r = ((x ^ y) % 17) as f32 / 16.0;
            rough.set(x, y, [r, r, r, 1.0 - a * 0.5]);
        }
    }
    let mut c = PaintContent::default();
    c.set_image(BASE, base);
    c.set_image(ROUGH, rough);
    Layer::new("paint", "Paint", LayerKind::Paint(c)).with_blend(BASE, BlendMode::Normal, 1.0).with_blend(ROUGH, BlendMode::Normal, 0.8)
}

/// A uniform fill multiplied over the stack through a striped mask.
fn masked_fill(w: u32, h: u32) -> Layer {
    let mut mask = TiledImage::new(w, h, PixelFormat::from_id("gray8").unwrap()).unwrap();
    for y in 0..h {
        for x in 0..w {
            let m = if x % 4 < 2 { 1.0 } else { 0.25 };
            mask.set(x, y, [m; 4]);
        }
    }
    Layer::new("fill", "Fill", LayerKind::Fill(Fill::uniform(vec![(BASE, [0.9, 0.5, 0.3, 1.0]), (ROUGH, [0.2, 0.2, 0.2, 1.0])])))
        .with_blend(BASE, BlendMode::Multiply, 0.7)
        .with_blend(ROUGH, BlendMode::Normal, 0.5)
        .with_mask(mask)
}

/// A quad on material slot 0 and a cube on slot 1, each its own texture set of another size.
fn document() -> Document {
    let mut cube = seamed_cube();
    cube.material = 1;
    cube.positions.iter_mut().for_each(|p| p[0] += 2.5);
    let mut doc = Document::new(Mesh { materials: vec!["a".into(), "b".into()], parts: vec![quad(), cube] });
    for (i, (w, h)) in [(16u32, 16u32), (24, 20)].into_iter().enumerate() {
        let mut set = TextureSet::new(format!("s{i}"), format!("S{i}"), (w, h)).with_channel(BASE).with_channel(ROUGH);
        set.material_slot = i;
        set.layers.push(varied_paint(w, h, i as u32 * 5));
        set.layers.push(masked_fill(w, h));
        doc.texture_sets.push(set);
    }
    doc
}

#[test]
fn texel_indices_follow_the_surface_map_convention() {
    assert_eq!(texel_index(0.0, 16), Some(0));
    assert_eq!(texel_index(0.999_999, 16), Some(15));
    assert_eq!(texel_index(1.0, 16), Some(0), "repeats");
    assert_eq!(texel_index(-0.01, 16), Some(15), "repeats below zero");
    assert_eq!(texel_index(0.5, 16), Some(8));
    assert_eq!(texel_index(f32::NAN, 16), None);
    assert_eq!(texel_index(0.5, 0), None);
}

#[test]
fn a_straight_on_pick_lands_on_the_texel_under_the_cursor() {
    let doc = document();
    let mesh = Mesh { materials: doc.mesh.materials.clone(), parts: vec![doc.mesh.parts[0].clone()] };
    let scene = ViewScene::new(&mesh, Fit::IDENTITY);
    let lens = Lens::Orthographic { height: 2.0, near: 0.1, far: 10.0 };
    let cam = Camera::looking_at([0.0, 0.0, 5.0], [0.0; 3], [0.0, 1.0, 0.0], &lens, [160.0, 160.0]);
    let view = scene.projector(&mesh, cam).unwrap();
    let probe = TexelProbe::new(&doc);
    for (sx, sy) in [(5.0f32, 5.0f32), (85.0, 15.0), (155.0, 150.0), (41.0, 121.0)] {
        let at = probe.locate(&view.pick([sx, sy]).unwrap()).unwrap();
        assert_eq!((at.set, at.x, at.y), (0, (sx / 10.0) as u32, (sy / 10.0) as u32), "screen {sx},{sy}");
    }
}

#[test]
fn stored_values_equal_the_composite_at_the_picked_texel() {
    let doc = document();
    let scene = ViewScene::new(&doc.mesh, doc.mesh.fit());
    let probe = TexelProbe::new(&doc);
    let composites: Vec<Vec<_>> = doc.texture_sets.iter().map(|s| [BASE, ROUGH].iter().map(|&c| Compositor::new(s).composite_tex(c).0).collect()).collect();
    let persp = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    let (mut hits, mut seen) = (0, std::collections::BTreeSet::new());
    for (yaw, pitch) in [(0.0f32, 0.0f32), (0.7, 0.4), (2.5, -0.6), (-0.6, 1.1)] {
        let eye = [3.2 * yaw.sin() * pitch.cos(), 3.2 * pitch.sin(), 3.2 * yaw.cos() * pitch.cos()];
        let cam = Camera::looking_at(eye, [0.0; 3], [0.0, 1.0, 0.0], &persp, [200.0, 150.0]);
        let view = scene.projector(&doc.mesh, cam).unwrap();
        for y in (0..150).step_by(3) {
            for x in (0..200).step_by(3) {
                let Some(hit) = view.pick([x as f32 + 0.5, y as f32 + 0.5]) else { continue };
                let at = probe.locate(&hit).unwrap();
                assert_eq!(at.set, hit.part, "each part has its own set");
                let all = probe.channels(at);
                assert_eq!(all.iter().map(|v| v.channel).collect::<Vec<_>>(), vec![BASE, ROUGH]);
                for (ci, &channel) in [BASE, ROUGH].iter().enumerate() {
                    let tex = &composites[at.set][ci];
                    let want = tex.px[at.y as usize * tex.w + at.x as usize];
                    let got = probe.stored(at, channel).unwrap();
                    assert_eq!(got.value, want, "set {} texel {},{} {channel:?}", at.set, at.x, at.y);
                    assert_eq!(all[ci], got);
                }
                hits += 1;
                seen.insert((at.set, at.x, at.y));
            }
        }
    }
    assert!(hits > 1000 && seen.len() > 300, "{hits} picks over {} texels", seen.len());
    assert_eq!(probe.stored(TexelAt { set: 0, x: 0, y: 0 }, BASE).unwrap().color_space, ColorSpace::Srgb);
    assert_eq!(probe.stored(TexelAt { set: 0, x: 0, y: 0 }, ROUGH).unwrap().color_space, ColorSpace::Linear);
}

#[test]
fn undeclared_channels_unknown_sets_and_outside_texels_give_nothing() {
    let doc = document();
    let probe = TexelProbe::new(&doc);
    assert!(probe.stored(TexelAt { set: 0, x: 1, y: 1 }, Channel::Metallic).is_none());
    assert!(probe.stored(TexelAt { set: 7, x: 1, y: 1 }, BASE).is_none());
    assert!(probe.stored(TexelAt { set: 0, x: 16, y: 1 }, BASE).is_none());
    assert!(probe.channels(TexelAt { set: 0, x: 1, y: 99 }).is_empty());
    assert!(probe.texel_of(3, [0.5, 0.5]).is_none());
}
