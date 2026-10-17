//! [`VolumeTransform`]: a [`Volume`] placed in a [`SceneFrame`], as the map from object space into the volume's
//! local frame that every volume-based projector starts from.
//!
//! Units: one *scene unit* is the scene's largest extent ([`SceneFrame::largest_extent`]). A volume with scale 1
//! is a cube one scene unit wide, so it encloses the whole scene; its centre sits at the scene's centre plus
//! `offset` scene units, and it is turned by `rotation` ([`super::rotation`]: degrees about x, then y, then z).
//! Local coordinates run −0.5 … 0.5 across the volume on each axis. A negative scale mirrors along that axis.

use super::frame::SceneFrame;
use super::modes::Volume;
use super::rotation::{self, EulerOrder, Mat3};
use crate::mesh::Vec3;

/// Half the side of the volume in local coordinates.
pub const HALF: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeTransform {
    /// Object → volume axes (the transposed rotation).
    to_local: Mat3,
    /// The volume's centre in object space.
    centre: Vec3,
    /// 1 / (scale × scene unit) per axis (signed: mirrored axes are negative).
    inv: Vec3,
    /// Scene units per local unit along the smallest axis, for footprints (object length → local length).
    local_per_object: f32,
    /// Stop at the volume's edge.
    pub clip: bool,
}

impl VolumeTransform {
    pub fn new(volume: &Volume, frame: &SceneFrame) -> VolumeTransform {
        let unit = frame.largest_extent();
        let finite = |v: f32, d: f32| if v.is_finite() { v } else { d };
        let radians = volume.rotation.map(|d| finite(d, 0.0).to_radians());
        let to_local = rotation::transpose(&rotation::matrix(radians, EulerOrder::Xyz));
        let (c, o) = (frame.centre, volume.offset.map(|v| finite(v, 0.0) * unit));
        let centre = [c[0] + o[0], c[1] + o[1], c[2] + o[2]];
        let inv = volume.scale.map(|s| {
            let s = finite(s, 1.0);
            let s = if s.abs() < 1e-6 { 1e-6f32.copysign(s) } else { s };
            1.0 / (s * unit)
        });
        let local_per_object = inv.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        VolumeTransform { to_local, centre, inv, local_per_object, clip: volume.clip }
    }

    /// The object-space point `q` in local coordinates.
    #[inline]
    pub fn local(&self, q: Vec3) -> Vec3 {
        let d = [q[0] - self.centre[0], q[1] - self.centre[1], q[2] - self.centre[2]];
        let r = rotation::apply(&self.to_local, d);
        [r[0] * self.inv[0], r[1] * self.inv[1], r[2] * self.inv[2]]
    }

    /// The object-space normal `n` turned into the volume's axes (mirrored axes flip it), unit length; zero for
    /// a zero normal.
    #[inline]
    pub fn local_normal(&self, n: Vec3) -> Vec3 {
        let r = rotation::apply(&self.to_local, n);
        let r = [r[0] * self.inv[0].signum(), r[1] * self.inv[1].signum(), r[2] * self.inv[2].signum()];
        let l = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        if l > 1e-12 { [r[0] / l, r[1] / l, r[2] / l] } else { [0.0; 3] }
    }

    /// An object-space length in local units (along the volume's most compressed axis, so footprints err on the
    /// blurry side rather than shimmer).
    #[inline]
    pub fn local_len(&self, object_len: f32) -> f32 {
        object_len * self.local_per_object
    }

    /// True if `l` (local) lies inside the volume.
    #[inline]
    pub fn inside(l: Vec3) -> bool {
        l.iter().all(|v| v.abs() <= HALF)
    }

    /// 1 inside the volume or when not clipping, else 0.
    #[inline]
    pub fn clip_coverage(&self, l: Vec3) -> f32 {
        if !self.clip || VolumeTransform::inside(l) { 1.0 } else { 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn default_volume_encloses_the_scene() {
        let frame = SceneFrame { centre: [1.0, 2.0, 3.0], size: [4.0, 2.0, 1.0] };
        let t = VolumeTransform::new(&Volume::default(), &frame);
        assert!(close(t.local([1.0, 2.0, 3.0]), [0.0; 3]));
        assert!(close(t.local([3.0, 3.0, 3.5]), [0.5, 0.25, 0.125]));
        assert!(VolumeTransform::inside(t.local([3.0, 3.0, 3.5])));
        assert_eq!(t.local_len(4.0), 1.0);
    }

    #[test]
    fn offset_rotation_scale_and_mirror() {
        let frame = SceneFrame { centre: [0.0; 3], size: [2.0, 2.0, 2.0] };
        let v = Volume { offset: [0.5, 0.0, 0.0], rotation: [0.0, 0.0, 90.0], scale: [0.5, 1.0, -1.0], clip: true };
        let t = VolumeTransform::new(&v, &frame);
        // centre at x = 1; turned 90° about z: object +y is the volume's +x
        assert!(close(t.local([1.0, 0.5, 0.0]), [0.5, 0.0, 0.0]));
        assert!(close(t.local([1.0, 0.0, 0.5]), [0.0, 0.0, -0.25]));
        assert!(close(t.local_normal([0.0, 1.0, 0.0]), [1.0, 0.0, 0.0]));
        assert!(close(t.local_normal([0.0, 0.0, 1.0]), [0.0, 0.0, -1.0]));
        assert_eq!(t.clip_coverage([0.6, 0.0, 0.0]), 0.0);
        assert_eq!(t.clip_coverage([0.4, 0.0, 0.0]), 1.0);
    }
}
