//! Pixel formats: how many components a pixel has and how each sample is stored in bytes.
//!
//! Samples are little-endian. Integer samples map to 0..1 (`U8` / 255, `U16` / 65535); float
//! samples are stored as is. Gray layouts store the red component and expand to `[v, v, v, a]`;
//! layouts without alpha read alpha as 1.

use crate::half::{f16_to_f32, f32_to_f16};

/// Storage type of one sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleType {
    U8,
    U16,
    F16,
    F32,
}

impl SampleType {
    pub const ALL: [SampleType; 4] = [SampleType::U8, SampleType::U16, SampleType::F16, SampleType::F32];

    pub fn bytes(self) -> usize {
        match self {
            SampleType::U8 => 1,
            SampleType::U16 | SampleType::F16 => 2,
            SampleType::F32 => 4,
        }
    }

    /// True for the float types, which can hold values outside 0..1.
    pub fn is_float(self) -> bool {
        matches!(self, SampleType::F16 | SampleType::F32)
    }

    /// Decodes one sample from the start of `b` (which must hold at least [`Self::bytes`] bytes).
    #[inline]
    pub fn decode(self, b: &[u8]) -> f32 {
        match self {
            SampleType::U8 => b[0] as f32 / 255.0,
            SampleType::U16 => u16::from_le_bytes([b[0], b[1]]) as f32 / 65535.0,
            SampleType::F16 => f16_to_f32(u16::from_le_bytes([b[0], b[1]])),
            SampleType::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        }
    }

    /// Encodes one sample into the start of `b`. Integers clamp to 0..1 and round to nearest;
    /// NaN becomes 0.
    #[inline]
    pub fn encode(self, v: f32, b: &mut [u8]) {
        match self {
            SampleType::U8 => b[0] = (unit(v) * 255.0).round() as u8,
            SampleType::U16 => b[..2].copy_from_slice(&((unit(v) * 65535.0).round() as u16).to_le_bytes()),
            SampleType::F16 => b[..2].copy_from_slice(&f32_to_f16(v).to_le_bytes()),
            SampleType::F32 => b[..4].copy_from_slice(&v.to_le_bytes()),
        }
    }
}

fn unit(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

/// Which components a pixel has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelLayout {
    Gray,
    GrayAlpha,
    Rgb,
    Rgba,
}

impl PixelLayout {
    pub const ALL: [PixelLayout; 4] = [PixelLayout::Gray, PixelLayout::GrayAlpha, PixelLayout::Rgb, PixelLayout::Rgba];

    pub fn components(self) -> usize {
        match self {
            PixelLayout::Gray => 1,
            PixelLayout::GrayAlpha => 2,
            PixelLayout::Rgb => 3,
            PixelLayout::Rgba => 4,
        }
    }

    pub fn has_alpha(self) -> bool {
        matches!(self, PixelLayout::GrayAlpha | PixelLayout::Rgba)
    }

    /// The same layout with an alpha component (used for paint layers, where alpha is coverage).
    pub fn with_alpha(self) -> PixelLayout {
        match self {
            PixelLayout::Gray | PixelLayout::GrayAlpha => PixelLayout::GrayAlpha,
            PixelLayout::Rgb | PixelLayout::Rgba => PixelLayout::Rgba,
        }
    }

    /// The same layout without alpha.
    pub fn without_alpha(self) -> PixelLayout {
        match self {
            PixelLayout::Gray | PixelLayout::GrayAlpha => PixelLayout::Gray,
            PixelLayout::Rgb | PixelLayout::Rgba => PixelLayout::Rgb,
        }
    }
}

/// Layout plus sample type. String ids look like `gray8`, `rgba16`, `rgb16f`, `graya32f`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelFormat {
    pub layout: PixelLayout,
    pub sample: SampleType,
}

impl PixelFormat {
    pub const fn new(layout: PixelLayout, sample: SampleType) -> PixelFormat {
        PixelFormat { layout, sample }
    }

    pub fn bytes_per_sample(self) -> usize {
        self.sample.bytes()
    }

    pub fn bytes_per_pixel(self) -> usize {
        self.layout.components() * self.sample.bytes()
    }

    pub fn with_alpha(self) -> PixelFormat {
        PixelFormat::new(self.layout.with_alpha(), self.sample)
    }

    pub fn without_alpha(self) -> PixelFormat {
        PixelFormat::new(self.layout.without_alpha(), self.sample)
    }

    /// Decodes one pixel (`b` holds at least [`Self::bytes_per_pixel`] bytes) into normalised RGBA.
    #[inline]
    pub fn decode(self, b: &[u8]) -> [f32; 4] {
        let s = self.sample;
        let n = s.bytes();
        match self.layout {
            PixelLayout::Gray => {
                let v = s.decode(b);
                [v, v, v, 1.0]
            }
            PixelLayout::GrayAlpha => {
                let v = s.decode(b);
                [v, v, v, s.decode(&b[n..])]
            }
            PixelLayout::Rgb => [s.decode(b), s.decode(&b[n..]), s.decode(&b[2 * n..]), 1.0],
            PixelLayout::Rgba => [s.decode(b), s.decode(&b[n..]), s.decode(&b[2 * n..]), s.decode(&b[3 * n..])],
        }
    }

    /// Encodes normalised RGBA into one pixel. Gray layouts keep the red component.
    #[inline]
    pub fn encode(self, p: [f32; 4], b: &mut [u8]) {
        let s = self.sample;
        let n = s.bytes();
        let comps: &[usize] = match self.layout {
            PixelLayout::Gray => &[0],
            PixelLayout::GrayAlpha => &[0, 3],
            PixelLayout::Rgb => &[0, 1, 2],
            PixelLayout::Rgba => &[0, 1, 2, 3],
        };
        for (i, &c) in comps.iter().enumerate() {
            s.encode(p[c], &mut b[i * n..]);
        }
    }

    /// Stable string id, e.g. `rgb8`, `gray16`, `rgba16f`.
    pub fn id(self) -> String {
        let layout = match self.layout {
            PixelLayout::Gray => "gray",
            PixelLayout::GrayAlpha => "graya",
            PixelLayout::Rgb => "rgb",
            PixelLayout::Rgba => "rgba",
        };
        let sample = match self.sample {
            SampleType::U8 => "8",
            SampleType::U16 => "16",
            SampleType::F16 => "16f",
            SampleType::F32 => "32f",
        };
        format!("{layout}{sample}")
    }

    /// Parses an id written by [`Self::id`].
    pub fn from_id(id: &str) -> Option<PixelFormat> {
        let digits = id.find(|c: char| c.is_ascii_digit())?;
        let layout = match &id[..digits] {
            "gray" => PixelLayout::Gray,
            "graya" => PixelLayout::GrayAlpha,
            "rgb" => PixelLayout::Rgb,
            "rgba" => PixelLayout::Rgba,
            _ => return None,
        };
        let sample = match &id[digits..] {
            "8" => SampleType::U8,
            "16" => SampleType::U16,
            "16f" => SampleType::F16,
            "32f" => SampleType::F32,
            _ => return None,
        };
        Some(PixelFormat::new(layout, sample))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(f: PixelFormat, p: [f32; 4]) -> [f32; 4] {
        let mut b = vec![0u8; f.bytes_per_pixel()];
        f.encode(p, &mut b);
        f.decode(&b)
    }

    #[test]
    fn every_sample_type_round_trips() {
        for s in SampleType::ALL {
            let tol = match s {
                SampleType::U8 => 0.5 / 255.0,
                SampleType::U16 => 0.5 / 65535.0,
                SampleType::F16 => 1e-3,
                SampleType::F32 => 0.0,
            };
            for v in [0.0f32, 0.1, 0.25, 0.5, 0.77, 1.0] {
                let mut b = [0u8; 4];
                s.encode(v, &mut b);
                assert!((s.decode(&b) - v).abs() <= tol + 1e-6, "{s:?} {v}");
            }
        }
    }

    #[test]
    fn integer_codes_are_exact() {
        for i in 0..=255u8 {
            let mut b = [0u8; 1];
            SampleType::U8.encode(SampleType::U8.decode(&[i]), &mut b);
            assert_eq!(b[0], i);
        }
        for i in (0..=65535u16).step_by(7) {
            let src = i.to_le_bytes();
            let mut b = [0u8; 2];
            SampleType::U16.encode(SampleType::U16.decode(&src), &mut b);
            assert_eq!(u16::from_le_bytes(b), i);
        }
    }

    #[test]
    fn f16_is_exact_for_half_values_and_keeps_range() {
        for v in [0.5f32, -2.0, 1024.0, 0.099975586, 65504.0] {
            let mut b = [0u8; 2];
            SampleType::F16.encode(v, &mut b);
            assert_eq!(SampleType::F16.decode(&b), v);
        }
        let mut b = [0u8; 2];
        SampleType::U8.encode(f32::NAN, &mut b);
        assert_eq!(b[0], 0);
        SampleType::U8.encode(3.0, &mut b);
        assert_eq!(b[0], 255);
    }

    #[test]
    fn layouts_expand_and_fill_alpha() {
        let p = [0.2, 0.4, 0.6, 0.5];
        let f32fmt = |l| PixelFormat::new(l, SampleType::F32);
        assert_eq!(round_trip(f32fmt(PixelLayout::Gray), p), [0.2, 0.2, 0.2, 1.0]);
        assert_eq!(round_trip(f32fmt(PixelLayout::GrayAlpha), p), [0.2, 0.2, 0.2, 0.5]);
        assert_eq!(round_trip(f32fmt(PixelLayout::Rgb), p), [0.2, 0.4, 0.6, 1.0]);
        assert_eq!(round_trip(f32fmt(PixelLayout::Rgba), p), p);
        assert_eq!(PixelFormat::new(PixelLayout::Rgba, SampleType::U16).bytes_per_pixel(), 8);
    }

    #[test]
    fn ids_round_trip() {
        for l in PixelLayout::ALL {
            for s in SampleType::ALL {
                let f = PixelFormat::new(l, s);
                assert_eq!(PixelFormat::from_id(&f.id()), Some(f));
            }
        }
        assert_eq!(PixelFormat::from_id("rgb8").map(|f| f.layout), Some(PixelLayout::Rgb));
        assert_eq!(PixelFormat::from_id("cmyk8"), None);
        assert_eq!(PixelFormat::from_id("rgb"), None);
    }
}
