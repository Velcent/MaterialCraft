//! [`PreviewGrid`]: layer thumbnails composited on a reduced grid (LS-08). Every image of a layer and the
//! set's baked maps are point-sampled down to the grid, then the layer is composited there on its own, so a
//! thumbnail costs a few thousand texels whatever the texture resolution. Meant for a background thread.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use super::channel::Channel;
use super::effect::{EffectKind, EffectRegistry};
use super::image::{Rect, TILE_SIZE, TiledImage};
use super::layer::{Layer, LayerKind};
use super::stack::Compositor;
use super::texture_set::{BakedMap, TextureSet};
use crate::tex::Tex;

/// A texture set's shell (channels, baked maps) at thumbnail size.
#[derive(Debug, Clone)]
pub struct PreviewGrid {
    shell: TextureSet,
    from: (u32, u32),
}

impl PreviewGrid {
    /// A grid whose longer edge is `max_edge` texels (at least 1), with the aspect of `set`.
    pub fn new(set: &TextureSet, max_edge: u32) -> PreviewGrid {
        let (w, h) = set.resolution;
        let max_edge = max_edge.max(1);
        let size = if w >= h {
            (max_edge.min(w.max(1)), ((u64::from(max_edge.min(w.max(1))) * u64::from(h) / u64::from(w.max(1))) as u32).max(1))
        } else {
            (((u64::from(max_edge.min(h)) * u64::from(w) / u64::from(h)) as u32).max(1), max_edge.min(h))
        };
        let mut shell = TextureSet::new(set.id.clone(), set.name.clone(), size);
        shell.channels = set.channels.clone();
        shell.material_slot = set.material_slot;
        shell.uv_set = set.uv_set;
        shell.baked = set.baked.iter().map(|b| BakedMap { kind: b.kind.clone(), image: sample(&b.image, size) }).collect();
        PreviewGrid { shell, from: set.resolution }
    }

    /// Width and height of the grid.
    pub fn size(&self) -> (u32, u32) {
        self.shell.resolution
    }

    /// `layer` with every image (paint, effect and mask images, children's too) sampled down to the grid.
    pub fn reduce(&self, layer: &Layer) -> Layer {
        let size = self.size();
        let mut out = layer.clone();
        if let LayerKind::Paint(p) = &mut out.kind {
            p.images.iter_mut().for_each(|(_, i)| *i = sample(i, size));
        }
        for e in &mut out.effects {
            if let EffectKind::Paint(p) = &mut e.kind {
                p.images.iter_mut().for_each(|(_, i)| *i = sample(i, size));
            }
        }
        if let Some(m) = &mut out.mask {
            for e in &mut m.effects {
                if let EffectKind::Paint(p) = &mut e.kind {
                    p.image = sample(&p.image, size);
                }
            }
        }
        if let LayerKind::Group(children) = &mut out.kind {
            for c in children.iter_mut() {
                *c = self.reduce(c);
            }
        }
        out
    }

    /// The content of `layer` (already [`Self::reduce`]d) for `channel`: stored encoding, alpha = coverage.
    pub fn content(&self, layer: &Layer, channel: Channel, effects: &EffectRegistry) -> Tex {
        let (w, h) = self.size();
        let mut px = vec![[0.0f32; 4]; w as usize * h as usize];
        Compositor::with_effects(&self.shell, effects).layer_content_rect(layer, channel, Rect { x: 0, y: 0, w, h }, &mut px);
        Tex { w: w as usize, h: h as usize, px }
    }

    /// The grey values of `layer`'s mask (already reduced) as an opaque grey texture, or `None` without a mask.
    pub fn mask(&self, layer: &Layer, effects: &EffectRegistry) -> Option<Tex> {
        let (w, h) = self.size();
        let mut grey = vec![0.0f32; w as usize * h as usize];
        Compositor::with_effects(&self.shell, effects).mask_rect(layer, Rect { x: 0, y: 0, w, h }, &mut grey)?;
        Some(Tex { w: w as usize, h: h as usize, px: grey.into_iter().map(|g| [g, g, g, 1.0]).collect() })
    }

    /// The resolution the grid was made from.
    pub fn source_size(&self) -> (u32, u32) {
        self.from
    }
}

/// `image` point-sampled to `size`; unallocated source tiles stay unallocated.
fn sample(image: &TiledImage, size: (u32, u32)) -> TiledImage {
    let Ok(mut out) = TiledImage::new(size.0, size.1, image.format()) else {
        return image.clone();
    };
    let (sw, sh) = image.size();
    for y in 0..size.1 {
        let sy = ((u64::from(y) * 2 + 1) * u64::from(sh) / (u64::from(size.1) * 2)) as u32;
        for x in 0..size.0 {
            let sx = ((u64::from(x) * 2 + 1) * u64::from(sw) / (u64::from(size.0) * 2)) as u32;
            if image.tile_allocated(sx / TILE_SIZE, sy / TILE_SIZE) {
                out.set(x, y, image.get(sx, sy));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::blend::BlendMode;
    use crate::paint::effect::{GreyBlend, MaskEffect, MaskPixels};
    use crate::paint::layer::{Fill, PaintContent};
    use crate::paint::mask::Mask;
    use crate::paint::{ChannelSpec, PixelFormat};

    #[test]
    fn thumbnails_show_content_and_mask_at_grid_size() {
        let set = TextureSet::new("s", "S", (512, 256)).with_channel(Channel::Roughness);
        let grid = PreviewGrid::new(&set, 64);
        assert_eq!(grid.size(), (64, 32));
        let format = ChannelSpec::new(Channel::Roughness).layer_format();
        let mut image = TiledImage::new(512, 256, format).unwrap();
        for y in 0..256 {
            for x in 0..256 {
                image.set(x, y, [0.8, 0.8, 0.8, 1.0]);
            }
        }
        let mut mask_image = TiledImage::new(512, 256, PixelFormat::from_id("graya8").unwrap()).unwrap();
        for y in 128..256 {
            for x in 0..512 {
                mask_image.set(x, y, [1.0; 4]);
            }
        }
        let mut l = Layer::new("p", "P", LayerKind::Paint(PaintContent::from_images(vec![(Channel::Roughness, image)]))).with_blend(
            Channel::Roughness,
            BlendMode::Multiply,
            0.3,
        );
        l.mask = Some(Mask::new(0.0).with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::Paint(MaskPixels::new(mask_image)))));
        let reduced = grid.reduce(&l);
        let none = EffectRegistry::new();
        let c = grid.content(&reduced, Channel::Roughness, &none);
        let at = |t: &Tex, x: usize, y: usize| t.px[y * t.w + x];
        assert!((at(&c, 10, 10)[0] - 0.8).abs() < 0.01 && at(&c, 10, 10)[3] == 1.0, "content ignores blend and opacity");
        assert_eq!(at(&c, 50, 10)[3], 0.0, "unpainted half is empty");
        let m = grid.mask(&reduced, &none).unwrap();
        assert_eq!((at(&m, 5, 5)[0], at(&m, 5, 25)[0]), (0.0, 1.0));
        let fill = Layer::new("f", "F", LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [0.25; 4])])));
        assert_eq!(at(&grid.content(&fill, Channel::Roughness, &none), 0, 0), [0.25, 0.25, 0.25, 1.0]);
        assert!(grid.mask(&fill, &none).is_none());
        let group = Layer::new("g", "G", LayerKind::Group(vec![reduced.clone()]));
        assert!(at(&grid.content(&group, Channel::Roughness, &none), 10, 30)[3] > 0.0, "a group shows its children, masked");
        assert_eq!(PreviewGrid::new(&TextureSet::new("t", "T", (16, 4096)), 64).size(), (1, 64));
    }
}
