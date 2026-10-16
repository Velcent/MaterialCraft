//! PNG entries: 8/16-bit integer samples, colour type from the layout, 16-bit big-endian.

use crate::deflate::Level;
use crate::paint::{PixelFormat, PixelLayout, SampleType, TiledImage};
use crate::png::{self, ColorType};

/// Largest width or height our PNG reader accepts.
const PNG_MAX_DIMENSION: u32 = 16384;
/// Largest decompressed PNG data our reader accepts.
const PNG_MAX_DATA: usize = 1 << 30;

fn color_type(layout: PixelLayout) -> ColorType {
    match layout {
        PixelLayout::Gray => ColorType::Gray,
        PixelLayout::GrayAlpha => ColorType::GrayAlpha,
        PixelLayout::Rgb => ColorType::Rgb,
        PixelLayout::Rgba => ColorType::Rgba,
    }
}

fn png_depth(sample: SampleType) -> u8 {
    if sample == SampleType::U16 {
        16
    } else {
        8
    }
}

/// Swaps the bytes of every 16-bit sample (little-endian model ↔ big-endian PNG).
fn swap16(raw: &mut [u8]) {
    raw.chunks_exact_mut(2).for_each(|s| s.swap(0, 1));
}

pub(super) fn encode(img: &TiledImage) -> Result<Vec<u8>, String> {
    let (w, h) = img.size();
    let f = img.format();
    let data = (w as usize * f.bytes_per_pixel() + 1) * h as usize;
    if w > PNG_MAX_DIMENSION || h > PNG_MAX_DIMENSION || data > PNG_MAX_DATA {
        return Err(format!(
            "{w}x{h} {} is too large for a PNG entry (at most {PNG_MAX_DIMENSION} pixels a side and 1 GiB of data)",
            f.id()
        ));
    }
    let mut raw = img.to_raw();
    if f.sample == SampleType::U16 {
        swap16(&mut raw);
    }
    let depth = png_depth(f.sample);
    Ok(png::encode_raw(
        w as usize,
        h as usize,
        color_type(f.layout),
        depth,
        &raw,
        Level::Fast,
    ))
}

pub(super) fn decode(
    bytes: &[u8],
    format: PixelFormat,
    size: (u32, u32),
) -> Result<Vec<u8>, String> {
    // IHDR is the first chunk: width, height, bit depth and colour type at fixed offsets.
    let ihdr = bytes
        .get(12..26)
        .filter(|h| &h[..4] == b"IHDR")
        .ok_or("not a PNG file")?;
    let be = |i: usize| u32::from_be_bytes([ihdr[i], ihdr[i + 1], ihdr[i + 2], ihdr[i + 3]]);
    if (be(4), be(8)) != size {
        return Err(format!(
            "PNG is {}x{}, expected {}x{}",
            be(4),
            be(8),
            size.0,
            size.1
        ));
    }
    let img = png::decode_raw(bytes)?;
    let (color, depth) = (color_type(format.layout), png_depth(format.sample));
    if img.color != color || img.bit_depth != depth {
        return Err(format!(
            "PNG is {:?} {}-bit, expected {:?} {depth}-bit for {}",
            img.color,
            img.bit_depth,
            color,
            format.id()
        ));
    }
    let mut raw = img.samples;
    if depth == 16 {
        swap16(&mut raw);
    }
    Ok(raw)
}
