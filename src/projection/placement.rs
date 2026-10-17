//! [`UvPlacement`]: an [`ImagePlacement`] as an affine map from projection coordinates (0…1, centre 0.5) to image
//! coordinates.
//!
//! Convention (the node editor's Transform: rotation and tiling about the centre): a point `p` is moved by
//! `-offset`, rotated by `-rotation` about the centre and scaled by `tiling`, so a tiling of 2 repeats the image
//! twice, a positive offset moves the image towards +u/+v and a positive rotation turns it counter-clockwise.

use super::data::ImagePlacement;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvPlacement {
    /// Row-major 2×3: `image = m · (p − centre) + centre`, with the offset folded into the last column.
    m: [[f32; 3]; 2],
    /// Image units per projection unit (the larger axis), for footprints.
    scale: f32,
}

impl UvPlacement {
    pub fn new(p: &ImagePlacement) -> UvPlacement {
        let finite = |v: f32, d: f32| if v.is_finite() { v } else { d };
        let (tu, tv) = (finite(p.tiling[0], 1.0), finite(p.tiling[1], 1.0));
        let a = finite(p.rotation, 0.0).to_radians();
        let (s, c) = a.sin_cos();
        // rotate by −a, then scale per image axis
        let r = [[c * tu, s * tu], [-s * tv, c * tv]];
        let (ox, oy) = (finite(p.offset[0], 0.0), finite(p.offset[1], 0.0));
        let t = |row: [f32; 2]| -(row[0] * ox + row[1] * oy);
        let m = [[r[0][0], r[0][1], t(r[0])], [r[1][0], r[1][1], t(r[1])]];
        UvPlacement { m, scale: tu.abs().max(tv.abs()) }
    }

    /// The identity placement.
    pub fn identity() -> UvPlacement {
        UvPlacement::new(&ImagePlacement::default())
    }

    /// Image coordinate of projection coordinate `p`.
    #[inline]
    pub fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        let (x, y) = (p[0] - 0.5, p[1] - 0.5);
        let [a, b] = self.m;
        [a[0] * x + a[1] * y + a[2] + 0.5, b[0] * x + b[1] * y + b[2] + 0.5]
    }

    /// How many image units one projection unit spans (the larger axis).
    pub fn scale(&self) -> f32 {
        self.scale
    }
}
