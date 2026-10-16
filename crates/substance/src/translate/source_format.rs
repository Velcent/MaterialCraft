//! [`SourceFormat`]: Painter's stored pixel formats (texture-cache GL formats and image-resource formats),
//! how to read one pixel, and which of our formats holds it without loss.
//!
//! Texture-cache pixels are premultiplied by coverage (the last component); colour (sRGB) entries are
//! premultiplied in linear light before sRGB encoding (Verified on partially covered corpus pixels).
//! Image resources are straight.

use crate::decode::image::PixelFormat as ImageFormat;
use materialcraft::half::f16_to_f32;
use materialcraft::paint::{PixelFormat, PixelLayout, SampleType};

/// GL internal formats found in the texture cache.
const GL_SRGB8_ALPHA8: u32 = 0x8C43;
const GL_RG8: u32 = 0x822B;
const GL_RG16F: u32 = 0x822F;
const GL_RGBA16F: u32 = 0x881A;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    /// Cache: sRGB colour + coverage, premultiplied in linear light.
    CacheSrgb8Alpha8,
    /// Cache: value + coverage, premultiplied.
    CacheRg8,
    CacheRg16F,
    /// Cache: encoded vector + coverage, premultiplied (normals).
    CacheRgba16F,
    Bgra8,
    Rgba8,
    Rgba16,
    Rgba16F,
    Rgba32F,
    Lum8,
    Lum16,
}

/// How the colour components relate to the last (coverage) component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alpha {
    Straight,
    Premultiplied,
    /// Premultiplied in linear light, then sRGB-encoded.
    PremultipliedLinearSrgb,
}

impl SourceFormat {
    pub fn from_gl(internal_format: u32) -> Option<SourceFormat> {
        Some(match internal_format {
            GL_SRGB8_ALPHA8 => SourceFormat::CacheSrgb8Alpha8,
            GL_RG8 => SourceFormat::CacheRg8,
            GL_RG16F => SourceFormat::CacheRg16F,
            GL_RGBA16F => SourceFormat::CacheRgba16F,
            _ => return None,
        })
    }

    pub fn from_image(format: &ImageFormat) -> Option<SourceFormat> {
        Some(match format {
            ImageFormat::Bgra8 => SourceFormat::Bgra8,
            ImageFormat::Rgba8 => SourceFormat::Rgba8,
            ImageFormat::Rgba16 => SourceFormat::Rgba16,
            ImageFormat::Rgba16F => SourceFormat::Rgba16F,
            ImageFormat::Rgba32F => SourceFormat::Rgba32F,
            ImageFormat::Lum8 => SourceFormat::Lum8,
            ImageFormat::Lum16 => SourceFormat::Lum16,
            ImageFormat::Other(_) => return None,
        })
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            SourceFormat::Lum8 => 1,
            SourceFormat::CacheRg8 | SourceFormat::Lum16 => 2,
            SourceFormat::CacheSrgb8Alpha8 | SourceFormat::CacheRg16F | SourceFormat::Bgra8 | SourceFormat::Rgba8 => 4,
            SourceFormat::CacheRgba16F | SourceFormat::Rgba16 | SourceFormat::Rgba16F => 8,
            SourceFormat::Rgba32F => 16,
        }
    }

    pub fn alpha(self) -> Alpha {
        match self {
            SourceFormat::CacheSrgb8Alpha8 => Alpha::PremultipliedLinearSrgb,
            SourceFormat::CacheRg8 | SourceFormat::CacheRg16F | SourceFormat::CacheRgba16F => Alpha::Premultiplied,
            _ => Alpha::Straight,
        }
    }

    /// Our format that holds the values without loss (cache normals: 16-bit integers, as they are 0..1).
    pub fn target(self) -> PixelFormat {
        let f = PixelFormat::new;
        match self {
            SourceFormat::CacheSrgb8Alpha8 | SourceFormat::Bgra8 | SourceFormat::Rgba8 => {
                f(PixelLayout::Rgba, SampleType::U8)
            }
            SourceFormat::CacheRg8 => f(PixelLayout::GrayAlpha, SampleType::U8),
            SourceFormat::CacheRg16F => f(PixelLayout::GrayAlpha, SampleType::F16),
            SourceFormat::CacheRgba16F | SourceFormat::Rgba16 => f(PixelLayout::Rgba, SampleType::U16),
            SourceFormat::Rgba16F => f(PixelLayout::Rgba, SampleType::F16),
            SourceFormat::Rgba32F => f(PixelLayout::Rgba, SampleType::F32),
            SourceFormat::Lum8 => f(PixelLayout::Gray, SampleType::U8),
            SourceFormat::Lum16 => f(PixelLayout::Gray, SampleType::U16),
        }
    }

    /// One pixel as RGBA (gray formats in R, G and B; coverage or alpha in A). `b` holds one pixel.
    pub fn decode(self, b: &[u8]) -> [f32; 4] {
        let u8n = |i: usize| b[i] as f32 / 255.0;
        let u16n = |i: usize| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]) as f32 / 65535.0;
        let f16 = |i: usize| f16_to_f32(u16::from_le_bytes([b[2 * i], b[2 * i + 1]]));
        let f32n = |i: usize| f32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
        match self {
            SourceFormat::CacheSrgb8Alpha8 | SourceFormat::Rgba8 => [u8n(0), u8n(1), u8n(2), u8n(3)],
            SourceFormat::Bgra8 => [u8n(2), u8n(1), u8n(0), u8n(3)],
            SourceFormat::CacheRg8 => [u8n(0), u8n(0), u8n(0), u8n(1)],
            SourceFormat::CacheRg16F => [f16(0), f16(0), f16(0), f16(1)],
            SourceFormat::CacheRgba16F | SourceFormat::Rgba16F => [f16(0), f16(1), f16(2), f16(3)],
            SourceFormat::Rgba16 => [u16n(0), u16n(1), u16n(2), u16n(3)],
            SourceFormat::Rgba32F => [f32n(0), f32n(1), f32n(2), f32n(3)],
            SourceFormat::Lum8 => [u8n(0), u8n(0), u8n(0), 1.0],
            SourceFormat::Lum16 => [u16n(0), u16n(0), u16n(0), 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_swizzles_and_names_targets() {
        assert_eq!(SourceFormat::Bgra8.decode(&[0, 0, 255, 255]), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(SourceFormat::CacheRg8.decode(&[255, 0]), [1.0, 1.0, 1.0, 0.0]);
        assert_eq!(SourceFormat::Lum16.decode(&[0xff, 0xff])[0], 1.0);
        assert_eq!(SourceFormat::from_gl(0x822F), Some(SourceFormat::CacheRg16F));
        assert_eq!(SourceFormat::from_gl(1), None);
        assert_eq!(SourceFormat::CacheRg16F.target().id(), "graya16f");
        assert_eq!(SourceFormat::from_image(&ImageFormat::Lum8), Some(SourceFormat::Lum8));
        for f in [
            SourceFormat::Rgba32F,
            SourceFormat::CacheRgba16F,
            SourceFormat::Lum8,
            SourceFormat::Rgba16,
        ] {
            let zero = vec![0u8; f.bytes_per_pixel()];
            assert_eq!(f.decode(&zero)[0], 0.0);
        }
    }
}
