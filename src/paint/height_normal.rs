//! Tangent-space normals from a height map, for mesh textures, the way Substance Painter derives them.
//!
//! Unlike [`crate::ops::normal_from_height`] (made for tiling material textures), this version
//! clamps at the image edges instead of wrapping, since the opposite edge of a mesh texture
//! belongs to an unrelated UV island, and it never depends on editor job state: the result is
//! always complete. It reads across UV island borders: the texels around an island hold Painter's
//! padding, which Painter reads too.
//!
//! Scale (Verified against Painter exports at 1K, 2K and 4K): the slope is [`UV_SLOPE`] × ∂h/∂u in
//! UV units, i.e. `width / 100` per height step between neighbouring texels. It does not depend on
//! the mesh size.

use super::par::for_each_mut;
use crate::tex::Tex;

/// Normal tilt per unit of height per UV unit at strength 1 (Painter's scale).
pub const UV_SLOPE: f32 = 0.01;

/// How the height gradient is estimated. A document records its filter, since Painter changed it
/// between releases ([`crate::paint::Document::height_filter`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeightFilter {
    /// 3×3 Sobel (`[1 2 1] ⊗ [-1 0 1] / 8`): Painter up to document format 61 (2021).
    Sobel,
    /// One-texel differences, `h(x) - h(x-1)` across and `h(row) - h(row above)` down the image:
    /// Painter from document format 64 (late 2021 on). Sharper than Sobel.
    #[default]
    Backward,
}

impl HeightFilter {
    /// Stable id used in files: `sobel` or `backward`.
    pub fn id(self) -> &'static str {
        match self {
            HeightFilter::Sobel => "sobel",
            HeightFilter::Backward => "backward",
        }
    }

    /// Parses an id; `None` for unknown ids.
    pub fn from_id(id: &str) -> Option<HeightFilter> {
        [HeightFilter::Sobel, HeightFilter::Backward].into_iter().find(|f| f.id() == id)
    }

    /// Height change per texel along image x and image y (rows top first, y down) at `(x, y)`;
    /// `g(dx, dy)` reads the height at an offset, clamped to the image.
    fn gradient(self, g: impl Fn(isize, isize) -> f32) -> (f32, f32) {
        match self {
            HeightFilter::Sobel => {
                let gx = g(1, -1) - g(-1, -1) + 2.0 * (g(1, 0) - g(-1, 0)) + g(1, 1) - g(-1, 1);
                let gy = g(-1, 1) - g(-1, -1) + 2.0 * (g(0, 1) - g(0, -1)) + g(1, 1) - g(1, -1);
                (gx / 8.0, gy / 8.0)
            }
            HeightFilter::Backward => (g(0, 0) - g(-1, 0), g(0, 0) - g(0, -1)),
        }
    }
}

/// The encoded (`n * 0.5 + 0.5`), OpenGL (+Y up) normal map of the height in `height`'s red
/// component, with Painter's scale times `strength`. Alpha is 1.
pub fn normal_from_height(height: &Tex, filter: HeightFilter, strength: f32) -> Tex {
    let (w, h) = (height.w, height.h);
    let mut out = Tex { w, h, px: vec![[0.5, 0.5, 1.0, 1.0]; w * h] };
    if w == 0 || h == 0 || height.px.len() < w * h {
        return out;
    }
    let slope = Slope::new(filter, strength, (w, h));
    let mut rows: Vec<&mut [[f32; 4]]> = out.px.chunks_mut(w).collect();
    for_each_mut(&mut rows, |y, row| {
        for (x, p) in row.iter_mut().enumerate() {
            *p = slope.normal_at(x, y, |xx, yy| height.px[yy * w + xx][0]);
        }
    });
    out
}

/// The height → normal conversion of one texture size, texel by texel: [`normal_from_height`] over the whole map, or
/// a part of it (a brush stroke's tiles) with the same values.
#[derive(Debug, Clone, Copy)]
pub struct Slope {
    filter: HeightFilter,
    kx: f32,
    ky: f32,
    w: usize,
    h: usize,
}

impl Slope {
    /// For a `size` map (both sides above 0) with Painter's scale times `strength`.
    pub fn new(filter: HeightFilter, strength: f32, size: (usize, usize)) -> Slope {
        let (w, h) = size;
        Slope { filter, kx: strength * UV_SLOPE * w as f32, ky: strength * UV_SLOPE * h as f32, w, h }
    }

    /// The encoded normal at texel `(x, y)`; `height(x, y)` reads the height of a texel of the map (the
    /// neighbours asked for are clamped to the map, so at most one texel beyond `(x, y)` on each side).
    pub fn normal_at(&self, x: usize, y: usize, height: impl Fn(usize, usize) -> f32) -> [f32; 4] {
        let (w, h) = (self.w.max(1), self.h.max(1));
        let at = |dx: isize, dy: isize| height(x.saturating_add_signed(dx).min(w - 1), y.saturating_add_signed(dy).min(h - 1));
        let (gx, gy) = self.filter.gradient(at);
        // Image y runs against +v, so a height rising down the image tilts the normal up (+Y).
        let (nx, ny, nz) = (-gx * self.kx, gy * self.ky, 1.0f32);
        let l = (nx * nx + ny * ny + nz * nz).sqrt();
        let n = if l.is_finite() && l > 0.0 { [nx / l, ny / l, nz / l] } else { [0.0, 0.0, 1.0] };
        [n[0] * 0.5 + 0.5, n[1] * 0.5 + 0.5, n[2] * 0.5 + 0.5, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILTERS: [HeightFilter; 2] = [HeightFilter::Sobel, HeightFilter::Backward];

    fn tex(w: usize, h: usize, f: impl Fn(usize, usize) -> f32) -> Tex {
        Tex {
            w,
            h,
            px: (0..w * h)
                .map(|i| {
                    let v = f(i % w, i / w);
                    [v, v, v, 1.0]
                })
                .collect(),
        }
    }

    #[test]
    fn flat_height_gives_flat_normal() {
        for f in FILTERS {
            let n = normal_from_height(&tex(5, 3, |_, _| 0.7), f, 1.0);
            assert!(n.px.iter().all(|p| *p == [0.5, 0.5, 1.0, 1.0]));
        }
    }

    #[test]
    fn ramp_slope_follows_painters_scale() {
        // A ramp of 0.01 per texel on a 200-wide map: slope 0.01 × 200 × 0.01 = 0.02.
        for f in FILTERS {
            let n = normal_from_height(&tex(200, 4, |x, _| x as f32 * 0.01), f, 1.0);
            let p = n.px[200 + 100];
            let (nx, nz) = (p[0] * 2.0 - 1.0, p[2] * 2.0 - 1.0);
            assert!((nx / nz + 0.02).abs() < 1e-4, "{f:?}: {}", nx / nz);
            assert!((p[1] - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn edges_clamp_instead_of_wrapping() {
        // A ramp in x: wrapping would see a huge jump at the left and right edges.
        let n = normal_from_height(&tex(8, 4, |x, _| x as f32 / 8.0), HeightFilter::Sobel, 50.0);
        let (interior, left, right) = (n.px[8 + 3], n.px[8], n.px[8 + 7]);
        assert!(interior[0] < 0.5, "slope tilts the normal towards -x");
        // Clamped edges see half the slope, never the opposite sign.
        assert!(left[0] < 0.5 && right[0] < 0.5);
        assert!((left[0] - right[0]).abs() < 1e-6);
        assert!(n.px.iter().all(|p| (p[1] - 0.5).abs() < 1e-6));
    }

    #[test]
    fn rising_downwards_tilts_up() {
        // Height grows with image y (towards -v); with +Y up the normal leans towards +Y.
        for f in FILTERS {
            let n = normal_from_height(&tex(4, 8, |_, y| y as f32 / 8.0), f, 1.0);
            assert!(n.px[4 * 4 + 1][1] > 0.5, "{f:?}");
        }
    }

    #[test]
    fn backward_reads_left_and_up_only() {
        // A single raised texel tilts its own normal (−x, +y) and its right/lower neighbours the other way.
        let n = normal_from_height(&tex(5, 5, |x, y| f32::from(x == 2 && y == 2)), HeightFilter::Backward, 1.0);
        assert!(n.px[2 * 5 + 2][0] < 0.5 && n.px[2 * 5 + 2][1] > 0.5);
        assert!(n.px[2 * 5 + 3][0] > 0.5 && n.px[3 * 5 + 2][1] < 0.5);
        assert_eq!(n.px[2 * 5 + 1], [0.5, 0.5, 1.0, 1.0]);
        assert_eq!(n.px[5 + 2], [0.5, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn ids_round_trip() {
        for f in FILTERS {
            assert_eq!(HeightFilter::from_id(f.id()), Some(f));
        }
        assert_eq!(HeightFilter::from_id("Sobel"), None);
    }

    #[test]
    fn empty_and_short_inputs_do_not_panic() {
        let empty = Tex { w: 0, h: 0, px: vec![] };
        assert!(normal_from_height(&empty, HeightFilter::Sobel, 1.0).px.is_empty());
        let short = Tex { w: 4, h: 4, px: vec![[1.0; 4]; 3] };
        assert_eq!(normal_from_height(&short, HeightFilter::Backward, 1.0).px.len(), 16);
    }
}
