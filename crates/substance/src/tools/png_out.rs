//! [`PngMap`]: one float texture written as an 8- or 16-bit gray or RGB PNG.

use materialcraft::deflate::Level;
use materialcraft::png::{encode_raw, ColorType};
use materialcraft::tex::Tex;
use std::io;
use std::path::Path;

/// How to store a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PngMap {
    pub gray: bool,
    /// 8 or 16.
    pub bits: u8,
}

impl PngMap {
    /// PNG bytes of `tex` (values clamped to 0..1; gray maps store the red component).
    pub fn encode(self, tex: &Tex) -> Vec<u8> {
        let color = if self.gray { ColorType::Gray } else { ColorType::Rgb };
        let comps = color.channels();
        let mut samples = Vec::with_capacity(tex.px.len() * comps * (self.bits as usize / 8));
        for p in &tex.px {
            for &v in &p[..comps] {
                let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
                match self.bits {
                    16 => samples.extend_from_slice(&((v * 65535.0).round() as u16).to_be_bytes()),
                    _ => samples.push((v * 255.0).round() as u8),
                }
            }
        }
        encode_raw(tex.w, tex.h, color, self.bits, &samples, Level::Fast)
    }

    pub fn write(self, tex: &Tex, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.encode(tex))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_decoder() {
        let tex = Tex::solid(3, 2, [0.25, 0.5, 1.0, 1.0]);
        for (gray, bits) in [(true, 8), (false, 16)] {
            let png = PngMap { gray, bits }.encode(&tex);
            let raw = materialcraft::png::decode_raw(&png).unwrap();
            assert_eq!((raw.width, raw.height, raw.bit_depth), (3, 2, bits));
        }
    }
}
