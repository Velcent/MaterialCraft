//! The geometry the gizmo drags are made of: object-space rays, closest points, plane hits, rotations about any
//! axis and step snapping.

use super::super::rotation::{IDENTITY, Mat3};
use crate::mesh::Vec3;
use crate::v3::{add, cross, dot, len, mul, norm, sub};

/// A ray in the mesh's object space (the space volumes are placed in); `dir` has unit length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
}

impl Ray {
    /// A ray from `origin` along `dir` (made unit length); `None` for a zero or non-finite direction.
    pub fn new(origin: Vec3, dir: Vec3) -> Option<Ray> {
        let l = len(dir);
        (l > 1e-12 && l.is_finite() && origin.iter().all(|v| v.is_finite())).then(|| Ray { origin, dir: mul(dir, 1.0 / l) })
    }

    pub fn at(&self, t: f32) -> Vec3 {
        add(self.origin, mul(self.dir, t))
    }
}

/// Where the ray meets the plane through `point` with normal `normal`; `None` when it runs (nearly) along the
/// plane.
pub fn hit_plane(ray: &Ray, point: Vec3, normal: Vec3) -> Option<Vec3> {
    let d = dot(ray.dir, normal);
    if d.abs() < 1e-6 {
        return None;
    }
    let t = dot(sub(point, ray.origin), normal) / d;
    t.is_finite().then(|| ray.at(t))
}

/// The parameter `s` of the point `origin + s · axis` (unit `axis`) nearest to the ray; `None` when the ray runs
/// (nearly) along the axis.
pub fn closest_on_axis(ray: &Ray, origin: Vec3, axis: Vec3) -> Option<f32> {
    // minimise |origin + s·axis − (ray.origin + t·ray.dir)| over s and t
    let w = sub(origin, ray.origin);
    let b = dot(axis, ray.dir);
    let denom = 1.0 - b * b;
    if denom < 1e-6 {
        return None;
    }
    let s = (b * dot(w, ray.dir) - dot(w, axis)) / denom;
    s.is_finite().then_some(s)
}

/// The right-handed rotation by `radians` about the unit vector `axis` (Rodrigues).
pub fn about_vector(axis: Vec3, radians: f32) -> Mat3 {
    if len(axis) < 1e-12 || !radians.is_finite() {
        return IDENTITY;
    }
    let [x, y, z] = norm(axis);
    let (s, c) = radians.sin_cos();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

/// Column `k` of `m` (the image of axis `k`); zero for `k` past z.
pub fn column(m: &Mat3, k: usize) -> Vec3 {
    let pick = |row: &[f32; 3]| row.get(k).copied().unwrap_or(0.0);
    [pick(&m[0]), pick(&m[1]), pick(&m[2])]
}

/// The matrix with columns `x`, `y` and `z`.
pub fn from_columns(x: Vec3, y: Vec3, z: Vec3) -> Mat3 {
    [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]]
}

/// The signed angle (radians) that turns `from` into `to` about `axis`, both measured in the plane across it.
pub fn signed_angle(from: Vec3, to: Vec3, axis: Vec3) -> f32 {
    dot(cross(from, to), norm(axis)).atan2(dot(from, to))
}

/// `value` rounded to a multiple of `step` (unchanged for a step that is not positive and finite).
pub fn snap(value: f32, step: f32) -> f32 {
    if step > 0.0 && step.is_finite() { (value / step).round() * step } else { value }
}

/// A scale rounded to a multiple of `step`, never to zero: at least one step, with its sign kept.
pub fn snap_scale(value: f32, step: f32) -> f32 {
    if !(step > 0.0 && step.is_finite()) {
        return value;
    }
    let m = ((value.abs() / step).round() * step).max(step);
    if value < 0.0 { -m } else { m }
}

/// The world axis `k` (x, y or z; z past them).
pub fn unit_axis(k: usize) -> Vec3 {
    match k {
        0 => [1.0, 0.0, 0.0],
        1 => [0.0, 1.0, 0.0],
        _ => [0.0, 0.0, 1.0],
    }
}

/// Component `k` of `v` (0 past z).
pub fn component(v: Vec3, k: usize) -> f32 {
    v.get(k).copied().unwrap_or(0.0)
}

/// `v` with component `k` set to `value` (unchanged past z).
pub fn with_component(mut v: Vec3, k: usize, value: f32) -> Vec3 {
    if let Some(c) = v.get_mut(k) {
        *c = value;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::super::super::rotation::apply;
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn rays_meet_planes_and_axes() {
        let ray = Ray::new([0.0, 0.0, 5.0], [0.0, 0.0, -2.0]).unwrap();
        assert!(close(hit_plane(&ray, [3.0, 1.0, 1.0], [0.0, 0.0, 1.0]).unwrap(), [0.0, 0.0, 1.0]));
        assert!(hit_plane(&ray, [0.0; 3], [1.0, 0.0, 0.0]).is_none());
        // the x axis through (0, 0, 0) is nearest to a ray straight down through x = 2 at s = 2
        let down = Ray::new([2.0, 0.0, 9.0], [0.0, 0.0, -1.0]).unwrap();
        assert!((closest_on_axis(&down, [0.0; 3], [1.0, 0.0, 0.0]).unwrap() - 2.0).abs() < 1e-5);
        assert!(closest_on_axis(&down, [0.0; 3], [0.0, 0.0, 1.0]).is_none());
        assert!(Ray::new([0.0; 3], [0.0; 3]).is_none());
    }

    #[test]
    fn rotations_about_any_axis_match_the_axis_rotations() {
        let q = std::f32::consts::FRAC_PI_2;
        for k in 0..3 {
            let m = about_vector(unit_axis(k), q);
            let a = super::super::super::rotation::about(k, q);
            assert!(m.iter().flatten().zip(a.iter().flatten()).all(|(x, y)| (x - y).abs() < 1e-6));
        }
        let m = about_vector([1.0, 1.0, 1.0], 2.0 * std::f32::consts::PI / 3.0);
        assert!(close(apply(&m, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
        assert!((signed_angle([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]) - q).abs() < 1e-6);
        assert!((signed_angle([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]) + q).abs() < 1e-6);
    }

    #[test]
    fn snapping_rounds_to_steps() {
        assert_eq!(snap(0.4, 0.25), 0.5);
        assert_eq!(snap(0.37, 0.0), 0.37);
        assert_eq!(snap_scale(0.04, 0.25), 0.25);
        assert_eq!(snap_scale(-0.6, 0.25), -0.5);
        assert_eq!(column(&from_columns([1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]), 1), [4.0, 5.0, 6.0]);
    }
}
