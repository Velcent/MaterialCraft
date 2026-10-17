//! Orbit camera: view-projection matrix and eye position from the `View`.
use crate::render3d::View;

pub(super) type M4 = [f32; 16]; // column-major

fn m_mul(a: &M4, b: &M4) -> M4 {
    let mut o = [0.0f32; 16];
    for c in 0..4 {
        for r in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k * 4 + r] * b[c * 4 + k];
            }
            o[c * 4 + r] = s;
        }
    }
    o
}

pub(super) struct CamOut {
    pub vp: M4,
    pub eye: [f32; 3],
}

pub(super) fn camera(v: &View, aspect: f32) -> CamOut {
    use crate::v3::*;
    let (sy, cy) = v.yaw.sin_cos();
    let (sp, cp) = v.pitch.sin_cos();
    let eye0 = [v.dist * sy * cp, v.dist * sp, v.dist * cy * cp];
    let fwd = norm(mul(eye0, -1.0));
    let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
    let up = cross(right, fwd);
    let eye = add(eye0, add(mul(right, v.pan_x), mul(up, v.pan_y)));
    // view: rows right, up, -fwd
    let view: M4 = [
        right[0],
        up[0],
        -fwd[0],
        0.0, //
        right[1],
        up[1],
        -fwd[1],
        0.0, //
        right[2],
        up[2],
        -fwd[2],
        0.0, //
        -dot(right, eye),
        -dot(up, eye),
        dot(fwd, eye),
        1.0,
    ];
    let tf = (20.0f32).to_radians().tan();
    let (n, f) = (0.03f32, 60.0f32);
    let proj: M4 = [
        1.0 / (aspect * tf),
        0.0,
        0.0,
        0.0, //
        0.0,
        1.0 / tf,
        0.0,
        0.0, //
        0.0,
        0.0,
        -(f + n) / (f - n),
        -1.0, //
        0.0,
        0.0,
        -2.0 * f * n / (f - n),
        0.0,
    ];
    CamOut { vp: m_mul(&proj, &view), eye }
}
