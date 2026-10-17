//! Loading an exported texture as a float image, and resampling it to our resolution.

use materialcraft::png::{ColorType, decode_raw};
use materialcraft::tex::Tex;
use std::path::Path;

/// Decodes an 8- or 16-bit PNG to 0..1 RGBA (gray in R, G and B). Other formats are not read.
pub fn load_export(path: &Path) -> Result<Tex, String> {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if ext.as_deref() != Some("png") {
        return Err(format!(
            "{} files are skipped (only PNG is read)",
            ext.unwrap_or_default()
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let raw = decode_raw(&bytes)?;
    let comps = raw.color.channels();
    let wide = raw.bit_depth == 16;
    let sample = |i: usize| match wide {
        true => u16::from_be_bytes([raw.samples[2 * i], raw.samples[2 * i + 1]]) as f32 / 65535.0,
        false => raw.samples[i] as f32 / 255.0,
    };
    let px = (0..raw.width * raw.height)
        .map(|p| {
            let s = |c: usize| sample(p * comps + c);
            match raw.color {
                ColorType::Gray => [s(0), s(0), s(0), 1.0],
                ColorType::GrayAlpha => [s(0), s(0), s(0), s(1)],
                ColorType::Rgb => [s(0), s(1), s(2), 1.0],
                ColorType::Rgba => [s(0), s(1), s(2), s(3)],
            }
        })
        .collect();
    Ok(Tex {
        w: raw.width,
        h: raw.height,
        px,
    })
}

/// `tex` at `w × h` (bilinear), or unchanged if it already has that size.
pub fn resampled(tex: Tex, w: usize, h: usize) -> Tex {
    if (tex.w, tex.h) == (w, h) || tex.px.is_empty() {
        return tex;
    }
    let px = (0..w * h)
        .map(|i| tex.sample(((i % w) as f32 + 0.5) / w as f32, ((i / w) as f32 + 0.5) / h as f32))
        .collect();
    Tex { w, h, px }
}

/// The same image upside down (to test the other row order).
pub fn flipped(tex: &Tex) -> Tex {
    let px = tex.px.chunks(tex.w.max(1)).rev().flatten().copied().collect();
    Tex { w: tex.w, h: tex.h, px }
}
