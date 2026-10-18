//! [`Orbit`]: the orbit controller. A turntable camera circling a pivot at the origin (yaw around the world's up
//! axis, pitch above or below the horizon, no roll), at a distance, shifted sideways by a pan in the view plane.
//! Navigation moves it; [`Orbit::basis`] turns it into an eye position and viewing axes.

use super::Vec3;
use crate::v3::{add, cross, mul, norm};
use std::f32::consts::FRAC_PI_4;

/// The eye position and the camera's unit axes (right, up and the viewing direction `forward`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Basis {
    pub eye: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

/// A turntable orbit around the origin. Angles in radians; `distance` and `pan` in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    /// Rotation around the world's up (+y) axis; 0 looks from +z towards the origin.
    pub yaw: f32,
    /// Height above the horizon; positive looks down on the model.
    pub pitch: f32,
    /// Distance from the pivot to the eye (before the pan).
    pub distance: f32,
    /// Shift of eye and pivot along the camera's right and up axes.
    pub pan: [f32; 2],
}

impl Default for Orbit {
    /// The viewport's start view of the unit-size model.
    fn default() -> Orbit {
        Orbit { yaw: 0.5, pitch: 0.3, distance: 3.4, pan: [0.0, 0.0] }
    }
}

impl Orbit {
    /// Pitch limit: just short of looking straight down or up, where a turntable flips.
    pub const PITCH_LIMIT: f32 = 1.4;
    /// Closest and farthest distance navigation zooms to (the model is about unit size).
    pub const DISTANCE: (f32, f32) = (1.6, 9.0);
    /// How far navigation pans from the pivot along either axis.
    pub const PAN_LIMIT: f32 = 3.0;

    /// The eye and the camera's axes.
    pub fn basis(&self) -> Basis {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let eye0 = [self.distance * sy * cp, self.distance * sp, self.distance * cy * cp];
        let forward = norm(mul(eye0, -1.0));
        let right = norm(cross(forward, [0.0, 1.0, 0.0]));
        let up = cross(right, forward);
        let eye = add(eye0, add(mul(right, self.pan[0]), mul(up, self.pan[1])));
        Basis { eye, right, up, forward }
    }

    /// Turns the camera by `yaw` and `pitch` radians (pitch within [`Orbit::PITCH_LIMIT`]).
    pub fn rotate(&mut self, yaw: f32, pitch: f32) {
        self.yaw += yaw;
        self.pitch = (self.pitch + pitch).clamp(-Self::PITCH_LIMIT, Self::PITCH_LIMIT);
    }

    /// Turns the unsnapped angles in `raw` (kept by the caller for the whole gesture) and shows the nearest axis
    /// view: yaw in 90° steps, pitch in 45° steps (within the pitch limit).
    pub fn rotate_snapped(&mut self, raw: &mut (f32, f32), yaw: f32, pitch: f32) {
        raw.0 += yaw;
        raw.1 = (raw.1 + pitch).clamp(-Self::PITCH_LIMIT, Self::PITCH_LIMIT);
        self.yaw = (raw.0 / (2.0 * FRAC_PI_4)).round() * 2.0 * FRAC_PI_4;
        self.pitch = ((raw.1 / FRAC_PI_4).round() * FRAC_PI_4).clamp(-Self::PITCH_LIMIT, Self::PITCH_LIMIT);
    }

    /// Shifts the view by `dx`, `dy` world units along the camera's right and up axes (within the pan limit).
    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.pan[0] = (self.pan[0] + dx).clamp(-Self::PAN_LIMIT, Self::PAN_LIMIT);
        self.pan[1] = (self.pan[1] + dy).clamp(-Self::PAN_LIMIT, Self::PAN_LIMIT);
    }

    /// Scales the distance by `factor` (below 1 moves closer), within [`Orbit::DISTANCE`].
    pub fn zoom(&mut self, factor: f32) {
        if factor.is_finite() {
            self.distance = (self.distance * factor).clamp(Self::DISTANCE.0, Self::DISTANCE.1);
        }
    }
}
