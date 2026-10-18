//! [`Orbit::from_pose`]: the orbit pose of any camera given by its eye, viewing direction and up direction (cameras
//! imported from other programs). Exact for every pose: yaw and pitch give the viewing direction, the roll the turn
//! about it, the distance and pan the eye's position along and across it.

use super::Vec3;
use super::orbit::Orbit;
use crate::v3::{cross, dot, len, mul};
use std::f32::consts::FRAC_PI_2;

/// Below this length of its horizontal part a viewing direction counts as straight up or down.
const VERTICAL: f32 = 1e-5;
/// How far short of ±90° the pitch of a straight down or up view stays (radians), so the orbit's level axis exists.
const POLE_MARGIN: f32 = 2e-5;

impl Orbit {
    /// The orbit whose [`Orbit::basis`] has the eye `eye`, looks along `forward` and has `up` as close as possible to
    /// its up axis. `forward` and `up` need not be unit length; `None` when they are zero, parallel or not finite.
    pub fn from_pose(eye: Vec3, forward: Vec3, up: Vec3) -> Option<Orbit> {
        let finite = |v: Vec3| v.iter().all(|x| x.is_finite());
        if !(finite(eye) && finite(forward) && finite(up)) {
            return None;
        }
        let f = unit(forward)?;
        // the camera's own right axis; parallel forward and up have none
        let right = unit(cross(f, up))?;
        let horizontal = (f[0] * f[0] + f[2] * f[2]).sqrt();
        let (yaw, pitch) = if horizontal < VERTICAL {
            // looking straight down (or up): the yaw is free, so pick the one that needs no roll
            let pitch = if f[1] < 0.0 { FRAC_PI_2 - POLE_MARGIN } else { -(FRAC_PI_2 - POLE_MARGIN) };
            ((-right[2]).atan2(right[0]), pitch)
        } else {
            ((-f[0]).atan2(-f[2]), (-f[1]).clamp(-1.0, 1.0).asin())
        };
        let level = Orbit { yaw, pitch, distance: 1.0, pan: [0.0, 0.0], roll: 0.0 }.basis();
        let roll = dot(right, level.up).atan2(dot(right, level.right));
        let mut orbit = Orbit { yaw, pitch, distance: 1.0, pan: [0.0, 0.0], roll };
        let b = orbit.basis();
        // eye = −distance · forward + pan · (right, up), with the axes of the orbit just built
        orbit.distance = -dot(eye, b.forward);
        orbit.pan = [dot(eye, b.right), dot(eye, b.up)];
        Some(orbit)
    }
}

fn unit(v: Vec3) -> Option<Vec3> {
    let l = len(v);
    (l > 1e-12 && l.is_finite()).then(|| mul(v, 1.0 / l))
}
