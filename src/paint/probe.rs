//! [`TexelProbe`]: what a document's layer stack holds at one point of the mesh, channel by channel.
//!
//! The one lookup the colour eyedropper's stored-value pick (CP-06 with Shift) and the material picker (PT-18) share
//! (ADR 0008 mc2-q-pt-08c): a surface hit from the view projector names a mesh part and a UV; the part's material slot
//! names the texture set, the UV the texel, and [`Compositor::composite_rect`] composites that single texel. The
//! result is the channel's stored value (in the channel's stored encoding) with its coverage, exactly what the full
//! composite holds at that texel.

use super::channel::{Channel, ColorSpace};
use super::doc::Document;
use super::image::Rect;
use super::stack::{Compositor, FillContext, NO_FILL_CONTEXT};
use super::texture_set::TextureSet;
use crate::projection::SurfaceHit;
use std::borrow::Cow;

/// One texel of one texture set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TexelAt {
    /// Index into [`Document::texture_sets`].
    pub set: usize,
    pub x: u32,
    pub y: u32,
}

/// A channel's composited value at one texel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoredValue {
    pub channel: Channel,
    /// RGB in the channel's stored encoding (see `color_space`), alpha the accumulated coverage.
    pub value: [f32; 4],
    /// How `value`'s RGB is encoded.
    pub color_space: ColorSpace,
}

/// Looks up composited channel values under points of a document's mesh. Cheap to create; borrows the document.
#[derive(Debug, Clone, Copy)]
pub struct TexelProbe<'a> {
    doc: &'a Document,
    fills: &'a FillContext<'a>,
}

impl<'a> TexelProbe<'a> {
    /// A probe without a fill context (uniform and texture-space/UV image fills only).
    pub fn new(doc: &'a Document) -> TexelProbe<'a> {
        TexelProbe { doc, fills: &NO_FILL_CONTEXT }
    }

    /// Builder: evaluates fills with `fills`, as the composite the viewport shows does.
    pub fn with_fills(mut self, fills: &'a FillContext<'a>) -> TexelProbe<'a> {
        self.fills = fills;
        self
    }

    /// The texel under a surface hit: the texture set of the hit part's material slot, the texel holding the hit's
    /// UV. `None` when no texture set paints that part or the UV is not finite.
    pub fn locate(&self, hit: &SurfaceHit) -> Option<TexelAt> {
        let material = self.doc.mesh.parts.get(hit.part)?.material;
        let set = self.doc.texture_sets.iter().position(|s| s.material_slot == material)?;
        self.texel_of(set, hit.uv)
    }

    /// The texel of texture set `set` holding `uv` (UV origin top-left; UVs outside 0…1 repeat, as the viewport
    /// samples them). `None` for an unknown set, an empty set or a UV that is not finite.
    pub fn texel_of(&self, set: usize, uv: [f32; 2]) -> Option<TexelAt> {
        let (w, h) = self.doc.texture_sets.get(set)?.resolution;
        Some(TexelAt { set, x: texel_index(uv[0], w)?, y: texel_index(uv[1], h)? })
    }

    /// `channel`'s composited value at `at`; `None` when the texture set does not declare the channel or the texel
    /// lies outside it.
    pub fn stored(&self, at: TexelAt, channel: Channel) -> Option<StoredValue> {
        let set = self.set_at(at)?;
        set.has_channel(channel).then(|| self.composite(&set, at, channel))
    }

    /// Every declared channel's composited value at `at`, in the set's channel order (the material picker).
    pub fn channels(&self, at: TexelAt) -> Vec<StoredValue> {
        let Some(set) = self.set_at(at) else { return Vec::new() };
        set.channels.iter().map(|spec| self.composite(&set, at, spec.channel)).collect()
    }

    /// The set holding `at`, with its layer instances resolved as for compositing.
    fn set_at(&self, at: TexelAt) -> Option<Cow<'a, TextureSet>> {
        let set = self.doc.resolved_set(at.set)?;
        (at.x < set.resolution.0 && at.y < set.resolution.1).then_some(set)
    }

    fn composite(&self, set: &TextureSet, at: TexelAt, channel: Channel) -> StoredValue {
        let compositor = Compositor::new(set).with_fills(self.fills);
        let mut out = [[0.0f32; 4]; 1];
        compositor.composite_rect(channel, Rect { x: at.x, y: at.y, w: 1, h: 1 }, &mut out);
        let color_space = set.channel_spec(channel).map_or(channel.default_color_space(), |s| s.color_space);
        StoredValue { channel, value: out[0], color_space }
    }
}

/// The texel index along one axis of `size` texels holding coordinate `t` (repeating outside 0…1).
fn texel_index(t: f32, size: u32) -> Option<u32> {
    if !t.is_finite() || size == 0 {
        return None;
    }
    let f = t - t.floor();
    // `f` is in [0, 1); rounding can still land on `size` for f just below 1.
    Some(((f * size as f32) as u32).min(size - 1))
}

#[cfg(test)]
mod tests;
