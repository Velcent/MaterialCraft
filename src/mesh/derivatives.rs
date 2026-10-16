//! How position changes with UV across one triangle: the basis that tangents and UV scale are built from.

use super::Vec3;
use crate::v3::{cross, len, mul, sub};

/// Surface derivatives of one triangle with respect to its UVs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriangleFrame {
    /// Object-space change of position per unit of u.
    pub dp_du: Vec3,
    /// Object-space change of position per unit of v (v grows downwards in the texture, see the module docs).
    pub dp_dv: Vec3,
    /// Object-space area of the triangle.
    pub area: f32,
}

impl TriangleFrame {
    /// The derivatives of the triangle `p` with UVs `uv`, or `None` when the UVs are degenerate
    /// (collinear or coincident), in which case no tangent direction can be derived from them.
    pub fn new(p: [Vec3; 3], uv: [[f32; 2]; 3]) -> Option<TriangleFrame> {
        let (e1, e2) = (sub(p[1], p[0]), sub(p[2], p[0]));
        let (du1, dv1) = (uv[1][0] - uv[0][0], uv[1][1] - uv[0][1]);
        let (du2, dv2) = (uv[2][0] - uv[0][0], uv[2][1] - uv[0][1]);
        let det = du1 * dv2 - du2 * dv1;
        // relative threshold: the UV triangle must not be a sliver compared with its edge lengths
        let scale = (du1.abs() + dv1.abs()) * (du2.abs() + dv2.abs());
        if det.abs() <= 1e-7 * scale || !det.is_finite() {
            return None;
        }
        let r = 1.0 / det;
        let dp_du = mul(sub(mul(e1, dv2), mul(e2, dv1)), r);
        let dp_dv = mul(sub(mul(e2, du1), mul(e1, du2)), r);
        let area = 0.5 * len(cross(e1, e2));
        let finite = dp_du.iter().chain(&dp_dv).all(|x| x.is_finite()) && area.is_finite();
        finite.then_some(TriangleFrame { dp_du, dp_dv, area })
    }

    /// The frame of triangle `tri`, or `None` when an index is out of range or the UVs are degenerate.
    pub fn of_triangle(
        positions: &[Vec3],
        uvs: &[[f32; 2]],
        tri: [u32; 3],
    ) -> Option<TriangleFrame> {
        let i = tri.map(|i| i as usize);
        let p = [
            *positions.get(i[0])?,
            *positions.get(i[1])?,
            *positions.get(i[2])?,
        ];
        let uv = [*uvs.get(i[0])?, *uvs.get(i[1])?, *uvs.get(i[2])?];
        TriangleFrame::new(p, uv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::test_util::{close, quad};

    #[test]
    fn quad_derivatives() {
        let q = quad();
        let f = TriangleFrame::of_triangle(&q.positions, &q.uv_sets[0], q.triangles[0]).unwrap();
        assert!(close(&f.dp_du, &[2.0, 0.0, 0.0], 1e-6));
        assert!(close(&f.dp_dv, &[0.0, -2.0, 0.0], 1e-6));
        assert!((f.area - 2.0).abs() < 1e-6);
    }

    #[test]
    fn degenerate_uvs_have_no_frame() {
        let p = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(TriangleFrame::new(p, [[0.5, 0.5]; 3]), None);
        assert_eq!(
            TriangleFrame::new(p, [[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]]),
            None
        );
        assert_eq!(
            TriangleFrame::of_triangle(&p, &[[0.0, 0.0]; 3], [0, 1, 7]),
            None
        );
    }
}
