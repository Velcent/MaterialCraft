//! How a stroke's coverage lands in a channel image: [`StampMode`] and the per-pixel blend from the pixel as it
//! was before the stroke.

use crate::paint::TiledImage;
use crate::paint::working_space::{linear_to_srgb, srgb_to_linear};

/// What a stroke does to its images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StampMode {
    /// Lays the value over what is there (images with alpha keep coverage in alpha, straight colour).
    Paint,
    /// Lowers coverage (alpha) and keeps the stored values; images without alpha go towards black (PT-20).
    Erase,
}

/// One image a stroke writes, with the value it paints there.
pub struct ChannelTarget<'a> {
    pub image: &'a mut TiledImage,
    /// Straight RGBA in the image's stored encoding; alpha scales the coverage.
    pub value: [f32; 4],
    pub mode: StampMode,
    /// The image stores sRGB-encoded colour: blend in linear light (as layers composite).
    pub srgb: bool,
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
        let v = t.value.map(|c| if c.is_finite() { c } else { 0.0 });
        let value = if t.srgb { [srgb_to_linear(v[0]), srgb_to_linear(v[1]), srgb_to_linear(v[2]), v[3]] } else { v };
        Blend { mode: t.mode, value, srgb: t.srgb, has_alpha: t.image.format().layout.has_alpha() }
    }

    /// The pixel that was `orig` before the stroke, under stroke coverage `c` (0 … 1).
    #[inline]
    pub fn apply(&self, orig: [f32; 4], c: f32) -> [f32; 4] {
        if c <= 0.0 {
            return orig;
        }
        match (self.mode, self.has_alpha) {
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

    fn mix(&self, orig: [f32; 4], to: [f32; 4], c: f32) -> [f32; 4] {
        let o = self.decode(orig);
        let [o0, o1, o2, _] = o;
        let [t0, t1, t2, _] = to;
        self.encode([o0 + (t0 - o0) * c, o1 + (t1 - o1) * c, o2 + (t2 - o2) * c, orig[3]])
    }

    fn decode(&self, p: [f32; 4]) -> [f32; 4] {
        if self.srgb { [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2]), p[3]] } else { p }
    }

    fn encode(&self, p: [f32; 4]) -> [f32; 4] {
        if self.srgb { [linear_to_srgb(p[0]), linear_to_srgb(p[1]), linear_to_srgb(p[2]), p[3]] } else { p }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::PixelFormat;

    fn target(img: &mut TiledImage, value: [f32; 4], mode: StampMode) -> Blend {
        Blend::new(&ChannelTarget { image: img, value, mode, srgb: false })
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
    fn srgb_images_blend_in_linear_light() {
        let mut img = TiledImage::new(4, 4, PixelFormat::from_id("rgb8").unwrap()).unwrap();
        let b = Blend::new(&ChannelTarget { image: &mut img, value: [1.0, 1.0, 1.0, 1.0], mode: StampMode::Paint, srgb: true });
        let half = b.apply([0.0, 0.0, 0.0, 1.0], 0.5);
        assert!((half[0] - linear_to_srgb(0.5)).abs() < 1e-5);
    }
}
