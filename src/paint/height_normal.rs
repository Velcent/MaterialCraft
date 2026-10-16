//! Tangent-space normals from a height map, for mesh textures.
//!
//! Unlike [`crate::ops::normal_from_height`] (made for tiling material textures), this version
//! clamps at the image edges instead of wrapping, since the opposite edge of a mesh texture
//! belongs to an unrelated UV island, and it never depends on editor job state: the result is
//! always complete.

use super::par::for_each_mut;
use crate::tex::Tex;

/// The encoded (`n * 0.5 + 0.5`), OpenGL (+Y up) normal map of the height in `height`'s red
/// component. The slope is scaled by `strength * width * 0.05`, as in
/// [`crate::ops::normal_from_height`], and estimated with a 3×3 Scharr kernel. Alpha is 1.
pub fn normal_from_height(height: &Tex, strength: f32) -> Tex {
    let (w, h) = (height.w, height.h);
    let mut out = Tex {
        w,
        h,
        px: vec![[0.5, 0.5, 1.0, 1.0]; w * h],
    };
    if w == 0 || h == 0 || height.px.len() < w * h {
        return out;
    }
    let k = strength * w as f32 * 0.05;
    let at = |x: usize, y: usize, dx: isize, dy: isize| {
        let xx = x.saturating_add_signed(dx).min(w - 1);
        let yy = y.saturating_add_signed(dy).min(h - 1);
        height.px[yy * w + xx][0]
    };
    let mut rows: Vec<&mut [[f32; 4]]> = out.px.chunks_mut(w).collect();
    for_each_mut(&mut rows, |y, row| {
        for (x, p) in row.iter_mut().enumerate() {
            let g = |dx, dy| at(x, y, dx, dy);
            let sx = 3.0 * (g(1, -1) - g(-1, -1))
                + 10.0 * (g(1, 0) - g(-1, 0))
                + 3.0 * (g(1, 1) - g(-1, 1));
            let sy = 3.0 * (g(-1, 1) - g(-1, -1))
                + 10.0 * (g(0, 1) - g(0, -1))
                + 3.0 * (g(1, 1) - g(1, -1));
            let (nx, ny, nz) = (-sx / 32.0 * k, sy / 32.0 * k, 1.0f32);
            let l = (nx * nx + ny * ny + nz * nz).sqrt();
            let n = if l.is_finite() && l > 0.0 {
                [nx / l, ny / l, nz / l]
            } else {
                [0.0, 0.0, 1.0]
            };
            *p = [n[0] * 0.5 + 0.5, n[1] * 0.5 + 0.5, n[2] * 0.5 + 0.5, 1.0];
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let n = normal_from_height(&tex(5, 3, |_, _| 0.7), 1.0);
        assert!(n.px.iter().all(|p| *p == [0.5, 0.5, 1.0, 1.0]));
    }

    #[test]
    fn edges_clamp_instead_of_wrapping() {
        // A ramp in x: wrapping would see a huge jump at the left and right edges.
        let n = normal_from_height(&tex(8, 4, |x, _| x as f32 / 8.0), 1.0);
        let interior = n.px[8 + 3];
        let left = n.px[8];
        let right = n.px[8 + 7];
        assert!(interior[0] < 0.5, "slope tilts the normal towards -x");
        // Clamped edges see half the slope, never the opposite sign.
        assert!(left[0] < 0.5 && right[0] < 0.5);
        assert!((left[0] - right[0]).abs() < 1e-6);
        assert!(n.px.iter().all(|p| (p[1] - 0.5).abs() < 1e-6));
    }

    #[test]
    fn rising_downwards_tilts_up() {
        // Height grows with image y (towards -v); with +Y up the normal leans towards +Y.
        let n = normal_from_height(&tex(4, 8, |_, y| y as f32 / 8.0), 1.0);
        assert!(n.px[4 * 4 + 1][1] > 0.5);
    }

    #[test]
    fn matches_ops_in_the_interior() {
        let t = tex(16, 16, |x, y| ((x * 7 + y * 3) % 11) as f32 / 11.0);
        let a = normal_from_height(&t, 0.5);
        let b = crate::ops::normal_from_height(&t, 0.5, false);
        for y in 1..15 {
            for x in 1..15 {
                let (p, q) = (a.px[y * 16 + x], b.px[y * 16 + x]);
                assert!((0..3).all(|c| (p[c] - q[c]).abs() < 1e-5), "{x},{y}");
            }
        }
    }

    #[test]
    fn empty_and_short_inputs_do_not_panic() {
        let empty = Tex {
            w: 0,
            h: 0,
            px: vec![],
        };
        assert!(normal_from_height(&empty, 1.0).px.is_empty());
        let short = Tex {
            w: 4,
            h: 4,
            px: vec![[1.0; 4]; 3],
        };
        assert_eq!(normal_from_height(&short, 1.0).px.len(), 16);
    }
}
