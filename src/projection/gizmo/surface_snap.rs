//! Snap to surface (PJ-24, planar and warp only, ADR 0008 mc2-q-pj-14): the volume is centred on a surface point
//! and turned so its depth axis (local +z, where a planar projection looks from) follows the surface normal. The
//! turn is the smallest one that does so: the volume's x axis keeps its direction as far as the new plane allows,
//! so dragging along a surface does not spin the image.

use super::super::frame::SceneFrame;
use super::super::modes::Volume;
use super::maths::{column, from_columns};
use super::placed::PlacedVolume;
use crate::mesh::Vec3;
use crate::v3::{cross, dot, len, mul, norm, sub};

/// `volume` placed on the surface point `pos` with normal `normal` (object space); scale and clipping are kept.
/// A zero or non-finite normal keeps the turn and only moves the volume.
pub fn snap_to_surface(volume: &Volume, frame: &SceneFrame, pos: Vec3, normal: Vec3) -> Volume {
    let mut placed = PlacedVolume::of(volume, frame);
    if pos.iter().all(|v| v.is_finite()) {
        placed.centre = pos;
    }
    if len(normal) > 1e-9 && normal.iter().all(|v| v.is_finite()) {
        let z = norm(normal);
        let old_x = column(&placed.rotation, 0);
        let old_y = column(&placed.rotation, 1);
        // the old x axis in the new plane; the old y axis when x runs along the normal
        let x = [old_x, old_y].into_iter().map(|a| sub(a, mul(z, dot(a, z)))).find(|a| len(*a) > 1e-4).map(norm).unwrap_or_else(|| any_across(z));
        let y = cross(z, x);
        placed.rotation = from_columns(x, y, z);
    }
    placed.to_volume(frame, volume.clip)
}

/// A unit vector across `z`.
fn any_across(z: Vec3) -> Vec3 {
    let helper = if z[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    norm(sub(helper, mul(z, dot(helper, z))))
}
