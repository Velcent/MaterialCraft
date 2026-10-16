//! Tangent-space normal vector maths shared by the blend modes and the final material output.
//!
//! Normals are stored encoded (`c = n * 0.5 + 0.5`, OpenGL +Y). These helpers decode, combine and
//! re-encode them.

pub type Vec3 = [f32; 3];

/// The flat normal (0, 0, 1).
pub const FLAT: Vec3 = [0.0, 0.0, 1.0];

/// Encoded colour → unit vector. A degenerate input gives the flat normal.
pub fn decode(c: [f32; 4]) -> Vec3 {
    normalize([c[0] * 2.0 - 1.0, c[1] * 2.0 - 1.0, c[2] * 2.0 - 1.0])
}

/// Unit vector → encoded colour (alpha 1).
pub fn encode(n: Vec3) -> [f32; 4] {
    let n = normalize(n);
    [n[0] * 0.5 + 0.5, n[1] * 0.5 + 0.5, n[2] * 0.5 + 0.5, 1.0]
}

pub fn normalize(v: Vec3) -> Vec3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-8 && l.is_finite() {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        FLAT
    }
}

/// Whiteout blend: `normalize(b.xy + d.xy, b.z * d.z)`. Cheap, adds both slopes.
pub fn whiteout(base: Vec3, detail: Vec3) -> Vec3 {
    normalize([
        base[0] + detail[0],
        base[1] + detail[1],
        base[2] * detail[2],
    ])
}

/// Reoriented normal mapping (Barré-Brisebois & Hill 2012): rotates `detail` so that its "up" follows
/// `base`. `t = base + (0,0,1)`, `u = detail * (-1,-1,1)`, `r = t * dot(t, u) / t.z - u`.
/// A flat base returns `detail`, a flat detail returns `base`.
pub fn reoriented(base: Vec3, detail: Vec3) -> Vec3 {
    let t = [base[0], base[1], base[2] + 1.0];
    let u = [-detail[0], -detail[1], detail[2]];
    if t[2] <= 1e-6 {
        return normalize(detail); // base points straight down: no meaningful rotation
    }
    let k = (t[0] * u[0] + t[1] * u[1] + t[2] * u[2]) / t[2];
    normalize([t[0] * k - u[0], t[1] * k - u[1], t[2] * k - u[2]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5)
    }

    #[test]
    fn flat_is_identity() {
        let n = normalize([0.3, -0.2, 0.9]);
        assert!(close(reoriented(FLAT, n), n));
        assert!(close(reoriented(n, FLAT), n));
        assert!(close(whiteout(FLAT, n), normalize([0.3, -0.2, 0.9])));
        assert!(close(decode(encode(n)), n));
        assert_eq!(decode([0.5, 0.5, 0.5, 1.0]), FLAT);
    }

    #[test]
    fn reoriented_tilts_detail_with_base() {
        let base = normalize([0.5, 0.0, 1.0]);
        let r = reoriented(base, FLAT);
        assert!(close(r, base));
        let d = normalize([0.0, 0.5, 1.0]);
        let r = reoriented(base, d);
        assert!(r[0] > 0.3 && r[1] > 0.3);
    }
}
