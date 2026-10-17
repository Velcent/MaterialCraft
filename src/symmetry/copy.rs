//! [`CopyTransform`]: one symmetric copy as a rigid transform of object-space positions and normals.

use crate::mesh::Vec3;

/// One copy: `p' = linear · p + translate` for positions, `n' = linear · n` for normals (the linear part is a
/// rotation or a reflection, so it keeps lengths), and the flips of the copy's stamp in its own frame.
///
/// A texel is painted by copy `T` where `T(texel)` lies under the stamp: the copy appears at `T⁻¹` of the stamp.
/// Axis mirrors and rotations by multiples of 90° are exact (their matrices hold only 0 and ±1), so a mirrored
/// texel grid lands on itself bit for bit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CopyTransform {
    /// Row-major 3 × 3.
    pub linear: [[f32; 3]; 3],
    pub translate: Vec3,
    /// Flip the stamp along its frame's `[u, v]` axes, about the stamp's centre.
    pub flip: [bool; 2],
}

impl CopyTransform {
    pub const IDENTITY: CopyTransform = CopyTransform { linear: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], translate: [0.0; 3], flip: [false; 2] };

    /// The reflection across the plane `p[axis] = offset`; `None` for an axis past z or a non-finite offset.
    pub fn mirror(axis: usize, offset: f32) -> Option<CopyTransform> {
        if axis >= 3 || !offset.is_finite() {
            return None;
        }
        let mut c = CopyTransform::IDENTITY;
        let row = c.linear.get_mut(axis)?;
        *row.get_mut(axis)? = -1.0;
        *c.translate.get_mut(axis)? = 2.0 * offset;
        Some(c)
    }

    /// The rotation by `degrees` (right-handed) around object axis `axis` through `point`; `None` for an axis past
    /// z or a non-finite point or angle. Multiples of 90° use exact sines and cosines.
    pub fn rotation(axis: usize, point: Vec3, degrees: f64) -> Option<CopyTransform> {
        if axis >= 3 || !point.iter().all(|v| v.is_finite()) || !degrees.is_finite() {
            return None;
        }
        let (s, c) = exact_sin_cos(degrees);
        // the two axes the rotation turns, in right-handed order
        let (i, j) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut m = [[0.0f32; 3]; 3];
        set(&mut m, axis, axis, 1.0)?;
        set(&mut m, i, i, c)?;
        set(&mut m, i, j, -s)?;
        set(&mut m, j, i, s)?;
        set(&mut m, j, j, c)?;
        let turned = apply(&m, point);
        let translate = [point[0] - turned[0], point[1] - turned[1], point[2] - turned[2]];
        Some(CopyTransform { linear: m, translate, flip: [false; 2] })
    }

    /// True for the direct copy.
    pub fn is_identity(&self) -> bool {
        *self == CopyTransform::IDENTITY
    }

    /// True if the copy flips its stamp in either direction.
    pub fn flips(&self) -> bool {
        self.flip[0] || self.flip[1]
    }

    #[inline]
    pub fn point(&self, p: Vec3) -> Vec3 {
        let q = apply(&self.linear, p);
        [q[0] + self.translate[0], q[1] + self.translate[1], q[2] + self.translate[2]]
    }

    #[inline]
    pub fn normal(&self, n: Vec3) -> Vec3 {
        apply(&self.linear, n)
    }

    /// The position `p` in the copy's stamp frame: flipped about the stamp's `center` where the copy flips.
    #[inline]
    pub fn frame(&self, p: [f32; 2], center: [f32; 2]) -> [f32; 2] {
        let f = |v: f32, c: f32, on: bool| if on { c - (v - c) } else { v };
        [f(p[0], center[0], self.flip[0]), f(p[1], center[1], self.flip[1])]
    }

    /// The 4 × 4 column-major matrix of the position transform (translation in elements 12 … 14).
    pub fn matrix(&self) -> [f32; 16] {
        let mut out = [0.0; 16];
        for (r, row) in self.linear.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                if let Some(o) = out.get_mut(c * 4 + r) {
                    *o = *v;
                }
            }
        }
        for (k, t) in self.translate.iter().enumerate() {
            if let Some(o) = out.get_mut(12 + k) {
                *o = *t;
            }
        }
        out[15] = 1.0;
        out
    }
}

#[inline]
fn apply(m: &[[f32; 3]; 3], p: Vec3) -> Vec3 {
    let [a, b, c] = m;
    let row = |r: &[f32; 3]| r[0] * p[0] + r[1] * p[1] + r[2] * p[2];
    [row(a), row(b), row(c)]
}

fn set(m: &mut [[f32; 3]; 3], r: usize, c: usize, v: f32) -> Option<()> {
    *m.get_mut(r)?.get_mut(c)? = v;
    Some(())
}

/// Sine and cosine of `degrees`, exact at multiples of 90°.
fn exact_sin_cos(degrees: f64) -> (f32, f32) {
    let d = degrees.rem_euclid(360.0);
    if d.fract() == 0.0 && (d as u32).is_multiple_of(90) {
        return match d as u32 {
            0 => (0.0, 1.0),
            90 => (1.0, 0.0),
            180 => (0.0, -1.0),
            _ => (-1.0, 0.0),
        };
    }
    let r = d.to_radians();
    (r.sin() as f32, r.cos() as f32)
}
