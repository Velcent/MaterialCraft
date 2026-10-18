//! How a stroke's coverage lands in a channel image: [`StampMode`] and the per-pixel blend from the pixel as it
//! was before the stroke (paint, erase, clone; the kernels of [`StampMode::Local`] run in `local`).

use crate::paint::TiledImage;
use crate::paint::working_space::{linear_to_srgb, srgb_to_linear};

/// What a stroke does to its images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StampMode {
    /// Lays the value over what is there (images with alpha keep coverage in alpha, straight colour).
    Paint,
    /// Lowers coverage (alpha) and keeps the stored values; images without alpha go towards black (PT-20).
    Erase,
    /// Copies the image as it was before the stroke from `offset` texels away (PT-27): texel `(x, y)` takes the
    /// pixel at `(x + offset[0], y + offset[1])`, so full coverage copies texel for texel. Where the source lies
    /// outside the image the texel stays. Paint content only, the mesh never changes (PT-30).
    Clone { offset: [i32; 2] },
    /// Each dab runs a [`super::LocalKernel`] (smudge, blur) on the values under it, in order, so a dab sees what
    /// the dabs before it did. The kernels are handed to the stroke with [`super::StampStroke::set_kernels`].
    Local,
}

/// One image a stroke writes, with the value it paints there.
pub struct ChannelTarget<'a> {
    pub image: &'a mut TiledImage,
    /// Straight RGBA in the image's stored encoding; alpha scales the coverage.
    pub value: [f32; 4],
    pub mode: StampMode,
    /// The image stores sRGB-encoded colour: blend in linear light (as layers composite).
    pub srgb: bool,
    /// A value per texel instead of `value` (projection painting, PJ-26); ignored by [`StampMode::Erase`]. Texels it
    /// has no value for keep their pixels.
    pub source: Option<&'a dyn TexelValue>,
}

/// A value per texel of a texture set, for [`ChannelTarget::source`].
pub trait TexelValue: Sync {
    /// Straight RGBA in the target image's stored encoding at texel `(x, y)` (alpha scales the coverage); `None`
    /// where there is nothing to paint.
    fn value(&self, x: u32, y: u32) -> Option<[f32; 4]>;
}

/// The per-pixel blend of one target, prepared once per stroke.
#[derive(Debug, Clone, Copy)]
pub(super) struct Blend {
    mode: StampMode,
    /// The value, in the space it blends in (linear light for sRGB images).
    value: [f32; 4],
    srgb: bool,
    has_alpha: bool,
}

impl Blend {
    pub fn new(t: &ChannelTarget) -> Blend {
        Blend { mode: t.mode, value: [0.0; 4], srgb: t.srgb, has_alpha: t.image.format().layout.has_alpha() }.with_value(t.value)
    }

    /// The same blend painting `value` (straight RGBA in the image's stored encoding) instead.
    #[inline]
    pub fn with_value(self, value: [f32; 4]) -> Blend {
        let v = value.map(|c| if c.is_finite() { c } else { 0.0 });
        let value = if self.srgb { [srgb_to_linear(v[0]), srgb_to_linear(v[1]), srgb_to_linear(v[2]), v[3]] } else { v };
        Blend { value, ..self }
    }

    /// True if the blend lays a value down (where a per-texel source applies).
    pub fn paints(&self) -> bool {
        self.mode == StampMode::Paint
    }

    pub fn mode(&self) -> StampMode {
        self.mode
    }

    /// The pixel that was `orig` before the stroke, under stroke coverage `c` (0 … 1).
    #[inline]
    pub fn apply(&self, orig: [f32; 4], c: f32) -> [f32; 4] {
        if c <= 0.0 {
            return orig;
        }
        match (self.mode, self.has_alpha) {
            // local kernels write as they go; clone takes its value per texel ([`Self::copy`])
            (StampMode::Local | StampMode::Clone { .. }, _) => orig,
            (StampMode::Erase, true) => [orig[0], orig[1], orig[2], orig[3] * (1.0 - c)],
            (StampMode::Erase, false) => self.mix(orig, [0.0, 0.0, 0.0, orig[3]], c),
            (StampMode::Paint, false) => self.mix(orig, self.value, c * self.value[3].clamp(0.0, 1.0)),
            (StampMode::Paint, true) => {
                let c = c * self.value[3].clamp(0.0, 1.0);
                let a0 = orig[3].clamp(0.0, 1.0);
                let a = c + a0 * (1.0 - c);
                if a <= 0.0 {
                    return orig;
                }
                let o = self.decode(orig);
                let k0 = a0 * (1.0 - c) / a;
                let k1 = c / a;
                let rgb = [o[0] * k0 + self.value[0] * k1, o[1] * k0 + self.value[1] * k1, o[2] * k0 + self.value[2] * k1];
                self.encode([rgb[0], rgb[1], rgb[2], a])
            }
        }
    }

    /// Clone: the pixel that was `orig` with `src` (both as stored) laid over it at coverage `c`. Full coverage
    /// gives `src` exactly; partial coverage mixes in linear light, premultiplied on images with alpha.
    #[inline]
    pub fn copy(&self, orig: [f32; 4], src: [f32; 4], c: f32) -> [f32; 4] {
        if c <= 0.0 || !src.iter().all(|v| v.is_finite()) {
            return orig;
        }
        if c >= 1.0 {
            return src;
        }
        if !self.has_alpha {
            let s = self.decode(src);
            let o = self.decode(orig);
            return self.encode([o[0] + (s[0] - o[0]) * c, o[1] + (s[1] - o[1]) * c, o[2] + (s[2] - o[2]) * c, orig[3]]);
        }
        let (o, s) = (self.decode(orig), self.decode(src));
        let a = o[3] + (s[3] - o[3]) * c;
        let ch = |oc: f32, sc: f32| if a > 1e-6 { (oc * o[3] + (sc * s[3] - oc * o[3]) * c) / a } else { oc + (sc - oc) * c };
        self.encode([ch(o[0], s[0]), ch(o[1], s[1]), ch(o[2], s[2]), a])
    }

    fn mix(&self, orig: [f32; 4], to: [f32; 4], c: f32) -> [f32; 4] {
        let o = self.decode(orig);
        let [o0, o1, o2, _] = o;
        let [t0, t1, t2, _] = to;
        self.encode([o0 + (t0 - o0) * c, o1 + (t1 - o1) * c, o2 + (t2 - o2) * c, orig[3]])
    }

    /// A stored pixel in the space it blends in (linear light for sRGB images).
    pub fn decode(&self, p: [f32; 4]) -> [f32; 4] {
        if self.srgb { [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2]), p[3]] } else { p }
    }

    /// The inverse of [`Self::decode`].
    pub fn encode(&self, p: [f32; 4]) -> [f32; 4] {
        if self.srgb { [linear_to_srgb(p[0]), linear_to_srgb(p[1]), linear_to_srgb(p[2]), p[3]] } else { p }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::PixelFormat;

    fn target(img: &mut TiledImage, value: [f32; 4], mode: StampMode) -> Blend {
        Blend::new(&ChannelTarget { image: img, value, mode, srgb: false, source: None })
    }

    #[test]
    fn paint_lays_over_and_erase_lowers_coverage() {
        let mut img = TiledImage::new(4, 4, PixelFormat::from_id("rgba32f").unwrap()).unwrap();
        let paint = target(&mut img, [1.0, 0.0, 0.0, 1.0], StampMode::Paint);
        assert_eq!(paint.apply([0.0, 0.0, 1.0, 0.0], 0.5), [1.0, 0.0, 0.0, 0.5], "over nothing the colour stays straight");
        assert_eq!(paint.apply([0.0, 0.0, 1.0, 1.0], 1.0), [1.0, 0.0, 0.0, 1.0]);
        let half = paint.apply([0.0, 0.0, 1.0, 1.0], 0.5);
        assert!((half[0] - 0.5).abs() < 1e-6 && (half[2] - 0.5).abs() < 1e-6 && half[3] == 1.0);
        assert_eq!(paint.apply([0.2, 0.3, 0.4, 0.5], 0.0), [0.2, 0.3, 0.4, 0.5]);
        let erase = target(&mut img, [0.0; 4], StampMode::Erase);
        assert_eq!(erase.apply([0.2, 0.3, 0.4, 0.8], 0.5), [0.2, 0.3, 0.4, 0.4]);
        let mut grey = TiledImage::new(4, 4, PixelFormat::from_id("gray8").unwrap()).unwrap();
        let erase = target(&mut grey, [0.0; 4], StampMode::Erase);
        assert_eq!(erase.apply([0.8, 0.8, 0.8, 1.0], 0.5)[0], 0.4, "without alpha the eraser goes towards black");
    }

    #[test]
    fn clone_copies_exactly_at_full_coverage_and_mixes_premultiplied_below() {
        let mut img = TiledImage::new(4, 4, PixelFormat::from_id("rgba8").unwrap()).unwrap();
        let b = Blend::new(&ChannelTarget { image: &mut img, value: [0.0; 4], mode: StampMode::Clone { offset: [3, 0] }, srgb: true, source: None });
        let src = [0.3, 0.6, 0.9, 0.7];
        assert_eq!(b.copy([0.1, 0.1, 0.1, 1.0], src, 1.0), src);
        assert_eq!(b.copy([0.1, 0.1, 0.1, 1.0], src, 0.0), [0.1, 0.1, 0.1, 1.0]);
        assert_eq!(b.apply([0.1, 0.1, 0.1, 1.0], 1.0), [0.1, 0.1, 0.1, 1.0], "clone takes its value per texel");
        // over a transparent pixel the source colour comes through unchanged
        let half = b.copy([0.0, 0.0, 0.0, 0.0], [1.0, 0.5, 0.25, 1.0], 0.5);
        assert!((half[0] - 1.0).abs() < 1e-5 && (half[1] - 0.5).abs() < 1e-5 && (half[3] - 0.5).abs() < 1e-6, "{half:?}");
    }

    #[test]
    fn srgb_images_blend_in_linear_light() {
        let mut img = TiledImage::new(4, 4, PixelFormat::from_id("rgb8").unwrap()).unwrap();
        let b = Blend::new(&ChannelTarget { image: &mut img, value: [1.0, 1.0, 1.0, 1.0], mode: StampMode::Paint, srgb: true, source: None });
        let half = b.apply([0.0, 0.0, 0.0, 1.0], 0.5);
        assert!((half[0] - linear_to_srgb(0.5)).abs() < 1e-5);
    }
}
