//! The space layers blend in, and the conversion between a channel's stored encoding and that space.
//!
//! Rule: sRGB-encoded channels (base colour, emissive, any channel declared `srgb`) composite in **linear
//! light**, as Substance Painter does: stored values are decoded to linear, blended (all modes, opacity and
//! coverage), and the result is encoded back to sRGB. Linear channels blend their stored values as they are.
//! Only RGB is converted; alpha (coverage) is never encoded.

use super::channel::ColorSpace;

/// The space a channel's layers blend in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkingSpace {
    /// Stored values blend as they are (linear channels).
    Stored,
    /// sRGB-encoded values are decoded to linear light, blended, and encoded again.
    LinearLight,
}

impl WorkingSpace {
    /// The working space for a channel stored in `color_space`.
    pub fn of(color_space: ColorSpace) -> WorkingSpace {
        match color_space {
            ColorSpace::Srgb => WorkingSpace::LinearLight,
            ColorSpace::Linear => WorkingSpace::Stored,
        }
    }
}

/// sRGB electro-optical transfer function (IEC 61966-2-1): encoded → linear. Mirrors below zero.
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Inverse of [`srgb_to_linear`]: linear → encoded.
pub fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Converts pixels between stored encoding and working space. Decoding uses a table for values that are
/// exact 8-bit steps (the usual storage of colour channels) and the formula otherwise.
#[derive(Debug, Clone)]
pub struct SpaceConverter {
    space: WorkingSpace,
    decode_u8: [f32; 256],
}

impl SpaceConverter {
    pub fn new(space: WorkingSpace) -> SpaceConverter {
        let mut decode_u8 = [0.0; 256];
        if space == WorkingSpace::LinearLight {
            for (k, v) in decode_u8.iter_mut().enumerate() {
                *v = srgb_to_linear(k as f32 / 255.0);
            }
        }
        SpaceConverter { space, decode_u8 }
    }

    pub fn space(&self) -> WorkingSpace {
        self.space
    }

    /// One stored pixel in working space.
    pub fn to_working(&self, p: [f32; 4]) -> [f32; 4] {
        match self.space {
            WorkingSpace::Stored => p,
            WorkingSpace::LinearLight => [
                self.decode(p[0]),
                self.decode(p[1]),
                self.decode(p[2]),
                p[3],
            ],
        }
    }

    /// Stored pixels → working space, in place.
    pub fn to_working_all(&self, px: &mut [[f32; 4]]) {
        if self.space == WorkingSpace::LinearLight {
            for p in px {
                *p = self.to_working(*p);
            }
        }
    }

    /// Working-space pixels → stored encoding, in place.
    pub fn to_stored_all(&self, px: &mut [[f32; 4]]) {
        if self.space == WorkingSpace::LinearLight {
            for p in px {
                *p = [
                    linear_to_srgb(p[0]),
                    linear_to_srgb(p[1]),
                    linear_to_srgb(p[2]),
                    p[3],
                ];
            }
        }
    }

    fn decode(&self, v: f32) -> f32 {
        let steps = v * 255.0;
        let k = (steps + 0.5) as usize; // saturating: negative and NaN give 0
        match self.decode_u8.get(k) {
            Some(lin) if (steps - k as f32).abs() < 1e-3 => *lin,
            _ => srgb_to_linear(v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_bit_round_trip_is_exact() {
        let c = SpaceConverter::new(WorkingSpace::LinearLight);
        for k in 0..=255u32 {
            let v = k as f32 / 255.0;
            let mut p = [c.to_working([v, v, v, 0.25])];
            assert!((p[0][0] - srgb_to_linear(v)).abs() < 1e-7);
            c.to_stored_all(&mut p);
            assert_eq!((p[0][0] * 255.0).round() as u32, k);
            assert!((p[0][0] - v).abs() < 1e-5, "{k}: {}", p[0][0]);
            assert_eq!(p[0][3], 0.25);
        }
    }

    #[test]
    fn known_values_and_rule() {
        assert!((srgb_to_linear(0.735_357) - 0.5).abs() < 1e-5);
        assert!((linear_to_srgb(0.5) - 0.735_357).abs() < 1e-5);
        assert_eq!(
            WorkingSpace::of(ColorSpace::Srgb),
            WorkingSpace::LinearLight
        );
        assert_eq!(WorkingSpace::of(ColorSpace::Linear), WorkingSpace::Stored);
        let c = SpaceConverter::new(WorkingSpace::LinearLight);
        let off_grid = 0.123_456;
        assert!((c.to_working([off_grid; 4])[0] - srgb_to_linear(off_grid)).abs() < 1e-7);
    }

    #[test]
    fn stored_space_is_identity() {
        let c = SpaceConverter::new(WorkingSpace::Stored);
        let p = [0.2, 0.4, 0.6, 0.8];
        assert_eq!(c.to_working(p), p);
        let mut px = [p];
        c.to_stored_all(&mut px);
        assert_eq!(px[0], p);
    }
}
