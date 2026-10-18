//! Projection painting (PJ-26): an image or a material laid over the view ([`ScreenImageTransform`]) and painted
//! onto the visible surface through the brush. The brush's stroke decides where and how strongly paint lands
//! (dabs, flow, opacity, the depth test); [`ProjectedPaint`] decides what lands there: per channel the image's value
//! at the texel's place on screen ([`TexelValue`], through [`ScreenProjector::image_tap`]), and, as a
//! [`TexelFactor`], that nothing lands where the image does not cover the screen.
//!
//! Projection painting has no symmetry (ADR 0008 mc2-q-pt-04c): the stroke's caller paints it without copies.

use super::factors::TexelFactor;
use super::write::TexelValue;
use crate::mesh::SurfaceMap;
use crate::paint::Channel;
use crate::projection::{Filter, ImageLevels, ScreenImageTransform, ScreenProjector, ViewProjector};

/// Texel → image coordinate through the view: shared by every channel of one projection.
#[derive(Clone, Copy)]
struct Lookup<'a> {
    surface: &'a SurfaceMap,
    view: &'a dyn ViewProjector,
    image: ScreenImageTransform,
    filter: Filter,
}

impl Lookup<'_> {
    /// The image coordinate and footprint of texel `(x, y)`'s surface point; `None` without a surface point or
    /// behind the camera.
    fn tap(&self, x: u32, y: u32) -> Option<([f32; 2], f32)> {
        let i = (y as usize).checked_mul(self.surface.width())?.checked_add(x as usize)?;
        let (pos, _, _) = self.surface.point_index(i)?;
        let tap = ScreenProjector { view: self.view, image: self.image }.image_tap(pos)?;
        Some((tap.uv, tap.footprint))
    }
}

/// One channel's projected image.
pub struct ProjectedChannel<'a> {
    lookup: Lookup<'a>,
    levels: ImageLevels<'a>,
    /// Paint the image's red value as grey (masks).
    grey: bool,
}

impl TexelValue for ProjectedChannel<'_> {
    fn value(&self, x: u32, y: u32) -> Option<[f32; 4]> {
        let (uv, footprint) = self.lookup.tap(x, y)?;
        let v = self.levels.sample(uv, footprint, self.lookup.filter, self.lookup.image.repeat)?;
        Some(if self.grey { [v[0], v[0], v[0], v[3]] } else { v })
    }
}

/// An image or material laid over the view, ready to paint through: per channel an image (values in the channel's
/// stored encoding, straight alpha scaling the coverage) and optionally a grey image for masks.
pub struct ProjectedPaint<'a> {
    lookup: Lookup<'a>,
    channels: Vec<(Channel, ProjectedChannel<'a>)>,
    mask: Option<ProjectedChannel<'a>>,
}

impl<'a> ProjectedPaint<'a> {
    /// A projection through `view` of images placed by `image`, onto the texels of `surface` (the painted set's
    /// surface map), sampled with `filter`. Add its images with [`Self::with_channel`] and [`Self::with_mask`].
    pub fn new(surface: &'a SurfaceMap, view: &'a dyn ViewProjector, image: ScreenImageTransform, filter: Filter) -> ProjectedPaint<'a> {
        ProjectedPaint { lookup: Lookup { surface, view, image, filter }, channels: Vec::new(), mask: None }
    }

    /// Builder: `channel` paints `levels` (replacing an earlier image for it).
    pub fn with_channel(mut self, channel: Channel, levels: ImageLevels<'a>) -> ProjectedPaint<'a> {
        self.channels.retain(|(c, _)| *c != channel);
        self.channels.push((channel, ProjectedChannel { lookup: self.lookup, levels, grey: false }));
        self
    }

    /// Builder: masks paint the red values of `levels` as grey.
    pub fn with_mask(mut self, levels: ImageLevels<'a>) -> ProjectedPaint<'a> {
        self.mask = Some(ProjectedChannel { lookup: self.lookup, levels, grey: true });
        self
    }

    /// The channels the projection provides (others are left alone).
    pub fn channels(&self) -> impl Iterator<Item = Channel> + '_ {
        self.channels.iter().map(|(c, _)| *c)
    }

    /// What `channel` paints per texel, if the projection provides it.
    pub fn channel(&self, channel: Channel) -> Option<&dyn TexelValue> {
        self.channels.iter().find(|(c, _)| *c == channel).map(|(_, p)| p as &dyn TexelValue)
    }

    /// What masks paint per texel, if the projection provides a grey image.
    pub fn mask(&self) -> Option<&dyn TexelValue> {
        self.mask.as_ref().map(|p| p as &dyn TexelValue)
    }

    /// Where the image lies on screen.
    pub fn placement(&self) -> ScreenImageTransform {
        self.lookup.image
    }
}

/// No paint where the image does not cover the texel's screen position (2D-view strokes, which have none, paint
/// nothing).
impl TexelFactor for ProjectedPaint<'_> {
    fn factor(&self, _x: u32, _y: u32, screen: Option<[f32; 2]>) -> f32 {
        let Some(s) = screen else { return 0.0 };
        if self.lookup.image.covers(self.lookup.image.to_image(s)) { 1.0 } else { 0.0 }
    }
}
