//! [`UvMask`]: which texels lie inside the UV islands of one material slot (texel centre inside a triangle),
//! optionally shrunk so texels next to island borders, where Painter's export padding starts, are left out.

use materialcraft::mesh::Mesh;

pub struct UvMask {
    pub width: usize,
    pub height: usize,
    pub inside: Vec<bool>,
}

impl UvMask {
    /// Rasterises UV set `uv_set` of every part on material `slot` (UV origin top-left, as in our meshes).
    pub fn rasterise(mesh: &Mesh, slot: usize, uv_set: usize, width: usize, height: usize) -> UvMask {
        let mut mask = UvMask {
            width,
            height,
            inside: vec![false; width * height],
        };
        for part in mesh.parts.iter().filter(|p| p.material == slot) {
            let Some(uvs) = part.uv_sets.get(uv_set) else { continue };
            for tri in &part.triangles {
                let corner = |i: usize| {
                    uvs.get(tri[i] as usize)
                        .map(|uv| [uv[0] * width as f32, uv[1] * height as f32])
                };
                if let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) {
                    mask.fill_triangle(a, b, c);
                }
            }
        }
        mask
    }

    fn fill_triangle(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2]) {
        let area = edge(a, b, c);
        if area == 0.0 || !area.is_finite() {
            return;
        }
        let lo = |i: usize| a[i].min(b[i]).min(c[i]).floor().max(0.0) as usize;
        let hi = |i: usize, n: usize| (a[i].max(b[i]).max(c[i]).ceil().max(0.0) as usize).min(n);
        for y in lo(1)..hi(1, self.height) {
            for x in lo(0)..hi(0, self.width) {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let (w0, w1, w2) = (edge(b, c, p) / area, edge(c, a, p) / area, edge(a, b, p) / area);
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    self.inside[y * self.width + x] = true;
                }
            }
        }
    }

    /// Removes texels within `steps` texels (4-neighbourhood) of the outside.
    pub fn shrink(&mut self, steps: usize) {
        for _ in 0..steps {
            let prev = self.inside.clone();
            let (w, h) = (self.width, self.height);
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    let out = |xx: usize, yy: usize| !prev[yy * w + xx];
                    if prev[i]
                        && (x == 0
                            || y == 0
                            || x + 1 == w
                            || y + 1 == h
                            || out(x - 1, y)
                            || out(x + 1, y)
                            || out(x, y - 1)
                            || out(x, y + 1))
                    {
                        self.inside[i] = false;
                    }
                }
            }
        }
    }

    pub fn count(&self) -> usize {
        self.inside.iter().filter(|b| **b).count()
    }
}

fn edge(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::mesh::MeshPart;

    #[test]
    fn rasterises_and_shrinks_a_half_square() {
        let part = MeshPart {
            uv_sets: vec![vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]],
            triangles: vec![[0, 1, 2]],
            ..MeshPart::default()
        };
        let mesh = Mesh {
            materials: vec!["m".into()],
            parts: vec![part],
        };
        let mut mask = UvMask::rasterise(&mesh, 0, 0, 8, 8);
        assert_eq!(mask.count(), 36);
        assert!(mask.inside[0] && !mask.inside[63]);
        mask.shrink(1);
        assert!(!mask.inside[0] && mask.inside[9]);
        assert_eq!(UvMask::rasterise(&mesh, 1, 0, 8, 8).count(), 0);
    }
}
