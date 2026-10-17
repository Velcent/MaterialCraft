//! Camera and light moves of the Paint viewport, as plain functions on the view (ADR 0008
//! mc2-vq12: orbit, orbit snapping to axis views, pan, zoom, turning the light). Which gesture
//! triggers which move comes from the shortcut table.

use materialcraft::render3d::View;
use std::f32::consts::FRAC_PI_4;

/// Pitch limit, as before (just short of straight down or up).
const PITCH: f32 = 1.4;
const TURN: f32 = 0.01;

pub fn orbit(v: &mut View, dx: f32, dy: f32) {
    v.yaw -= dx * TURN;
    v.pitch = (v.pitch + dy * TURN).clamp(-PITCH, PITCH);
}

/// Orbits from the unsnapped angles in `raw` (kept for the whole drag) and shows the nearest
/// axis view: yaw in 90° steps, pitch in 45° steps (within the pitch limit).
pub fn orbit_snapped(v: &mut View, raw: &mut (f32, f32), dx: f32, dy: f32) {
    raw.0 -= dx * TURN;
    raw.1 = (raw.1 + dy * TURN).clamp(-PITCH, PITCH);
    v.yaw = (raw.0 / (2.0 * FRAC_PI_4)).round() * 2.0 * FRAC_PI_4;
    v.pitch = ((raw.1 / FRAC_PI_4).round() * FRAC_PI_4).clamp(-PITCH, PITCH);
}

pub fn pan(v: &mut View, dx: f32, dy: f32) {
    let k = 0.004 * v.dist / 3.4;
    v.pan_x = (v.pan_x - dx * k).clamp(-3.0, 3.0);
    v.pan_y = (v.pan_y + dy * k).clamp(-3.0, 3.0);
}

/// Zooms by a drag (down = closer) or by wheel points (`amount` positive = closer).
pub fn zoom(v: &mut View, amount: f32) {
    v.dist = (v.dist * (1.0 - amount * 0.001)).clamp(1.6, 9.0);
}

pub fn light(v: &mut View, dx: f32, dy: f32) {
    v.light_yaw -= dx * TURN;
    v.light_pitch = (v.light_pitch + dy * TURN).clamp(-PITCH, PITCH);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn snapped_orbit_shows_axis_views_and_keeps_the_raw_angle() {
        let mut v = View::default();
        let mut raw = (0.0, 0.0);
        orbit_snapped(&mut v, &mut raw, -50.0, 0.0);
        assert_eq!(v.yaw, 0.0, "half a radian rounds back to the front view");
        orbit_snapped(&mut v, &mut raw, -110.0, 10.0);
        assert!((v.yaw - FRAC_PI_2).abs() < 1e-6 && v.pitch == 0.0);
        assert!((raw.0 - 1.6).abs() < 1e-5, "the raw angle keeps counting");
    }

    #[test]
    fn moves_stay_in_their_limits() {
        let mut v = View::default();
        orbit(&mut v, 0.0, 1e6);
        assert_eq!(v.pitch, PITCH);
        zoom(&mut v, 1e6);
        assert_eq!(v.dist, 1.6);
        pan(&mut v, 1e6, 0.0);
        assert_eq!(v.pan_x, -3.0);
    }
}
