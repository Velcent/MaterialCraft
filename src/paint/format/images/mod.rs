//! Image entries: integer formats as PNG (8 or 16 bits, colour type from the layout, lossless),
//! float formats as OpenEXR (HALF or FLOAT channels `Y`, `A`, `R`, `G`, `B`, ZIP compression).

mod exr;
mod png;
#[cfg(test)]
mod tests;

use crate::paint::{PixelFormat, TiledImage};

/// How an image is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Png,
    Exr,
}

impl Encoding {
    /// PNG for 8/16-bit integer samples, OpenEXR for float samples.
    pub fn for_format(format: PixelFormat) -> Encoding {
        if format.sample.is_float() { Encoding::Exr } else { Encoding::Png }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Encoding::Png => "png",
            Encoding::Exr => "exr",
        }
    }
}

/// Encodes `img` losslessly in the encoding of its format.
pub fn encode(img: &TiledImage) -> Result<Vec<u8>, String> {
    match Encoding::for_format(img.format()) {
        Encoding::Png => png::encode(img),
        Encoding::Exr => exr::encode(img),
    }
}

/// Decodes an image that must have exactly `format` and `size`. The size is checked before any
/// pixel memory is allocated.
pub fn decode(bytes: &[u8], format: PixelFormat, size: (u32, u32)) -> Result<TiledImage, String> {
    let raw = match Encoding::for_format(format) {
        Encoding::Png => png::decode(bytes, format, size)?,
        Encoding::Exr => exr::decode(bytes, format, size)?,
    };
    TiledImage::from_raw(size.0, size.1, format, &raw).map_err(|e| e.to_string())
}
