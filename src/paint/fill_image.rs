//! [`ImageFill`]: a fill whose channels come from images (bitmaps) laid on by the fill's projection, with
//! constant values for the channels that have no image.
//!
//! Each image keeps its prefiltered level chain ([`LevelChain`]) next to it, built on first use and shared by
//! clones; replacing an image drops it.

use super::channel::Channel;
use super::effect::EffectKind;
use super::image::TiledImage;
use super::layer::{FillSource, Layer, LayerKind};
use crate::projection::{ImageLevels, LevelChain, Texels};
use std::fmt;
use std::sync::{Arc, OnceLock};

impl Texels for TiledImage {
    fn size(&self) -> (u32, u32) {
        TiledImage::size(self)
    }

    fn texel(&self, x: u32, y: u32) -> [f32; 4] {
        self.get(x, y)
    }
}

/// One channel's image of an [`ImageFill`]: straight values in the channel's stored encoding, alpha = coverage.
/// Any size; it is sampled through the fill's projection.
#[derive(Clone)]
pub struct FillImage {
    image: TiledImage,
    levels: Arc<OnceLock<LevelChain>>,
}

impl FillImage {
    pub fn new(image: TiledImage) -> FillImage {
        FillImage { image, levels: Arc::new(OnceLock::new()) }
    }

    pub fn image(&self) -> &TiledImage {
        &self.image
    }

    /// The image with its prefiltered levels (built on the first call).
    pub fn levels(&self) -> ImageLevels<'_> {
        let chain = self.levels.get_or_init(|| LevelChain::build(&self.image));
        ImageLevels::new(&self.image, chain)
    }
}

impl PartialEq for FillImage {
    fn eq(&self, other: &FillImage) -> bool {
        self.image == other.image
    }
}

impl fmt::Debug for FillImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FillImage").field("size", &self.image.size()).field("format", &self.image.format().id()).finish()
    }
}

/// Images per channel, and constants (normalised RGBA in the stored encoding, as a uniform fill's) for the
/// channels without an image. A channel is in at most one of the two lists.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImageFill {
    images: Vec<(Channel, FillImage)>,
    values: Vec<(Channel, [f32; 4])>,
}

impl ImageFill {
    pub fn images(&self) -> impl Iterator<Item = (Channel, &FillImage)> {
        self.images.iter().map(|(c, i)| (*c, i))
    }

    pub fn values(&self) -> &[(Channel, [f32; 4])] {
        &self.values
    }

    pub fn image(&self, channel: Channel) -> Option<&FillImage> {
        self.images.iter().find(|(c, _)| *c == channel).map(|(_, i)| i)
    }

    pub fn value(&self, channel: Channel) -> Option<[f32; 4]> {
        self.values.iter().find(|(c, _)| *c == channel).map(|(_, v)| *v)
    }

    /// Sets `channel` to `image` (replacing an image or a constant).
    pub fn set_image(&mut self, channel: Channel, image: TiledImage) {
        self.values.retain(|(c, _)| *c != channel);
        let image = FillImage::new(image);
        match self.images.iter_mut().find(|(c, _)| *c == channel) {
            Some(slot) => slot.1 = image,
            None => self.images.push((channel, image)),
        }
    }

    /// Sets `channel` to a constant (replacing an image or a constant).
    pub fn set_value(&mut self, channel: Channel, value: [f32; 4]) {
        self.images.retain(|(c, _)| *c != channel);
        match self.values.iter_mut().find(|(c, _)| *c == channel) {
            Some(slot) => slot.1 = value,
            None => self.values.push((channel, value)),
        }
    }

    /// Every channel the fill sets: images first, then constants.
    pub fn channels(&self) -> Vec<Channel> {
        self.images.iter().map(|(c, _)| *c).chain(self.values.iter().map(|(c, _)| *c)).collect()
    }

    /// Mutable access to the image of `channel` for an editor; the level chain is rebuilt on the next use.
    pub fn image_mut(&mut self, channel: Channel) -> Option<&mut TiledImage> {
        let slot = self.images.iter_mut().find(|(c, _)| *c == channel)?;
        slot.1.levels = Arc::new(OnceLock::new());
        Some(&mut slot.1.image)
    }
}

/// Which fill image of a layer: of the layer's own fill (`effect` = `None`) or of content effect `effect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FillImageRef {
    pub effect: Option<usize>,
    pub channel: Channel,
}

impl Layer {
    /// The images of the layer's image fills (its own fill first, then its content effects'). Unlike
    /// [`Layer::images`] they are not painted on and may have any size.
    pub fn fill_images(&self) -> Vec<(FillImageRef, &TiledImage)> {
        let fills = std::iter::once((None, &self.kind)).filter_map(|(e, k)| match k {
            LayerKind::Fill(f) => Some((e, f)),
            _ => None,
        });
        let effects = self.effects.iter().enumerate().filter_map(|(i, e)| match &e.kind {
            EffectKind::Fill(f) => Some((Some(i), f)),
            _ => None,
        });
        let mut out = Vec::new();
        for (effect, fill) in fills.chain(effects) {
            if let FillSource::Image(images) = &fill.source {
                out.extend(images.images().map(|(channel, i)| (FillImageRef { effect, channel }, i.image())));
            }
        }
        out
    }

    /// Mutable access to one of [`Self::fill_images`].
    pub fn fill_image_mut(&mut self, which: FillImageRef) -> Option<&mut TiledImage> {
        let fill = match which.effect {
            None => match &mut self.kind {
                LayerKind::Fill(f) => f,
                _ => return None,
            },
            Some(i) => match &mut self.effects.get_mut(i)?.kind {
                EffectKind::Fill(f) => f,
                _ => return None,
            },
        };
        match &mut fill.source {
            FillSource::Image(images) => images.image_mut(which.channel),
            _ => None,
        }
    }
}
