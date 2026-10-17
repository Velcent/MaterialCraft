//! [`Compositor`]: flattens a texture set's layer stack into one channel, tile by tile.
//!
//! Rules, bottom to top, starting from the channel default with coverage 0:
//! - hidden layers, layers without a blend entry for the channel and `Disable` entries are skipped;
//! - a layer's strength is `layer opacity × entry opacity × mask` (mask = red × alpha of the mask image);
//! - paint layers blend their channel image (alpha = coverage), uniform fills blend their value with
//!   coverage 1, material fills are skipped and reported by [`Compositor::warnings`];
//! - groups: `Passthrough` blends the children straight into the stack, then fades the result
//!   against the stack below by the group's strength; any other mode composites the children in
//!   isolation (from the default, coverage 0) and blends that over the stack with the group's mode.
//!
//! Layers blend in the channel's working space (see [`super::working_space`]): sRGB channels in linear
//! light. The output's RGB is the channel value in its stored encoding, alpha the accumulated coverage.

use super::blend::BlendMode;
use super::channel::Channel;
use super::image::{ImageError, Rect, TILE_SIZE, TiledImage};
use super::layer::{FillSource, Layer, LayerKind};
use super::par::for_each_mut;
use super::texture_set::TextureSet;
use super::validate::MAX_GROUP_DEPTH;
use super::working_space::{SpaceConverter, WorkingSpace};
use pass::{Pass, Scratch, affects, layer_weights};

mod pass;
use crate::tex::Tex;

/// Something the compositor could not do exactly. Layers are named by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositeWarning {
    /// A material fill cannot be evaluated by the core; the layer was left out.
    MaterialFillSkipped { layer: String },
    /// A group nested deeper than [`MAX_GROUP_DEPTH`]; it was left out.
    GroupTooDeep { layer: String },
    /// A paint image or mask does not match the texture set resolution; it was read as is (cropped / zero-padded).
    ImageSizeMismatch { layer: String },
}

/// Composites the layer stack of one texture set. Cheap to create; borrows the set.
#[derive(Debug, Clone, Copy)]
pub struct Compositor<'a> {
    set: &'a TextureSet,
}

impl<'a> Compositor<'a> {
    pub fn new(set: &'a TextureSet) -> Compositor<'a> {
        Compositor { set }
    }

    /// Composites `channel` over the pixel rectangle `r` into `out` (`r.area()` pixels, row by row).
    /// This is the per-tile building block; it does not allocate per pixel.
    pub fn composite_rect(&self, channel: Channel, r: Rect, out: &mut [[f32; 4]]) {
        let pass = Pass { channel, r, space: SpaceConverter::new(self.working_space(channel)) };
        out.fill(pass.empty());
        let mut scratch = Scratch::default();
        self.apply_layers(&self.set.layers, &pass, out, 0, &mut scratch);
        pass.space.to_stored_all(out);
    }

    /// The space `channel` blends in, from its declared colour space (or the channel's default).
    pub fn working_space(&self, channel: Channel) -> WorkingSpace {
        let space = self.set.channel_spec(channel).map_or(channel.default_color_space(), |s| s.color_space);
        WorkingSpace::of(space)
    }

    /// Recomputes the listed tiles of `target` (sized like the texture set) in parallel.
    pub fn update(&self, channel: Channel, target: &mut TiledImage, tiles: &[(u32, u32)]) -> Vec<CompositeWarning> {
        target.update_tiles(tiles, |r, px| self.composite_rect(channel, r, px));
        self.warnings(channel)
    }

    /// The whole channel as a tiled image in the channel's format plus alpha (coverage).
    pub fn composite_image(&self, channel: Channel) -> Result<(TiledImage, Vec<CompositeWarning>), ImageError> {
        let format = self.set.channel_spec(channel).map_or(channel.default_format(), |s| s.format).with_alpha();
        let (w, h) = self.set.resolution;
        let mut img = TiledImage::new(w, h, format)?;
        let all = img.all_tiles();
        let warnings = self.update(channel, &mut img, &all);
        Ok((img, warnings))
    }

    /// The whole channel as a float texture (alpha = coverage).
    pub fn composite_tex(&self, channel: Channel) -> (Tex, Vec<CompositeWarning>) {
        let (w, h) = (self.set.resolution.0 as usize, self.set.resolution.1 as usize);
        let t = TILE_SIZE as usize;
        let mut px = vec![[0.0f32; 4]; w * h];
        if w > 0 {
            let mut bands: Vec<&mut [[f32; 4]]> = px.chunks_mut(t * w).collect();
            for_each_mut(&mut bands, |band_index, band| {
                let rows = band.len() / w;
                let mut tile = vec![[0.0f32; 4]; t * rows];
                for x in (0..w).step_by(t) {
                    let r = Rect { x: x as u32, y: (band_index * t) as u32, w: (w - x).min(t) as u32, h: rows as u32 };
                    self.composite_rect(channel, r, &mut tile[..r.area()]);
                    for (row, line) in tile[..r.area()].chunks_exact(r.w as usize).enumerate() {
                        band[row * w + x..row * w + x + line.len()].copy_from_slice(line);
                    }
                }
            });
        }
        (Tex { w, h, px }, self.warnings(channel))
    }

    /// What compositing `channel` cannot do exactly, in stack order.
    pub fn warnings(&self, channel: Channel) -> Vec<CompositeWarning> {
        let mut out = Vec::new();
        self.collect_warnings(&self.set.layers, channel, 0, &mut out);
        out
    }

    fn collect_warnings(&self, layers: &[Layer], channel: Channel, depth: usize, out: &mut Vec<CompositeWarning>) {
        for l in layers.iter().filter(|l| affects(l, channel)) {
            let id = || l.id.clone();
            if l.images().iter().any(|(c, i)| c.is_none_or(|c| c == channel) && i.size() != self.set.resolution) {
                out.push(CompositeWarning::ImageSizeMismatch { layer: id() });
            }
            match &l.kind {
                LayerKind::Fill(f) if matches!(f.source, FillSource::Material { .. }) => out.push(CompositeWarning::MaterialFillSkipped { layer: id() }),
                LayerKind::Group(_) if depth >= MAX_GROUP_DEPTH => out.push(CompositeWarning::GroupTooDeep { layer: id() }),
                LayerKind::Group(children) => self.collect_warnings(children, channel, depth + 1, out),
                _ => {}
            }
        }
    }

    fn apply_layers(&self, layers: &[Layer], pass: &Pass, acc: &mut [[f32; 4]], depth: usize, s: &mut Scratch) {
        for l in layers {
            self.apply_layer(l, pass, acc, depth, s);
        }
    }

    fn apply_layer(&self, l: &Layer, pass: &Pass, acc: &mut [[f32; 4]], depth: usize, s: &mut Scratch) {
        let (channel, r) = (pass.channel, pass.r);
        let Some(blend) = l.blend_for(channel).filter(|_| affects(l, channel)) else {
            return;
        };
        let strength = l.opacity * blend.opacity;
        let Some(weights) = layer_weights(l, strength, r) else {
            return;
        };
        let mode = blend.mode;
        match &l.kind {
            LayerKind::Paint(p) => {
                let Some(img) = p.image(channel) else { return };
                if img.format().layout.has_alpha() && !img.region_allocated(r) {
                    return; // nothing painted here
                }
                s.top.resize(r.area(), [0.0; 4]);
                img.read_region(r, &mut s.top);
                pass.space.to_working_all(&mut s.top);
                for (i, a) in acc.iter_mut().enumerate() {
                    *a = mode.mix(*a, s.top[i], weights.at(i));
                }
            }
            LayerKind::Fill(f) => {
                let Some(v) = f.uniform_value(channel) else {
                    return;
                };
                let top = pass.space.to_working([v[0], v[1], v[2], 1.0]);
                for (i, a) in acc.iter_mut().enumerate() {
                    *a = mode.mix(*a, top, weights.at(i));
                }
            }
            LayerKind::Group(children) => {
                if depth >= MAX_GROUP_DEPTH {
                    return;
                }
                let mut inner = Scratch::default();
                if mode == BlendMode::Passthrough {
                    let below = acc.to_vec();
                    self.apply_layers(children, pass, acc, depth + 1, &mut inner);
                    for (i, a) in acc.iter_mut().enumerate() {
                        let w = weights.at(i);
                        *a = [0, 1, 2, 3].map(|c| below[i][c] + (a[c] - below[i][c]) * w);
                    }
                } else {
                    let mut group = vec![pass.empty(); acc.len()];
                    self.apply_layers(children, pass, &mut group, depth + 1, &mut inner);
                    for (i, a) in acc.iter_mut().enumerate() {
                        *a = mode.mix(*a, group[i], weights.at(i));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
