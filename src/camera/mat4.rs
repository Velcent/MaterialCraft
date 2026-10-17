//! 4×4 matrices, column-major: element (row r, column c) is `m[c * 4 + r]`; elements 12, 13, 14 are the translation.

use super::Vec3;
use crate::v3::{dot, mul, sub};

/// A 4×4 matrix, column-major (OpenGL and glTF order).
pub type Mat4 = [f32; 16];

/// The identity matrix.
pub const IDENTITY: Mat4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// `a · b` (`b` applied first).
pub fn multiply(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut o = [0.0f32; 16];
    for (i, out) in o.iter_mut().enumerate() {
        let (c, r) = (i / 4, i % 4);
        *out = (0..4).map(|k| at(a, r, k) * at(b, k, c)).sum();
    }
    o
}

/// `m · (p, 1)` as homogeneous coordinates `[x, y, z, w]`.
pub fn transform_point(m: &Mat4, p: Vec3) -> [f32; 4] {
    let mut o = [0.0f32; 4];
    for (r, out) in o.iter_mut().enumerate() {
        *out = at(m, r, 0) * p[0] + at(m, r, 1) * p[1] + at(m, r, 2) * p[2] + at(m, r, 3);
    }
    o
}

/// The inverse, or `None` for a singular (or non-finite) matrix.
pub fn inverse(m: &Mat4) -> Option<Mat4> {
    // cofactor expansion through 2×2 sub-determinants of the upper and lower row pairs
    let e = |r: usize, c: usize| at(m, r, c);
    let s0 = e(0, 0) * e(1, 1) - e(1, 0) * e(0, 1);
    let s1 = e(0, 0) * e(1, 2) - e(1, 0) * e(0, 2);
    let s2 = e(0, 0) * e(1, 3) - e(1, 0) * e(0, 3);
    let s3 = e(0, 1) * e(1, 2) - e(1, 1) * e(0, 2);
    let s4 = e(0, 1) * e(1, 3) - e(1, 1) * e(0, 3);
    let s5 = e(0, 2) * e(1, 3) - e(1, 2) * e(0, 3);
    let c5 = e(2, 2) * e(3, 3) - e(3, 2) * e(2, 3);
    let c4 = e(2, 1) * e(3, 3) - e(3, 1) * e(2, 3);
    let c3 = e(2, 1) * e(3, 2) - e(3, 1) * e(2, 2);
    let c2 = e(2, 0) * e(3, 3) - e(3, 0) * e(2, 3);
    let c1 = e(2, 0) * e(3, 2) - e(3, 0) * e(2, 2);
    let c0 = e(2, 0) * e(3, 1) - e(3, 0) * e(2, 1);
    let det = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0;
    if !det.is_finite() || det.abs() < f32::MIN_POSITIVE {
        return None;
    }
    let d = 1.0 / det;
    // rows of the inverse
    let rows = [
        [
            (e(1, 1) * c5 - e(1, 2) * c4 + e(1, 3) * c3) * d,
            (-e(0, 1) * c5 + e(0, 2) * c4 - e(0, 3) * c3) * d,
            (e(3, 1) * s5 - e(3, 2) * s4 + e(3, 3) * s3) * d,
            (-e(2, 1) * s5 + e(2, 2) * s4 - e(2, 3) * s3) * d,
        ],
        [
            (-e(1, 0) * c5 + e(1, 2) * c2 - e(1, 3) * c1) * d,
            (e(0, 0) * c5 - e(0, 2) * c2 + e(0, 3) * c1) * d,
            (-e(3, 0) * s5 + e(3, 2) * s2 - e(3, 3) * s1) * d,
            (e(2, 0) * s5 - e(2, 2) * s2 + e(2, 3) * s1) * d,
        ],
        [
            (e(1, 0) * c4 - e(1, 1) * c2 + e(1, 3) * c0) * d,
            (-e(0, 0) * c4 + e(0, 1) * c2 - e(0, 3) * c0) * d,
            (e(3, 0) * s4 - e(3, 1) * s2 + e(3, 3) * s0) * d,
            (-e(2, 0) * s4 + e(2, 1) * s2 - e(2, 3) * s0) * d,
        ],
        [
            (-e(1, 0) * c3 + e(1, 1) * c1 - e(1, 2) * c0) * d,
            (e(0, 0) * c3 - e(0, 1) * c1 + e(0, 2) * c0) * d,
            (-e(3, 0) * s3 + e(3, 1) * s1 - e(3, 2) * s0) * d,
            (e(2, 0) * s3 - e(2, 1) * s1 + e(2, 2) * s0) * d,
        ],
    ];
    let mut o = [0.0f32; 16];
    for (i, out) in o.iter_mut().enumerate() {
        let (c, r) = (i / 4, i % 4);
        *out = rows.get(r).and_then(|row| row.get(c)).copied().unwrap_or(0.0);
    }
    o.iter().all(|v| v.is_finite()).then_some(o)
}

/// The world → view matrix of a camera at `eye` with the orthonormal basis `right`, `up` and `forward` (the
/// viewing direction; view space looks down −z).
pub fn look_along(eye: Vec3, right: Vec3, up: Vec3, forward: Vec3) -> Mat4 {
    let back = mul(forward, -1.0);
    [right[0], up[0], back[0], 0.0, right[1], up[1], back[1], 0.0, right[2], up[2], back[2], 0.0, -dot(right, eye), -dot(up, eye), -dot(back, eye), 1.0]
}

/// The world → view matrix of a camera at `eye` looking at `target`, with `up` as the approximate up direction.
/// Falls back to another up direction when `up` is parallel to the viewing direction.
pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    use crate::v3::{cross, len, norm};
    let forward = norm(sub(target, eye));
    let mut right = cross(forward, up);
    if len(right) < 1e-6 {
        right = cross(forward, if forward[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] });
    }
    let right = norm(right);
    look_along(eye, right, cross(right, forward), forward)
}

#[inline]
fn at(m: &Mat4, r: usize, c: usize) -> f32 {
    m.get(c * 4 + r).copied().unwrap_or(0.0)
}
