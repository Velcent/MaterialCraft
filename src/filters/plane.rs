//! [`Plane`]: a rectangle of RGBA texels placed at a texel position of the texture set (which may reach beyond
//! the texture: padded tiles do), with clamped and bilinear reads.

/// Texels of a rectangle, row by row. `x0`, `y0` is the texture-set texel of the first entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Plane {
    pub x0: i64,
    pub y0: i64,
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 4]>,
}

impl Plane {
    /// A plane of `w` x `h` texels at `(x0, y0)`, every texel `fill`.
    pub fn filled(x0: i64, y0: i64, w: usize, h: usize, fill: [f32; 4]) -> Plane {
        Plane { x0, y0, w, h, px: vec![fill; w.saturating_mul(h)] }
    }

    /// The same placement with other texels (as many as this plane has).
    pub fn with_px(&self, px: Vec<[f32; 4]>) -> Plane {
        Plane { x0: self.x0, y0: self.y0, w: self.w, h: self.h, px }
    }

    /// Texel `(x, y)` of the plane (local coordinates), clamped to its edges; zero for an empty plane.
    pub fn at(&self, x: i64, y: i64) -> [f32; 4] {
        if self.w == 0 || self.h == 0 {
            return [0.0; 4];
        }
        let x = x.clamp(0, self.w as i64 - 1) as usize;
        let y = y.clamp(0, self.h as i64 - 1) as usize;
        self.px.get(y * self.w + x).copied().unwrap_or([0.0; 4])
    }

    /// Bilinear read at local texel coordinates (texel centres at whole numbers), clamped to the edges.
    pub fn bilinear(&self, x: f32, y: f32) -> [f32; 4] {
        let (x, y) = (if x.is_finite() { x } else { 0.0 }, if y.is_finite() { y } else { 0.0 });
        let (fx0, fy0) = (x.floor(), y.floor());
        let (tx, ty) = (x - fx0, y - fy0);
        let (ix, iy) = (fx0 as i64, fy0 as i64);
        let (a, b, c, d) = (self.at(ix, iy), self.at(ix + 1, iy), self.at(ix, iy + 1), self.at(ix + 1, iy + 1));
        let lerp = |p: f32, q: f32, t: f32| p + (q - p) * t;
        std::array::from_fn(|k| {
            let at = |v: [f32; 4]| v.get(k).copied().unwrap_or(0.0);
            lerp(lerp(at(a), at(b), tx), lerp(at(c), at(d), tx), ty)
        })
    }

    /// UV centre of local texel `(x, y)` on a texture set of `size` texels.
    pub fn uv(&self, x: usize, y: usize, size: (u32, u32)) -> (f32, f32) {
        let u = ((self.x0 + x as i64) as f32 + 0.5) / size.0.max(1) as f32;
        let v = ((self.y0 + y as i64) as f32 + 0.5) / size.1.max(1) as f32;
        (u, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamped_and_bilinear_reads() {
        let p = Plane { x0: -1, y0: 0, w: 2, h: 1, px: vec![[0.0; 4], [1.0; 4]] };
        assert_eq!(p.at(-5, 3), [0.0; 4]);
        assert_eq!(p.at(9, 0), [1.0; 4]);
        assert_eq!(p.bilinear(0.25, 0.0)[0], 0.25);
        assert_eq!(p.uv(1, 0, (4, 4)), (0.125, 0.125));
        assert_eq!(Plane::filled(0, 0, 0, 0, [1.0; 4]).at(0, 0), [0.0; 4]);
    }
}
