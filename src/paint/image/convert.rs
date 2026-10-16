//! Bulk conversion of [`TiledImage`]s: raw bytes, float textures and other pixel formats.

use super::{decode_tile, encode_tile, tile_rect, ImageError, Rect, TiledImage, MAX_DIMENSION, T};
use crate::paint::par::for_each_mut;
use crate::paint::pixel::PixelFormat;
use crate::tex::Tex;

impl TiledImage {
    /// Reads row-major, little-endian pixel data. All-zero tiles stay unallocated.
    pub fn from_raw(
        width: u32,
        height: u32,
        format: PixelFormat,
        raw: &[u8],
    ) -> Result<TiledImage, ImageError> {
        let mut img = TiledImage::new(width, height, format)?;
        let bpp = format.bytes_per_pixel();
        let expected = width as usize * height as usize * bpp;
        if raw.len() != expected {
            return Err(ImageError::WrongDataSize {
                expected,
                found: raw.len(),
            });
        }
        let tiles_x = img.tiles_x();
        for_each_mut(&mut img.tiles, |i, tile| {
            let Some(r) = tile_rect(width, height, i as u32 % tiles_x, i as u32 / tiles_x) else {
                return;
            };
            let mut buf = vec![0u8; T * T * bpp];
            let row_bytes = r.w as usize * bpp;
            for row in 0..r.h as usize {
                let src = ((r.y as usize + row) * width as usize + r.x as usize) * bpp;
                buf[row * T * bpp..row * T * bpp + row_bytes]
                    .copy_from_slice(&raw[src..src + row_bytes]);
            }
            if buf.iter().any(|&b| b != 0) {
                *tile = Some(buf.into_boxed_slice());
            }
        });
        Ok(img)
    }

    /// Row-major, little-endian pixel data (`width * height * bytes_per_pixel` bytes).
    pub fn to_raw(&self) -> Vec<u8> {
        let bpp = self.format.bytes_per_pixel();
        let mut out = vec![0u8; self.width as usize * self.height as usize * bpp];
        let band_bytes = T * self.width as usize * bpp;
        if band_bytes == 0 {
            return out;
        }
        let mut bands: Vec<&mut [u8]> = out.chunks_mut(band_bytes).collect();
        for_each_mut(&mut bands, |ty, band| {
            for tx in 0..self.tiles_x() {
                let (Some(r), Some(Some(t))) = (
                    self.tile_rect(tx, ty as u32),
                    self.tiles.get(ty * self.tiles_x() as usize + tx as usize),
                ) else {
                    continue;
                };
                let row_bytes = r.w as usize * bpp;
                for row in 0..r.h as usize {
                    let dst = (row * self.width as usize + r.x as usize) * bpp;
                    band[dst..dst + row_bytes]
                        .copy_from_slice(&t[row * T * bpp..row * T * bpp + row_bytes]);
                }
            }
        });
        out
    }

    /// Converts a float RGBA texture, encoding it in `format`. All-zero tiles stay unallocated.
    pub fn from_tex(tex: &Tex, format: PixelFormat) -> Result<TiledImage, ImageError> {
        if tex.w > MAX_DIMENSION as usize || tex.h > MAX_DIMENSION as usize {
            return Err(ImageError::TooLarge {
                width: tex.w as u64,
                height: tex.h as u64,
            });
        }
        if tex.px.len() != tex.w * tex.h {
            return Err(ImageError::WrongDataSize {
                expected: tex.w * tex.h,
                found: tex.px.len(),
            });
        }
        let (w, h) = (tex.w as u32, tex.h as u32);
        let mut img = TiledImage::new(w, h, format)?;
        let tiles_x = img.tiles_x();
        for_each_mut(&mut img.tiles, |i, tile| {
            let Some(r) = tile_rect(w, h, i as u32 % tiles_x, i as u32 / tiles_x) else {
                return;
            };
            let mut buf = vec![0u8; T * T * format.bytes_per_pixel()];
            let mut px = Vec::with_capacity(r.area());
            for y in r.y..r.y + r.h {
                let s = y as usize * tex.w + r.x as usize;
                px.extend_from_slice(&tex.px[s..s + r.w as usize]);
            }
            encode_tile(format, r, &px, &mut buf);
            if buf.iter().any(|&b| b != 0) {
                *tile = Some(buf.into_boxed_slice());
            }
        });
        Ok(img)
    }

    /// Decodes the whole image into a float RGBA texture.
    pub fn to_tex(&self) -> Tex {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut px = vec![[0.0f32; 4]; w * h];
        if w > 0 {
            let mut bands: Vec<&mut [[f32; 4]]> = px.chunks_mut(T * w).collect();
            for_each_mut(&mut bands, |ty, band| {
                let r = Rect {
                    x: 0,
                    y: (ty * T) as u32,
                    w: w as u32,
                    h: (band.len() / w) as u32,
                };
                self.read_region(r, band);
            });
        }
        Tex { w, h, px }
    }

    /// The same pixels re-encoded in another format.
    pub fn converted(&self, format: PixelFormat) -> TiledImage {
        let mut out = TiledImage {
            width: self.width,
            height: self.height,
            format,
            tiles: vec![None; self.tiles.len()],
            dirty: self.dirty.clone(),
        };
        let tiles_x = self.tiles_x();
        for_each_mut(&mut out.tiles, |i, tile| {
            let Some(src) = &self.tiles[i] else { return };
            let Some(r) = self.tile_rect(i as u32 % tiles_x, i as u32 / tiles_x) else {
                return;
            };
            let mut px = vec![[0.0f32; 4]; r.area()];
            decode_tile(self.format, r, src, &mut px);
            let mut buf = vec![0u8; T * T * format.bytes_per_pixel()];
            encode_tile(format, r, &px, &mut buf);
            *tile = Some(buf.into_boxed_slice());
        });
        out
    }
}
