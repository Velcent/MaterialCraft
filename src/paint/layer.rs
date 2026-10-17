//! Layers of a texture set's stack: paint, fill and group layers, their per-channel blending and masks.

use super::blend::BlendMode;
use super::channel::Channel;
use super::image::TiledImage;

/// How a layer affects one channel. A layer without an entry for a channel does not affect it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelBlend {
    pub channel: Channel,
    pub mode: BlendMode,
    /// 0..1, multiplied with the layer opacity.
    pub opacity: f32,
}

/// Restricts where a layer applies: the mask's value (red × alpha) scales the layer's strength.
#[derive(Debug, Clone, PartialEq)]
pub enum Mask {
    /// A grayscale image at the texture set's resolution.
    Image(TiledImage),
}

impl Mask {
    pub fn image(&self) -> &TiledImage {
        match self {
            Mask::Image(img) => img,
        }
    }

    pub fn image_mut(&mut self) -> &mut TiledImage {
        match self {
            Mask::Image(img) => img,
        }
    }
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
    pub kind: LayerKind,
}

impl Layer {
    /// A visible, fully opaque layer with no channel entries yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: LayerKind) -> Layer {
        Layer { id: id.into(), name: name.into(), visible: true, opacity: 1.0, blend: Vec::new(), mask: None, kind }
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

    pub fn with_mask(mut self, mask: TiledImage) -> Layer {
        self.mask = Some(Mask::Image(mask));
        self
    }

    pub fn children(&self) -> &[Layer] {
        match &self.kind {
            LayerKind::Group(c) => c,
            _ => &[],
        }
    }

    /// Every image the layer owns (paint images and mask), with the channel for paint images.
    pub fn images(&self) -> Vec<(Option<Channel>, &TiledImage)> {
        let mut out: Vec<(Option<Channel>, &TiledImage)> = Vec::new();
        if let LayerKind::Paint(p) = &self.kind {
            out.extend(p.images.iter().map(|(c, i)| (Some(*c), i)));
        }
        if let Some(m) = &self.mask {
            out.push((None, m.image()));
        }
        out
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
}
