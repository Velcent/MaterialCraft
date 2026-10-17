//! [`Pyramid`]: a grey image and successively halved, smoothed copies of it, so a value blurred by any radius
//! can be read at any point in constant time (the two nearest levels, bilinear, blended). Generators use it for
//! their blur and smoothness sliders and for multi-scale curvature.
//!
//! Level 0 is the image; level `k` is level `k − 1` smoothed with a 1-2-1 kernel and halved, so its texels
//! average about `2^k` source texels. Edges clamp (UV islands are not tileable).

/// One level: `w × h` values, rows top first.
struct Level {
    w: usize,
    h: usize,
    px: Vec<f32>,
}

impl Level {
    fn at(&self, x: isize, y: isize) -> f32 {
        let x = x.clamp(0, self.w as isize - 1) as usize;
        let y = y.clamp(0, self.h as isize - 1) as usize;
        self.px.get(y * self.w + x).copied().unwrap_or(0.0)
    }

    /// Bilinear at `u, v` in 0..1 (texel centres at `(i + 0.5) / w`).
    fn sample(&self, u: f32, v: f32) -> f32 {
        let x = u * self.w as f32 - 0.5;
        let y = v * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as isize, y0 as isize);
        let top = self.at(x0, y0) * (1.0 - fx) + self.at(x0 + 1, y0) * fx;
        let bottom = self.at(x0, y0 + 1) * (1.0 - fx) + self.at(x0 + 1, y0 + 1) * fx;
        top * (1.0 - fy) + bottom * fy
    }

    /// Smoothed (1-2-1 in both directions) and halved.
    fn down(&self) -> Level {
        let (w, h) = (self.w.div_ceil(2).max(1), self.h.div_ceil(2).max(1));
        let mut px = vec![0.0; w * h];
        for (y, row) in px.chunks_exact_mut(w).enumerate() {
            for (x, p) in row.iter_mut().enumerate() {
                let (sx, sy) = (2 * x as isize, 2 * y as isize);
                let mut sum = 0.0;
                for (dy, wy) in [(-1isize, 1.0f32), (0, 2.0), (1, 2.0), (2, 1.0)] {
                    for (dx, wx) in [(-1isize, 1.0f32), (0, 2.0), (1, 2.0), (2, 1.0)] {
                        sum += self.at(sx + dx, sy + dy) * wx * wy;
                    }
                }
                *p = sum / 36.0;
            }
        }
        Level { w, h, px }
    }
}

pub struct Pyramid {
    levels: Vec<Level>,
}

impl Pyramid {
    /// A pyramid of `px` (`w × h`, rows top first) down to a few texels.
    pub fn new(w: usize, h: usize, px: Vec<f32>) -> Pyramid {
        let mut levels = vec![Level { w: w.max(1), h: h.max(1), px }];
        while let Some(last) = levels.last() {
            if last.w <= 4 && last.h <= 4 || levels.len() >= 14 {
                break;
            }
            let next = last.down();
            levels.push(next);
        }
        Pyramid { levels }
    }

    /// Width of the full-resolution level.
    pub fn width(&self) -> usize {
        self.levels.first().map_or(1, |l| l.w)
    }

    /// The value at `u, v` blurred by about `radius` texels of the full-resolution level (0 = sharp).
    pub fn sample(&self, u: f32, v: f32, radius: f32) -> f32 {
        let level = if radius > 1.0 { radius.log2() } else { 0.0 };
        let max = (self.levels.len() - 1) as f32;
        let level = level.clamp(0.0, max);
        let (l0, f) = (level.floor() as usize, level.fract());
        let a = self.levels.get(l0).map_or(0.0, |l| l.sample(u, v));
        if f <= 1e-4 {
            return a;
        }
        let b = self.levels.get(l0 + 1).map_or(a, |l| l.sample(u, v));
        a + (b - a) * f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharp_reads_texels_and_blur_averages() {
        let (w, h) = (64, 64);
        let px: Vec<f32> = (0..w * h).map(|i| if (i % w) < 32 { 0.0 } else { 1.0 }).collect();
        let p = Pyramid::new(w, h, px);
        assert!(p.levels.len() >= 5);
        assert_eq!(p.sample(0.25, 0.5, 0.0), 0.0);
        assert_eq!(p.sample(0.75, 0.5, 0.0), 1.0);
        // At the edge between the halves a wide blur gives about the average.
        let edge = p.sample(0.5, 0.5, 8.0);
        assert!((edge - 0.5).abs() < 0.15, "{edge}");
        // Far from the edge a small blur keeps the value.
        assert!(p.sample(0.05, 0.5, 2.0) < 0.05);
        assert_eq!(p.width(), 64);
    }
}
