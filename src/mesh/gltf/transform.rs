//! Node transforms: glTF `matrix` or translation/rotation/scale, and baking them into a part.

use super::error::GltfError;
use crate::json::Value;
use crate::mesh::MeshPart;

/// A column-major 4×4 affine matrix, as in glTF.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Transform([f64; 16]);

impl Transform {
    pub const IDENTITY: Transform = Transform([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);

    /// The local transform of `node` (`at` names it in errors).
    pub fn of_node(node: &Value, at: &str) -> Result<Transform, GltfError> {
        let nums = |key: &str, n: usize| -> Result<Option<Vec<f64>>, GltfError> {
            let Some(v) = node.get(key) else {
                return Ok(None);
            };
            let list: Option<Vec<f64>> = v
                .as_array()
                .filter(|a| a.len() == n)
                .and_then(|a| a.iter().map(Value::as_f64).collect());
            list.filter(|l| l.iter().all(|x| x.is_finite()))
                .map(Some)
                .ok_or_else(|| {
                    GltfError::invalid(format!("{at}.{key}"), format!("expected {n} numbers"))
                })
        };
        if let Some(m) = nums("matrix", 16)? {
            let mut t = [0.0; 16];
            t.copy_from_slice(&m);
            return Ok(Transform(t));
        }
        let tr = nums("translation", 3)?.unwrap_or(vec![0.0; 3]);
        let q = nums("rotation", 4)?.unwrap_or(vec![0.0, 0.0, 0.0, 1.0]);
        let s = nums("scale", 3)?.unwrap_or(vec![1.0; 3]);
        let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
        let r = [
            [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y + z * w),
                2.0 * (x * z - y * w),
            ],
            [
                2.0 * (x * y - z * w),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z + x * w),
            ],
            [
                2.0 * (x * z + y * w),
                2.0 * (y * z - x * w),
                1.0 - 2.0 * (x * x + y * y),
            ],
        ];
        let mut t = Transform::IDENTITY.0;
        for col in 0..3 {
            for row in 0..3 {
                t[col * 4 + row] = r[col][row] * s[col];
            }
            t[12 + col] = tr[col];
        }
        Ok(Transform(t))
    }

    /// `self * other` (apply `other` first).
    pub fn then_local(&self, local: &Transform) -> Transform {
        let (a, b) = (&self.0, &local.0);
        let mut m = [0.0; 16];
        for col in 0..4 {
            for row in 0..4 {
                m[col * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum();
            }
        }
        Transform(m)
    }

    /// Moves `part` into this frame: positions by the matrix, normals by its inverse transpose,
    /// tangents by its upper 3×3. A mirroring transform also flips the winding and tangent sign.
    /// The identity leaves the part untouched (bit-exact).
    pub fn apply(&self, part: &mut MeshPart) {
        if *self == Transform::IDENTITY {
            return;
        }
        let m = &self.0;
        let lin = |v: [f32; 3], w: f64| -> [f64; 3] {
            let v = [v[0] as f64, v[1] as f64, v[2] as f64];
            std::array::from_fn(|r| m[r] * v[0] + m[4 + r] * v[1] + m[8 + r] * v[2] + m[12 + r] * w)
        };
        let to32 = |v: [f64; 3]| [v[0] as f32, v[1] as f32, v[2] as f32];
        for p in &mut part.positions {
            *p = to32(lin(*p, 1.0));
        }
        let det = self.det3();
        let inv_t = self.inverse_transpose3();
        for n in &mut part.normals {
            let v = [n[0] as f64, n[1] as f64, n[2] as f64];
            let r: [f64; 3] = std::array::from_fn(|i| (0..3).map(|k| inv_t[i][k] * v[k]).sum());
            *n = to32(normalized(r));
        }
        for t in &mut part.tangents {
            let r = to32(normalized(lin([t[0], t[1], t[2]], 0.0)));
            let w = if det < 0.0 { -t[3] } else { t[3] };
            *t = [r[0], r[1], r[2], w];
        }
        if det < 0.0 {
            part.triangles.iter_mut().for_each(|t| t.swap(1, 2));
        }
    }

    fn at(&self, row: usize, col: usize) -> f64 {
        self.0[col * 4 + row]
    }

    fn det3(&self) -> f64 {
        let a = |r, c| self.at(r, c);
        a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1))
            - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
            + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0))
    }

    /// Rows of the inverse transpose of the upper 3×3 (the cofactor matrix divided by the
    /// determinant; only the direction matters, so a zero determinant just skips the division).
    fn inverse_transpose3(&self) -> [[f64; 3]; 3] {
        let a = |r: usize, c: usize| self.at(r % 3, c % 3);
        let det = self.det3();
        let k = if det != 0.0 { 1.0 / det } else { 1.0 };
        std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                (a(r + 1, c + 1) * a(r + 2, c + 2) - a(r + 1, c + 2) * a(r + 2, c + 1)) * k
            })
        })
    }
}

fn normalized(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 && l.is_finite() {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part() -> MeshPart {
        MeshPart {
            positions: vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            normals: vec![[1.0, 0.0, 0.0]; 3],
            tangents: vec![[0.0, 1.0, 0.0, 1.0]; 3],
            triangles: vec![[0, 1, 2]],
            ..MeshPart::default()
        }
    }

    #[test]
    fn trs_and_mirroring() {
        let node = crate::json::parse(
            r#"{"translation": [1, 2, 3], "rotation": [0, 0, 0.7071067811865476, 0.7071067811865476], "scale": [2, 2, 2]}"#,
        )
        .unwrap();
        let t = Transform::of_node(&node, "nodes[0]").unwrap();
        let mut p = part();
        t.apply(&mut p);
        // 90° about +z: x → y; scaled by 2 and moved.
        assert!((p.positions[0][0] - 1.0).abs() < 1e-6 && (p.positions[0][1] - 4.0).abs() < 1e-6);
        assert!((p.normals[0][1] - 1.0).abs() < 1e-6);
        assert_eq!(p.triangles, [[0, 1, 2]]);

        let mirror = crate::json::parse(r#"{"scale": [-1, 1, 1]}"#).unwrap();
        let mut p = part();
        Transform::of_node(&mirror, "n").unwrap().apply(&mut p);
        assert_eq!(p.triangles, [[0, 2, 1]]);
        assert_eq!(p.tangents[0][3], -1.0);
        assert_eq!(p.normals[0], [-1.0, 0.0, 0.0]);

        let mut p = part();
        Transform::IDENTITY.apply(&mut p);
        assert_eq!(p, part());
        assert!(
            Transform::of_node(&crate::json::parse(r#"{"scale": [1]}"#).unwrap(), "n").is_err()
        );
    }
}
