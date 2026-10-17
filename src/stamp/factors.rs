//! Factors that scale every stamp per texel: the quick mask (PT-42, a greyscale image in texture space) and the
//! stencil (PT-15, an image laid over the view). The tools that edit them come later; the stamp stage multiplies in
//! whatever factors a stroke is given.

use crate::paint::TiledImage;
use crate::projection::ScreenImageTransform;

/// A per-texel multiplier, 0 (no paint) … 1.
pub trait TexelFactor: Sync {
    /// The factor at texel `(x, y)`; `screen` is where the texel lies on screen for 3D-view strokes.
    fn factor(&self, x: u32, y: u32, screen: Option<[f32; 2]>) -> f32;
}

/// A quick mask: no paint where its image is black. Its size must be the texture set's.
pub struct QuickMask<'a> {
    pub image: &'a TiledImage,
    /// Swap black and white.
    pub inverted: bool,
}

impl TexelFactor for QuickMask<'_> {
    fn factor(&self, x: u32, y: u32, _screen: Option<[f32; 2]>) -> f32 {
        let v = self.image.get(x, y)[0].clamp(0.0, 1.0);
        if self.inverted { 1.0 - v } else { v }
    }
}

/// A stencil: a greyscale image over the view, sampled (nearest) at each texel's screen position. 2D-view strokes
/// (no screen position) are not stencilled.
pub struct Stencil<'a> {
    pub image: &'a TiledImage,
    pub transform: ScreenImageTransform,
}

impl TexelFactor for Stencil<'_> {
    fn factor(&self, _x: u32, _y: u32, screen: Option<[f32; 2]>) -> f32 {
        let Some(s) = screen else { return 1.0 };
        let uv = self.transform.to_image(s);
        if !self.transform.covers(uv) {
            return 0.0;
        }
        let (w, h) = (self.image.width().max(1) as f32, self.image.height().max(1) as f32);
        let (u, v) = (uv[0].rem_euclid(1.0), uv[1].rem_euclid(1.0));
        self.image.get((u * w) as u32, (v * h) as u32)[0].clamp(0.0, 1.0)
    }
}
