//! Rotation matrices from Euler angles, and back.
//!
//! Our convention ([`Volume::rotation`](super::Volume)): degrees about the object's x, y and z axes, applied in
//! that order to the volume (first about x, then about y, then about z, all about the fixed object axes), so
//! the matrix is `R = Rz · Ry · Rx`. Other applications store angles in another order; [`EulerOrder`] names the
//! six orders so an importer can turn their angles into a matrix ([`matrix`]) and the matrix into ours
//! ([`angles_xyz`]).

/// Row-major 3 × 3.
pub type Mat3 = [[f32; 3]; 3];

pub const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// The order in which rotations about the fixed object axes are applied (`Xyz`: first about x, last about z).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EulerOrder {
    #[default]
    Xyz,
    Xzy,
    Yxz,
    Yzx,
    Zxy,
    Zyx,
}

impl EulerOrder {
    pub const ALL: [EulerOrder; 6] = [EulerOrder::Xyz, EulerOrder::Xzy, EulerOrder::Yxz, EulerOrder::Yzx, EulerOrder::Zxy, EulerOrder::Zyx];

    /// The axes in the order they are applied.
    pub fn axes(self) -> [usize; 3] {
        match self {
            EulerOrder::Xyz => [0, 1, 2],
            EulerOrder::Xzy => [0, 2, 1],
            EulerOrder::Yxz => [1, 0, 2],
            EulerOrder::Yzx => [1, 2, 0],
            EulerOrder::Zxy => [2, 0, 1],
            EulerOrder::Zyx => [2, 1, 0],
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            EulerOrder::Xyz => "xyz",
            EulerOrder::Xzy => "xzy",
            EulerOrder::Yxz => "yxz",
            EulerOrder::Yzx => "yzx",
            EulerOrder::Zxy => "zxy",
            EulerOrder::Zyx => "zyx",
        }
    }
}

/// The right-handed rotation by `radians` about object axis `axis` (0 = x, 1 = y, 2 = z; others: identity).
pub fn about(axis: usize, radians: f32) -> Mat3 {
    let (s, c) = radians.sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        2 => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
        _ => IDENTITY,
    }
}

pub fn mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let bt = transpose(b);
    a.map(|row| bt.map(|col| row[0] * col[0] + row[1] * col[1] + row[2] * col[2]))
}

pub fn transpose(m: &Mat3) -> Mat3 {
    [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]]
}

#[inline]
pub fn apply(m: &Mat3, v: [f32; 3]) -> [f32; 3] {
    [m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2], m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2], m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2]]
}

/// The rotation of angles `radians` (indexed by axis: about x, about y, about z) applied in `order`.
/// Non-finite angles count as 0.
pub fn matrix(radians: [f32; 3], order: EulerOrder) -> Mat3 {
    let mut m = IDENTITY;
    for axis in order.axes() {
        let a = radians.get(axis).copied().filter(|a| a.is_finite()).unwrap_or(0.0);
        m = mul(&about(axis, a), &m);
    }
    m
}

/// Our angles (radians, about x, y, z applied in that order) of rotation `m`: the inverse of
/// `matrix(_, EulerOrder::Xyz)`. At gimbal lock (y = ±90°) the angle about x is folded into z.
pub fn angles_xyz(m: &Mat3) -> [f32; 3] {
    // R = Rz Ry Rx: m[2][0] = −sin y, m[2][1] = cos y sin x, m[2][2] = cos y cos x,
    // m[1][0] = cos y sin z, m[0][0] = cos y cos z
    let sy = (-m[2][0]).clamp(-1.0, 1.0);
    let y = sy.asin();
    if sy.abs() < 0.999_999 {
        [m[2][1].atan2(m[2][2]), y, m[1][0].atan2(m[0][0])]
    } else {
        // cos y = 0: only x − z (y = +90°) or x + z (y = −90°) is defined; keep x = 0
        [0.0, y, (-m[0][1]).atan2(m[1][1])]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Mat3, b: &Mat3) -> bool {
        a.iter().flatten().zip(b.iter().flatten()).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn rotations_are_right_handed_and_ordered() {
        let q = std::f32::consts::FRAC_PI_2;
        // about z by 90°: x → y
        let v = apply(&about(2, q), [1.0, 0.0, 0.0]);
        assert!((v[1] - 1.0).abs() < 1e-6 && v[0].abs() < 1e-6);
        // first about x, then about z
        let m = matrix([q, 0.0, q], EulerOrder::Xyz);
        assert!(close(&m, &mul(&about(2, q), &about(0, q))));
        let m = matrix([q, 0.0, q], EulerOrder::Zyx);
        assert!(close(&m, &mul(&about(0, q), &about(2, q))));
    }

    #[test]
    fn angles_round_trip_through_any_order() {
        for order in EulerOrder::ALL {
            for a in [[0.3, -0.7, 1.9], [3.0, 0.0, 1.5], [-2.0, 1.2, 0.1]] {
                let m = matrix(a, order);
                let ours = angles_xyz(&m);
                assert!(close(&matrix(ours, EulerOrder::Xyz), &m), "{order:?} {a:?}");
            }
        }
        let locked = matrix([0.4, std::f32::consts::FRAC_PI_2, 0.2], EulerOrder::Xyz);
        assert!(close(&matrix(angles_xyz(&locked), EulerOrder::Xyz), &locked));
    }
}
