//! Small image operations needed to put two images side by side: green flip and area resampling.

use materialcraft::tex::Tex;

/// OpenGL ↔ DirectX normal maps.
pub fn flip_green(tex: &mut Tex) {
    tex.px.iter_mut().for_each(|p| p[1] = 1.0 - p[1]);
}

/// `tex` at `w`×`h`: an area average when shrinking (each output texel averages the input texels whose
/// centres fall inside it), nearest texel when enlarging. Returns the input unchanged when sizes match.
pub fn resized(tex: Tex, w: usize, h: usize) -> Tex {
    if (tex.w, tex.h) == (w, h) || tex.w == 0 || tex.h == 0 || w == 0 || h == 0 {
        return tex;
    }
    let mut px = vec![[0.0f32; 4]; w * h];
    for (y, row) in px.chunks_mut(w).enumerate() {
        let (y0, y1) = span(y, h, tex.h);
        for (x, out) in row.iter_mut().enumerate() {
            let (x0, x1) = span(x, w, tex.w);
            let mut sum = [0.0f32; 4];
            for sy in y0..y1 {
                for p in &tex.px[sy * tex.w + x0..sy * tex.w + x1] {
                    (0..4).for_each(|c| sum[c] += p[c]);
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as f32;
            *out = sum.map(|s| s / n);
        }
    }
    Tex { w, h, px }
}

/// Input range `[lo, hi)` covered by output index `i` of `n` over `len` inputs (at least one texel).
fn span(i: usize, n: usize, len: usize) -> (usize, usize) {
    let lo = (i * len / n).min(len - 1);
    let hi = ((i + 1) * len / n).clamp(lo + 1, len);
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_when_shrinking_and_repeats_when_growing() {
        let tex = Tex {
            w: 4,
            h: 2,
            px: (0..8).map(|i| [i as f32, 0.0, 0.0, 1.0]).collect(),
        };
        let small = resized(tex.clone(), 2, 1);
        assert_eq!((small.w, small.h), (2, 1));
        assert_eq!(small.px[0][0], (0.0 + 1.0 + 4.0 + 5.0) / 4.0);
        let big = resized(tex.clone(), 8, 4);
        assert_eq!(big.px[0][0], 0.0);
        assert_eq!(big.px[8 * 4 - 1][0], 7.0);
        assert_eq!(resized(tex.clone(), 4, 2).px, tex.px);
        let mut g = Tex::solid(1, 1, [0.5, 0.25, 1.0, 1.0]);
        flip_green(&mut g);
        assert_eq!(g.px[0][1], 0.75);
    }
}
