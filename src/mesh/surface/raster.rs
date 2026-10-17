//! Rasterising the set's triangles into texel space: which triangle each texel belongs to, where in it, and how
//! much of the texel the set covers.

use super::source::SurfaceTriangle;

/// "No triangle" in [`Texel::tri`].
pub(super) const NONE: u32 = u32::MAX;

/// Sub-samples per texel side used to measure coverage.
const SUB: usize = 4;

/// What one texel of the surface map refers to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Texel {
    /// Index into the map's triangles, or [`NONE`].
    pub tri: u32,
    /// Barycentrics of corners 1 and 2 at the texel centre (corner 0 has `1 - b1 - b2`). Outside 0..1 for texels
    /// whose centre lies off the triangle (partly covered edge texels and the gutter): the triangle's plane is then
    /// extended linearly.
    pub bary: [f32; 2],
    /// Share of the texel covered by the set, 0..1 (0 in the gutter).
    pub coverage: f32,
}

impl Default for Texel {
    fn default() -> Texel {
        Texel { tri: NONE, bary: [0.0; 2], coverage: 0.0 }
    }
}

/// A triangle in texel coordinates (x right, y down; texel `(x, y)` spans `x..x + 1`, `y..y + 1`).
#[derive(Debug, Clone, Copy)]
pub(super) struct UvTriangle {
    px: [[f32; 2]; 3],
    inv_den: f32,
}

impl UvTriangle {
    /// `None` when the triangle has no area in UV space.
    pub(super) fn new(uv: [[f32; 2]; 3], w: usize, h: usize) -> Option<UvTriangle> {
        let px = uv.map(|u| [u[0] * w as f32, u[1] * h as f32]);
        let den = (px[1][1] - px[2][1]) * (px[0][0] - px[2][0]) + (px[2][0] - px[1][0]) * (px[0][1] - px[2][1]);
        let ok = den.abs() > 1e-12 && px.iter().flatten().all(|v| v.is_finite());
        ok.then_some(UvTriangle { px, inv_den: 1.0 / den })
    }

    /// Barycentrics of the texel-space point `(x, y)`.
    pub(super) fn bary(&self, x: f32, y: f32) -> [f32; 3] {
        let p = &self.px;
        let l0 = ((p[1][1] - p[2][1]) * (x - p[2][0]) + (p[2][0] - p[1][0]) * (y - p[2][1])) * self.inv_den;
        let l1 = ((p[2][1] - p[0][1]) * (x - p[2][0]) + (p[0][0] - p[2][0]) * (y - p[2][1])) * self.inv_den;
        [l0, l1, 1.0 - l0 - l1]
    }

    /// The texel rows and columns the triangle touches, clipped to the image.
    fn bbox(&self, w: usize, h: usize) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
        let [a, b, c] = self.px;
        let lo = [a[0].min(b[0]).min(c[0]), a[1].min(b[1]).min(c[1])];
        let hi = [a[0].max(b[0]).max(c[0]), a[1].max(b[1]).max(c[1])];
        let clip = |a: f32, b: f32, n: usize| (a.floor().max(0.0) as usize).min(n)..(b.ceil().max(0.0) as usize).min(n);
        (clip(lo[0], hi[0], w), clip(lo[1], hi[1], h))
    }
}

/// Barycentric slack so texel centres exactly on a shared edge belong to a triangle.
const EDGE_EPS: f32 = 1e-5;

fn inside(b: [f32; 3]) -> bool {
    b.iter().all(|&l| l >= -EDGE_EPS)
}

/// Rasterises `tris` into a `w` x `h` texel grid. A texel belongs to the first triangle that contains its centre;
/// a partly covered texel whose centre no triangle contains belongs to the triangle covering most of it.
pub(super) fn rasterise(tris: &[SurfaceTriangle], w: usize, h: usize) -> Vec<Texel> {
    let mut texels = vec![Texel::default(); w * h];
    // per texel: 255 when the owner contains the centre, else the owner's sub-sample count
    let mut strength = vec![0u8; w * h];
    let step = 1.0 / SUB as f32;
    for (ti, tri) in tris.iter().enumerate() {
        let Some(uvt) = UvTriangle::new(tri.uv, w, h) else {
            continue;
        };
        let (xs, ys) = uvt.bbox(w, h);
        for y in ys {
            for x in xs.clone() {
                let i = y * w + x;
                let centre = uvt.bary(x as f32 + 0.5, y as f32 + 0.5);
                let mut hits = 0u8;
                for sy in 0..SUB {
                    for sx in 0..SUB {
                        let b = uvt.bary(x as f32 + (sx as f32 + 0.5) * step, y as f32 + (sy as f32 + 0.5) * step);
                        hits += u8::from(inside(b));
                    }
                }
                if hits == 0 && !inside(centre) {
                    continue;
                }
                let (Some(t), Some(s)) = (texels.get_mut(i), strength.get_mut(i)) else {
                    continue;
                };
                t.coverage = (t.coverage + f32::from(hits) / (SUB * SUB) as f32).min(1.0);
                let mine = if inside(centre) { u8::MAX } else { hits };
                if mine > *s {
                    *s = mine;
                    t.tri = ti as u32;
                    t.bary = [centre[1], centre[2]];
                }
            }
        }
    }
    // a texel whose centre is inside but no sub-sample is (a sliver) still counts as touched
    for (t, &s) in texels.iter_mut().zip(&strength) {
        if t.tri != NONE && t.coverage == 0.0 && s == u8::MAX {
            t.coverage = 1.0 / (SUB * SUB) as f32;
        }
    }
    texels
}
