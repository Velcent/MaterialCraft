//! PNG encoding of what the recipes make: 16-bit grey images for textures and alphas, 8-bit RGB thumbnails.

use materialcraft::deflate::Level;
use materialcraft::png::{self, ColorType};
use materialcraft::tex::Tex;

/// Side of every thumbnail, as the tray shows them.
pub const THUMB_PX: usize = 128;

/// A 16-bit grey PNG of `values` (0..1, row by row, `w` × `h`).
pub fn grey16(w: usize, h: usize, values: &[f32]) -> Vec<u8> {
    let samples: Vec<u8> = values.iter().flat_map(|v| ((v.clamp(0.0, 1.0) * 65535.0).round() as u16).to_be_bytes()).collect();
    png::encode_raw(w, h, ColorType::Gray, 16, &samples, Level::Default)
}

/// An 8-bit RGB PNG of `t`'s display values (alpha dropped).
pub fn rgb8(t: &Tex) -> Vec<u8> {
    let samples: Vec<u8> = t.px.iter().flat_map(|p| [byte(p[0]), byte(p[1]), byte(p[2])]).collect();
    png::encode_raw(t.w, t.h, ColorType::Rgb, 8, &samples, Level::Default)
}

/// A grey thumbnail of `f(u, v)` (u, v in 0..1 at texel centres), [`THUMB_PX`] square.
pub fn grey_thumb(f: impl Fn(f32, f32) -> f32) -> Vec<u8> {
    let n = THUMB_PX;
    let values: Vec<f32> = (0..n * n).map(|i| f(((i % n) as f32 + 0.5) / n as f32, ((i / n) as f32 + 0.5) / n as f32)).collect();
    let samples: Vec<u8> = values.iter().map(|v| byte(*v)).collect();
    png::encode_raw(n, n, ColorType::Gray, 8, &samples, Level::Default)
}

fn byte(v: f32) -> u8 {
    if v.is_finite() { (v.clamp(0.0, 1.0) * 255.0).round() as u8 } else { 0 }
}
