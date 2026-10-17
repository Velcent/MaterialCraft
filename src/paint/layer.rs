//! Layers of a texture set's stack: paint, fill and group layers, their per-channel blending, their content
//! effects and masks.
//!
//! A layer's content is its base (the [`LayerKind`]: painted pixels, a fill, or for a group its children)
//! followed by [`Layer::effects`], an ordered stack applied on top of the base inside the layer (see
//! [`super::effect`]). The result is blended over the stack below by the layer's own entries, scaled by its
//! [`Mask`].

use super::blend::BlendMode;
use super::channel::Channel;
use super::effect::{ContentEffect, EffectKind};
use super::geometry_mask::GeometryMask;
use super::image::TiledImage;
use super::mask::Mask;

/// How a layer (or a content effect) affects one channel. Without an entry for a channel it does not affect it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelBlend {
    pub channel: Channel,
    pub mode: BlendMode,
    /// 0..1, multiplied with the layer opacity.
    pub opacity: f32,
}

/// Painted pixels, one image per channel. Images are straight (not premultiplied); alpha is coverage.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaintContent {
    pub images: Vec<(Channel, TiledImage)>,
}

impl PaintContent {
    pub fn image(&self, channel: Channel) -> Option<&TiledImage> {
        self.images.iter().find(|(c, _)| *c == channel).map(|(_, i)| i)
    }

    pub fn image_mut(&mut self, channel: Channel) -> Option<&mut TiledImage> {
        self.images.iter_mut().find(|(c, _)| *c == channel).map(|(_, i)| i)
    }

    /// Adds or replaces the image of `channel`.
    pub fn set_image(&mut self, channel: Channel, image: TiledImage) {
        match self.image_mut(channel) {
            Some(slot) => *slot = image,
            None => self.images.push((channel, image)),
        }
    }
}

/// What a fill layer fills with.
#[derive(Debug, Clone, PartialEq)]
pub enum FillSource {
    /// A constant value per channel (normalised RGBA; alpha is ignored, coverage is 1).
    Uniform(Vec<(Channel, [f32; 4])>),
    /// A procedural material, evaluated outside the core document (path inside the document, e.g.
    /// `materials/rust.mca`, plus parameter overrides as strings).
    Material { path: String, params: Vec<(String, String)> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub source: FillSource,
}

impl Fill {
    /// A uniform fill.
    pub fn uniform(values: Vec<(Channel, [f32; 4])>) -> Fill {
        Fill { source: FillSource::Uniform(values) }
    }

    /// The channels a uniform fill sets (none for a material).
    pub fn channels(&self) -> Vec<Channel> {
        match &self.source {
            FillSource::Uniform(v) => v.iter().map(|(c, _)| *c).collect(),
            FillSource::Material { .. } => Vec::new(),
        }
    }

    /// The uniform value for `channel`, if this is a uniform fill that sets it.
    pub fn uniform_value(&self, channel: Channel) -> Option<[f32; 4]> {
        match &self.source {
            FillSource::Uniform(v) => v.iter().find(|(c, _)| *c == channel).map(|(_, v)| *v),
            FillSource::Material { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayerKind {
    Paint(PaintContent),
    Fill(Fill),
    /// Child layers, bottom to top. The group's blend mode `Passthrough` blends children straight into
    /// the stack; any other mode composites them in isolation first.
    Group(Vec<Layer>),
}

/// One layer of a stack.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// Unique within its texture set.
    pub id: String,
    pub name: String,
    pub visible: bool,
    /// 0..1 for all channels.
    pub opacity: f32,
    pub blend: Vec<ChannelBlend>,
    pub mask: Option<Mask>,
    /// Limits the layer to parts of the geometry (data only for now).
    pub geometry_mask: Option<GeometryMask>,
    /// The base content.
    pub kind: LayerKind,
    /// Content effects on top of the base, bottom → top.
    pub effects: Vec<ContentEffect>,
}

/// Which image of a layer an image is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerImage {
    /// The paint layer's image of a channel.
    Paint(Channel),
    /// The image of a channel in content effect `effect`.
    Effect { effect: usize, channel: Channel },
    /// The image of paint effect `effect` of the mask.
    Mask { effect: usize },
}

impl Layer {
    /// A visible, fully opaque layer with no channel entries, effects or masks yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: LayerKind) -> Layer {
        Layer { id: id.into(), name: name.into(), visible: true, opacity: 1.0, blend: Vec::new(), mask: None, geometry_mask: None, kind, effects: Vec::new() }
    }

    /// The blend entry for `channel`; `None` means the layer does not affect it.
    pub fn blend_for(&self, channel: Channel) -> Option<&ChannelBlend> {
        self.blend.iter().find(|b| b.channel == channel)
    }

    /// Adds or replaces the blend entry for `channel`.
    pub fn set_blend(&mut self, channel: Channel, mode: BlendMode, opacity: f32) {
        let entry = ChannelBlend { channel, mode, opacity };
        match self.blend.iter_mut().find(|b| b.channel == channel) {
            Some(b) => *b = entry,
            None => self.blend.push(entry),
        }
    }

    /// Builder form of [`Self::set_blend`].
    pub fn with_blend(mut self, channel: Channel, mode: BlendMode, opacity: f32) -> Layer {
        self.set_blend(channel, mode, opacity);
        self
    }

    /// Builder: a painted mask (black with `mask` painted on top, see [`Mask::from_image`]).
    pub fn with_mask(mut self, mask: TiledImage) -> Layer {
        self.mask = Some(Mask::from_image(mask));
        self
    }

    /// Builder: adds `effect` on top of the content.
    pub fn with_effect(mut self, effect: ContentEffect) -> Layer {
        self.effects.push(effect);
        self
    }

    pub fn children(&self) -> &[Layer] {
        match &self.kind {
            LayerKind::Group(c) => c,
            _ => &[],
        }
    }

    /// Every image the layer owns: paint images, content effect images and mask images, in that order.
    pub fn images(&self) -> Vec<(LayerImage, &TiledImage)> {
        let mut out: Vec<(LayerImage, &TiledImage)> = Vec::new();
        if let LayerKind::Paint(p) = &self.kind {
            out.extend(p.images.iter().map(|(c, i)| (LayerImage::Paint(*c), i)));
        }
        for (effect, e) in self.effects.iter().enumerate() {
            if let EffectKind::Paint(p) = &e.kind {
                out.extend(p.images.iter().map(|(channel, i)| (LayerImage::Effect { effect, channel: *channel }, i)));
            }
        }
        if let Some(m) = &self.mask {
            out.extend(m.images().map(|(effect, i)| (LayerImage::Mask { effect }, i)));
        }
        out
    }

    /// Mutable access to one of [`Self::images`].
    pub fn image_mut(&mut self, which: LayerImage) -> Option<&mut TiledImage> {
        match which {
            LayerImage::Paint(c) => match &mut self.kind {
                LayerKind::Paint(p) => p.image_mut(c),
                _ => None,
            },
            LayerImage::Effect { effect, channel } => match &mut self.effects.get_mut(effect)?.kind {
                EffectKind::Paint(p) => p.image_mut(channel),
                _ => None,
            },
            LayerImage::Mask { effect } => self.mask.as_mut()?.image_mut(effect),
        }
    }
}

/// Depth-first, pre-order walk over a layer tree, yielding `(depth, layer)`; a group comes before
/// its children. Created by [`crate::paint::TextureSet::walk_layers`].
pub struct LayerWalk<'a> {
    stack: Vec<std::slice::Iter<'a, Layer>>,
}

impl<'a> LayerWalk<'a> {
    pub fn new(layers: &'a [Layer]) -> LayerWalk<'a> {
        LayerWalk { stack: vec![layers.iter()] }
    }
}

impl<'a> Iterator for LayerWalk<'a> {
    type Item = (usize, &'a Layer);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let depth = self.stack.len().checked_sub(1)?;
            match self.stack.last_mut()?.next() {
                Some(l) => {
                    if let LayerKind::Group(children) = &l.kind {
                        self.stack.push(children.iter());
                    }
                    return Some((depth, l));
                }
                None => {
                    self.stack.pop();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::PixelFormat;
    use crate::paint::effect::{GreyBlend, MaskEffect, MaskPixels};

    #[test]
    fn blend_entries_and_walk() {
        let mut l = Layer::new("a", "A", LayerKind::Paint(PaintContent::default())).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0);
        l.set_blend(Channel::BaseColor, BlendMode::Multiply, 0.5);
        assert_eq!(l.blend.len(), 1);
        assert_eq!(l.blend_for(Channel::BaseColor).map(|b| b.mode), Some(BlendMode::Multiply));
        assert!(l.blend_for(Channel::Roughness).is_none());
        let g = Layer::new("g", "G", LayerKind::Group(vec![l.clone(), Layer::new("b", "B", LayerKind::Group(vec![]))]));
        let top = vec![g, Layer::new("c", "C", LayerKind::Fill(Fill::uniform(vec![])))];
        let ids: Vec<_> = LayerWalk::new(&top).map(|(d, l)| format!("{d}{}", l.id)).collect();
        assert_eq!(ids, ["0g", "1a", "1b", "0c"]);
        assert_eq!(Fill::uniform(vec![(Channel::Metallic, [1.0; 4])]).uniform_value(Channel::Metallic), Some([1.0; 4]));
    }

    #[test]
    fn images_of_base_effects_and_mask() {
        let img = || TiledImage::new(2, 2, PixelFormat::from_id("graya8").unwrap()).unwrap();
        let mut base = PaintContent::default();
        base.set_image(Channel::Roughness, img());
        let mut fx = PaintContent::default();
        fx.set_image(Channel::Height, img());
        let mask = Mask::new(1.0)
            .with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::Fill(0.5)))
            .with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::Paint(MaskPixels { image: img() })));
        let mut l = Layer::new("l", "L", LayerKind::Paint(base)).with_effect(ContentEffect::new(Vec::new(), EffectKind::Paint(fx)));
        l.mask = Some(mask);
        let roles: Vec<_> = l.images().into_iter().map(|(r, _)| r).collect();
        assert_eq!(roles, [LayerImage::Paint(Channel::Roughness), LayerImage::Effect { effect: 0, channel: Channel::Height }, LayerImage::Mask { effect: 1 }]);
        for r in roles {
            assert!(l.image_mut(r).is_some(), "{r:?}");
        }
        assert!(l.image_mut(LayerImage::Mask { effect: 0 }).is_none());
    }
}
