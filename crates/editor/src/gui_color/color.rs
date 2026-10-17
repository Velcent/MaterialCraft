//! [`FieldColor`]: a colour as a colour field stores it, and the conversions the picker needs: sRGB ↔ linear (the
//! only colour management for now, ADR 0008), HSV, and the sRGB hex code (CP-04).

use eframe::egui::Color32;
use materialcraft::paint::ColorSpace;
use materialcraft::paint::working_space::{linear_to_srgb, srgb_to_linear};

/// RGBA in the encoding of the field that holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldColor {
    pub rgba: [f32; 4],
    /// How `rgba`'s RGB is encoded (alpha never is).
    pub space: ColorSpace,
}

impl FieldColor {
    pub fn new(rgba: [f32; 4], space: ColorSpace) -> FieldColor {
        FieldColor { rgba, space }
    }

    pub fn rgb(&self) -> [f32; 3] {
        [self.rgba[0], self.rgba[1], self.rgba[2]]
    }

    /// The same colour with RGB replaced by `rgb` given in `from`, converted to this colour's space (exact when the
    /// spaces match); alpha is kept.
    pub fn with_rgb_in(&self, rgb: [f32; 3], from: ColorSpace) -> FieldColor {
        let [r, g, b] = convert(rgb, from, self.space);
        FieldColor { rgba: [r, g, b, self.rgba[3]], space: self.space }
    }

    /// Display-encoded (sRGB) RGB: what sliders, the spectrum and the hex code show.
    pub fn srgb(&self) -> [f32; 3] {
        convert(self.rgb(), self.space, ColorSpace::Srgb)
    }

    /// The colour from display-encoded RGB.
    pub fn with_srgb(&self, srgb: [f32; 3]) -> FieldColor {
        self.with_rgb_in(srgb, ColorSpace::Srgb)
    }

    /// The sRGB hex code (`#RRGGBB`), whatever the field's space (CP-04).
    pub fn hex(&self) -> String {
        hex(self.srgb())
    }

    /// The colour of a hex code, or `None` if `text` is not one.
    pub fn with_hex(&self, text: &str) -> Option<FieldColor> {
        let [r, g, b] = parse_hex(text)?;
        Some(self.with_srgb([r, g, b].map(|v| f32::from(v) / 255.0)))
    }
}

/// `rgb` given in `from`, encoded for `to`.
pub fn convert(rgb: [f32; 3], from: ColorSpace, to: ColorSpace) -> [f32; 3] {
    match (from, to) {
        (ColorSpace::Srgb, ColorSpace::Linear) => rgb.map(srgb_to_linear),
        (ColorSpace::Linear, ColorSpace::Srgb) => rgb.map(linear_to_srgb),
        _ => rgb,
    }
}

/// One display-encoded component as a byte (clamped, rounded).
pub fn to_byte(v: f32) -> u8 {
    if v.is_nan() { 0 } else { (v.clamp(0.0, 1.0) * 255.0).round() as u8 }
}

/// Display-encoded RGB as an opaque egui colour.
pub fn srgb_color32(srgb: [f32; 3]) -> Color32 {
    let [r, g, b] = srgb.map(to_byte);
    Color32::from_rgb(r, g, b)
}

/// `#RRGGBB` of display-encoded RGB.
pub fn hex(srgb: [f32; 3]) -> String {
    let [r, g, b] = srgb.map(to_byte);
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// The bytes of a hex code: six digits (`#` optional, either case) or the three-digit short form.
pub fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let t = text.trim();
    let t = t.strip_prefix('#').unwrap_or(t);
    if !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize| t.get(i..i + 1).and_then(|d| u8::from_str_radix(d, 16).ok());
    match t.len() {
        6 => {
            let byte = |i: usize| t.get(i..i + 2).and_then(|d| u8::from_str_radix(d, 16).ok());
            Some([byte(0)?, byte(2)?, byte(4)?])
        }
        3 => Some([digit(0)? * 17, digit(1)? * 17, digit(2)? * 17]),
        _ => None,
    }
}

/// Hue (0…1), saturation and value of RGB (each 0…1).
pub fn hsv_of(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let s = if max > 0.0 { d / max } else { 0.0 };
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    [h, s, max]
}

/// RGB of hue (0…1, wrapping), saturation and value.
pub fn rgb_of(hsv: [f32; 3]) -> [f32; 3] {
    let [h, s, v] = hsv;
    let h6 = (h.rem_euclid(1.0) * 6.0).clamp(0.0, 6.0);
    let c = v * s;
    let x = c * (1.0 - (h6.rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every 24-bit colour survives hex → field → hex, in an sRGB field and in a linear one: all 2^24 through the
    /// conversions, a dense sample (every 251st code) also through the text.
    #[test]
    fn hex_round_trip_is_exact_for_all_24_bit_colours() {
        let srgb = FieldColor::new([0.0, 0.0, 0.0, 1.0], ColorSpace::Srgb);
        let linear = FieldColor::new([0.0, 0.0, 0.0, 1.0], ColorSpace::Linear);
        for code in 0u32..1 << 24 {
            let bytes = [(code >> 16) as u8, (code >> 8) as u8, code as u8];
            let unit = bytes.map(|v| f32::from(v) / 255.0);
            for field in [srgb, linear] {
                assert_eq!(field.with_srgb(unit).srgb().map(to_byte), bytes, "{code:06X} in {:?}", field.space);
            }
            if code % 251 == 0 {
                let text = format!("#{code:06X}");
                for field in [srgb, linear] {
                    assert_eq!(field.with_hex(&text).map(|c| c.hex()), Some(text.clone()));
                }
            }
        }
    }

    #[test]
    fn hex_parsing_takes_both_forms_and_rejects_the_rest() {
        assert_eq!(parse_hex("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex(" FF8000 "), Some([255, 128, 0]));
        assert_eq!(parse_hex("#f80"), Some([255, 136, 0]));
        for bad in ["", "#", "#12345", "#1234567", "zzzzzz", "#12 456", "#ÿÿÿ"] {
            assert_eq!(parse_hex(bad), None, "{bad:?}");
        }
        assert_eq!(hex([1.0, 0.5, 0.0]), "#FF8000");
        assert_eq!(hex([2.0, -1.0, f32::NAN]), "#FF0000", "clamped");
    }

    #[test]
    fn hsv_round_trips_and_keeps_greys_hueless() {
        for code in (0u32..1 << 24).step_by(997) {
            let rgb = [(code >> 16) as u8, (code >> 8) as u8, code as u8].map(|v| f32::from(v) / 255.0);
            let back = rgb_of(hsv_of(rgb));
            assert!(rgb.iter().zip(back).all(|(a, b)| (a - b).abs() < 1e-5), "{rgb:?} {back:?}");
        }
        assert_eq!(hsv_of([0.5, 0.5, 0.5]), [0.0, 0.0, 0.5]);
        assert_eq!(rgb_of([1.0, 1.0, 1.0]), [1.0, 0.0, 0.0], "hue wraps");
    }

    #[test]
    fn picks_convert_between_spaces_and_stay_exact_within_one() {
        let lin = FieldColor::new([0.1, 0.2, 0.3, 0.5], ColorSpace::Linear);
        assert_eq!(lin.with_rgb_in([0.25, 0.5, 0.75], ColorSpace::Linear).rgba, [0.25, 0.5, 0.75, 0.5]);
        let from_srgb = lin.with_rgb_in([0.5, 0.5, 0.5], ColorSpace::Srgb);
        assert!((from_srgb.rgba[0] - 0.214_041_14).abs() < 1e-6);
        assert_eq!(FieldColor::new([0.5; 4], ColorSpace::Srgb).hex(), "#808080");
    }
}
