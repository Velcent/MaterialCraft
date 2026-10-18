//! [`Compositor`]: flattens a texture set's layer stack into one channel, tile by tile.
//!
//! Rules, bottom to top, starting from the channel default with coverage 0:
//! - hidden layers, layers without a blend entry for the channel and `Disable` entries are skipped;
//! - a layer's strength is `layer opacity × entry opacity × mask × geometry mask`; the mask is its initial value
//!   with its effects on top ([`crate::paint::mask`]), a disabled mask counts as 1; the geometry mask is 1 on the
//!   mesh parts it includes and 0 elsewhere, from the [`FillContext`]'s surface map (without one it counts as 1 and
//!   is reported);
//! - a layer's content is its base (paint image with alpha = coverage, uniform fill with coverage 1, image
//!   fill through its projection; other sources through the [`FillContext`], else skipped and reported) with its content effects on top, each blended by its own entry for the
//!   channel; the content is blended over the stack with the layer's mode and strength;
//! - effect layers (a layer that is not a folder, blending the channel with `Passthrough`): the content starts
//!   from the composite below instead of from nothing, and the result replaces the stack by the layer's strength;
//! - groups: `Passthrough` blends the children straight into the stack, then fades the result
//!   against the stack below by the group's strength; any other mode composites the children in
//!   isolation (from the default, coverage 0), applies the group's effects, and blends that over the stack
//!   with the group's mode;
//! - generators and filters are evaluated by the implementations in the [`EffectRegistry`]; effects without
//!   one, and kinds that are not computable yet, are left out and reported by [`Compositor::warnings`];
//! - levels, colour selection, polygon fill and references are native (see `native`); polygon fill needs the
//!   [`FillContext`]'s surface map, colour selection the set's baked map, a reference its anchor;
//! - fills are laid on by their projection ([`crate::projection`]); sources other than uniform values and
//!   images, and projections that need the mesh, are evaluated through the [`FillContext`].
//!
//! Layers blend in the channel's working space (see [`super::working_space`]): sRGB channels in linear
//! light. The output's RGB is the channel value in its stored encoding, alpha the accumulated coverage.

use super::blend::BlendMode;
use super::channel::Channel;
use super::effect::EffectRegistry;
use super::effect::registry::EMPTY_REGISTRY;
use super::image::{ImageError, Rect, TILE_SIZE, TiledImage};
use super::layer::{Layer, LayerKind};
use super::par::for_each_mut;
use super::texture_set::TextureSet;
use super::validate::MAX_GROUP_DEPTH;
use super::working_space::{SpaceConverter, WorkingSpace};
use pass::{Pass, Scratch, affects};

mod below;
mod content;
mod fill_context;
mod fill_eval;
mod geometry;
mod mask_eval;
mod native;
mod pass;
mod preview;
mod procedural;
mod warnings;
use crate::tex::Tex;
pub use fill_context::{ChainedSources, FillContext, FillRequest, FillSources, NO_FILL_CONTEXT, NoFillSources};
pub use warnings::CompositeWarning;

/// Composites the layer stack of one texture set. Cheap to create; borrows the set.
#[derive(Debug, Clone, Copy)]
pub struct Compositor<'a> {
    set: &'a TextureSet,
    effects: &'a EffectRegistry,
    fills: &'a FillContext<'a>,
}

impl<'a> Compositor<'a> {
    /// A compositor without generator or filter implementations (they are all not computable) and without a
    /// fill context (uniform and texture-space/UV image fills only).
    pub fn new(set: &'a TextureSet) -> Compositor<'a> {
        Compositor { set, effects: &EMPTY_REGISTRY, fills: &NO_FILL_CONTEXT }
    }

    /// A compositor that evaluates generators and filters through `effects`.
    pub fn with_effects(set: &'a TextureSet, effects: &'a EffectRegistry) -> Compositor<'a> {
        Compositor { set, effects, fills: &NO_FILL_CONTEXT }
    }

    /// Builder: evaluates fills with `fills` (surface map, scene frame, source evaluators).
    pub fn with_fills(mut self, fills: &'a FillContext<'a>) -> Compositor<'a> {
        self.fills = fills;
        self
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
        let Some(weights) = self.layer_weights(l, strength, r) else {
            return;
        };
        let mode = blend.mode;
        let group = matches!(l.kind, LayerKind::Group(_));
        if content::is_effect_layer(l, channel) {
            let content = self.content_over(l, l.effects.len(), pass, depth, acc.to_vec());
            for (i, a) in acc.iter_mut().enumerate() {
                let w = weights.at(i);
                *a = [0, 1, 2, 3].map(|k| a[k] + (content[i][k] - a[k]) * w);
            }
            return;
        }
        if (!group || mode != BlendMode::Passthrough) && self.has_active_effects(l, pass) {
            if group && depth >= MAX_GROUP_DEPTH {
                return;
            }
            let content = self.content(l, l.effects.len(), pass, depth);
            for (i, a) in acc.iter_mut().enumerate() {
                *a = mode.mix(*a, content[i], weights.at(i));
            }
            return;
        }
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
                let Some(top) = self.fill_top(f, pass) else {
                    return;
                };
                for (i, a) in acc.iter_mut().enumerate() {
                    *a = mode.mix(*a, top.at(i), weights.at(i));
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
#[cfg(test)]
mod tests_effects;
#[cfg(test)]
mod tests_fills;
#[cfg(test)]
mod tests_geometry;
#[cfg(test)]
mod tests_native;
