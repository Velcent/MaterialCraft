//! [`Conversion`]: Painter pixel buffers → [`TiledImage`]s in our formats, with straight (not premultiplied)
//! alpha and rows top first.

use super::source_format::{Alpha, SourceFormat};
use materialcraft::paint::{ImageError, PixelFormat, TiledImage};
use std::thread;

/// Order of the rows in a source buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowOrder {
    TopFirst,
    /// OpenGL texture order: the first row is the bottom of the image.
    BottomFirst,
}

/// Coverage at or above this counts as full.
const FULL: f32 = 0.999;

/// A converted image and whether every texel is fully covered (alpha ≥ [`FULL`]).
pub struct Converted {
    pub image: TiledImage,
    pub fully_covered: bool,
}

/// Converts one buffer. Missing trailing bytes (a short LZ4 block) read as zero.
#[derive(Debug, Clone, Copy)]
pub struct Conversion {
    pub source: SourceFormat,
    pub rows: RowOrder,
}

impl Conversion {
    pub fn convert(self, width: u32, height: u32, raw: &[u8]) -> Result<Converted, ImageError> {
        let target = self.source.target();
        let (w, h) = (width as usize, height as usize);
        let out_row = w * target.bytes_per_pixel();
        let mut out = vec![0u8; out_row * h];
        let mut fully_covered = true;
        if out_row > 0 {
            let threads = thread::available_parallelism().map_or(1, |n| n.get());
            let rows_per_job = h.div_ceil(threads).max(1);
            fully_covered = thread::scope(|s| {
                let jobs: Vec<_> = out
                    .chunks_mut(rows_per_job * out_row)
                    .enumerate()
                    .map(|(job, chunk)| {
                        s.spawn(move || self.convert_rows(job * rows_per_job, w, h, raw, target, chunk))
                    })
                    .collect();
                jobs.into_iter().all(|j| j.join().unwrap_or(false))
            });
        }
        Ok(Converted {
            image: TiledImage::from_raw(width, height, target, &out)?,
            fully_covered,
        })
    }

    /// Fills `out` (whole output rows starting at row `first`); true if every texel is fully covered.
    fn convert_rows(&self, first: usize, w: usize, h: usize, raw: &[u8], target: PixelFormat, out: &mut [u8]) -> bool {
        let mut covered = true;
        let in_bpp = self.source.bytes_per_pixel();
        let out_bpp = target.bytes_per_pixel();
        let mut pixel = vec![0u8; in_bpp];
        for (i, out_row) in out.chunks_mut(w * out_bpp).enumerate() {
            let y = first + i;
            let src_y = match self.rows {
                RowOrder::TopFirst => y,
                RowOrder::BottomFirst => h - 1 - y,
            };
            for x in 0..w {
                let at = (src_y * w + x) * in_bpp;
                let src: &[u8] = match raw.get(at..at + in_bpp) {
                    Some(src) => src,
                    None => {
                        let have = raw.get(at..).unwrap_or(&[]);
                        let n = have.len().min(in_bpp);
                        pixel.fill(0);
                        pixel[..n].copy_from_slice(&have[..n]);
                        &pixel
                    }
                };
                let straight = unpremultiply(self.source.decode(src), self.source.alpha());
                covered &= straight[3] >= FULL;
                target.encode(straight, &mut out_row[x * out_bpp..]);
            }
        }
        covered
    }
}

/// Straight colour from a source pixel; uncovered pixels become all zero.
pub fn unpremultiply(p: [f32; 4], alpha: Alpha) -> [f32; 4] {
    let a = p[3];
    if alpha == Alpha::Straight {
        return p;
    }
    if a.is_nan() || a <= 0.0 {
        return [0.0; 4];
    }
    match alpha {
        Alpha::Premultiplied => [p[0] / a, p[1] / a, p[2] / a, a],
        _ => {
            let c = |v: f32| linear_to_srgb((srgb_to_linear(v) / a).min(1.0));
            [c(p[0]), c(p[1]), c(p[2]), a]
        }
    }
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpremultiplies_and_flips_rows() {
        // 1×2 RG8 image, bottom row first: bottom (128, 128) = full coverage 0.5, top (64, 128) = 0.5 at half coverage.
        let raw = [128, 255, 64, 128];
        let c = Conversion {
            source: SourceFormat::CacheRg8,
            rows: RowOrder::BottomFirst,
        };
        let converted = c.convert(1, 2, &raw).unwrap();
        assert!(!converted.fully_covered);
        let img = converted.image;
        let top = img.get(0, 0);
        assert!((top[0] - 0.5).abs() < 0.01 && (top[3] - 0.5).abs() < 0.01, "{top:?}");
        assert!((img.get(0, 1)[0] - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn srgb_unpremultiplies_in_linear_light() {
        let half_lin = linear_to_srgb(0.5 * srgb_to_linear(0.8));
        let p = unpremultiply([half_lin, 0.0, 0.0, 0.5], Alpha::PremultipliedLinearSrgb);
        assert!((p[0] - 0.8).abs() < 1e-4);
        assert_eq!(unpremultiply([0.3, 0.3, 0.3, 0.0], Alpha::Premultiplied), [0.0; 4]);
        assert_eq!(
            unpremultiply([0.3, 0.3, 0.3, 0.0], Alpha::Straight),
            [0.3, 0.3, 0.3, 0.0]
        );
    }

    #[test]
    fn short_buffers_read_as_zero() {
        let c = Conversion {
            source: SourceFormat::Lum16,
            rows: RowOrder::TopFirst,
        };
        let img = c.convert(2, 2, &[0xff, 0xff, 0xff]).unwrap().image;
        assert_eq!(img.get(0, 0)[0], 1.0);
        assert!((img.get(1, 0)[0] - 255.0 / 65535.0).abs() < 1e-7);
        assert_eq!(img.get(1, 1)[0], 0.0);
    }
}
