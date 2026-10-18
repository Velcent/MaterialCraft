//! [`PlacedVolume`]: a [`Volume`] as the box it is in the mesh's object space (centre, axes, size), and back.
//! The gizmo edits this form; [`PlacedVolume::to_volume`] turns it into the stored offset, Euler angles and scale
//! with the conventions of [`super::super::volume`] and [`super::super::rotation`].

use super::super::frame::SceneFrame;
use super::super::modes::Volume;
use super::super::rotation::{self, EulerOrder, Mat3};
use super::maths::column;
use crate::mesh::Vec3;
use crate::v3::{add, mul, sub};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedVolume {
    /// Object-space centre.
    pub centre: Vec3,
    /// Volume axes → object axes (columns are the volume's x, y and z in object space).
    pub rotation: Mat3,
    /// Signed edge length along each volume axis in object units (negative: mirrored).
    pub size: Vec3,
}

impl PlacedVolume {
    /// Where `volume` lies in `frame` (non-finite settings count as their defaults, as the projector reads them).
    pub fn of(volume: &Volume, frame: &SceneFrame) -> PlacedVolume {
        let unit = frame.largest_extent();
        let finite = |v: f32, d: f32| if v.is_finite() { v } else { d };
        let offset = volume.offset.map(|v| finite(v, 0.0) * unit);
        let radians = volume.rotation.map(|d| finite(d, 0.0).to_radians());
        PlacedVolume {
            centre: add(frame.centre, offset),
            rotation: rotation::matrix(radians, EulerOrder::Xyz),
            size: volume.scale.map(|s| finite(s, 1.0) * unit),
        }
    }

    /// The stored settings of this box in `frame`, with `clip` as given (the gizmo never changes it).
    pub fn to_volume(&self, frame: &SceneFrame, clip: bool) -> Volume {
        let unit = frame.largest_extent();
        let c = frame.centre;
        let offset = [0, 1, 2].map(|k| (self.centre.get(k).copied().unwrap_or(0.0) - c.get(k).copied().unwrap_or(0.0)) / unit);
        let rotation = rotation::angles_xyz(&self.rotation).map(|r| tidy_degrees(r.to_degrees()));
        Volume { offset, rotation, scale: self.size.map(|s| s / unit), clip }
    }

    /// Volume axis `k` in object space (unit length).
    pub fn axis(&self, k: usize) -> Vec3 {
        column(&self.rotation, k)
    }

    /// The object-space point at local coordinates `l` (−0.5 … 0.5 across the box, as
    /// [`super::super::VolumeTransform::local`] returns them).
    pub fn point(&self, l: Vec3) -> Vec3 {
        let scaled = [l[0] * self.size[0], l[1] * self.size[1], l[2] * self.size[2]];
        add(self.centre, rotation::apply(&self.rotation, scaled))
    }

    /// The local coordinates of object-space point `q` (the inverse of [`PlacedVolume::point`]); `None` for a
    /// box flat along an axis.
    pub fn local(&self, q: Vec3) -> Option<Vec3> {
        let r = rotation::apply(&rotation::transpose(&self.rotation), sub(q, self.centre));
        let out = [r[0] / self.size[0], r[1] / self.size[1], r[2] / self.size[2]];
        out.iter().all(|v| v.is_finite()).then_some(out)
    }

    /// The box's twelve edges as object-space segments (for drawing its outline).
    pub fn edges(&self) -> Vec<[Vec3; 2]> {
        let h = 0.5;
        let corner = |i: usize| self.point([if i & 1 == 0 { -h } else { h }, if i & 2 == 0 { -h } else { h }, if i & 4 == 0 { -h } else { h }]);
        let mut out = Vec::with_capacity(12);
        for i in 0..8usize {
            for bit in [1usize, 2, 4] {
                if i & bit == 0 {
                    out.push([corner(i), corner(i | bit)]);
                }
            }
        }
        out
    }

    /// The centre of the box face on the +z side (where a planar projection looks from), for drawing.
    pub fn front(&self) -> Vec3 {
        add(self.centre, mul(self.axis(2), 0.5 * self.size[2]))
    }
}

/// Degrees with rounding noise near whole numbers removed and −180 shown as 180.
fn tidy_degrees(d: f32) -> f32 {
    let r = d.round();
    let d = if (d - r).abs() < 1e-3 { r } else { d };
    if d <= -180.0 { d + 360.0 } else { d }
}
