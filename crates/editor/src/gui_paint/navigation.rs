//! Camera and light moves of the Paint viewport (ADR 0008 mc2-vq12: orbit, orbit snapping to axis views, pan,
//! zoom, turning the light): pointer deltas turned into moves of the orbit controller ([`Orbit`]). Which gesture
//! triggers which move comes from the shortcut table.

use materialcraft::camera::Orbit;
use materialcraft::render3d::View;

/// Radians per point of pointer movement.
const TURN: f32 = 0.01;

/// Applies `f` to the view's orbit pose.
fn with_orbit(v: &mut View, f: impl FnOnce(&mut Orbit)) {
    let mut o = v.orbit();
    f(&mut o);
    v.set_orbit(&o);
}

pub fn orbit(v: &mut View, dx: f32, dy: f32) {
    with_orbit(v, |o| o.rotate(-dx * TURN, dy * TURN));
}

/// Orbits from the unsnapped angles in `raw` (kept for the whole drag) and shows the nearest
/// axis view: yaw in 90° steps, pitch in 45° steps (within the pitch limit).
pub fn orbit_snapped(v: &mut View, raw: &mut (f32, f32), dx: f32, dy: f32) {
    with_orbit(v, |o| o.rotate_snapped(raw, -dx * TURN, dy * TURN));
}

/// Pans by a pointer delta; the speed follows the distance, so the model keeps up with the pointer.
pub fn pan(v: &mut View, dx: f32, dy: f32) {
    let k = 0.004 * v.dist / 3.4;
    with_orbit(v, |o| o.pan_by(-dx * k, dy * k));
}

/// Zooms by a drag (down = closer) or by wheel points (`amount` positive = closer).
pub fn zoom(v: &mut View, amount: f32) {
    with_orbit(v, |o| o.zoom(1.0 - amount * 0.001));
}

pub fn light(v: &mut View, dx: f32, dy: f32) {
    v.light_yaw -= dx * TURN;
    v.light_pitch = (v.light_pitch + dy * TURN).clamp(-Orbit::PITCH_LIMIT, Orbit::PITCH_LIMIT);
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
        assert_eq!(v.pitch, Orbit::PITCH_LIMIT);
        zoom(&mut v, 1e6);
        assert_eq!(v.dist, Orbit::DISTANCE.0);
        pan(&mut v, 1e6, 0.0);
        assert_eq!(v.pan_x, -Orbit::PAN_LIMIT);
    }
}
