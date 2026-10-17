//! Secondary ray directions: a low-discrepancy set over a cone around a direction, turned by a per-texel angle so
//! neighbouring texels don't repeat the same pattern.

use super::settings::RayDistribution;
use crate::v3::*;

/// The i-th of n points of the Hammersley set in the unit square.
pub(crate) fn hammersley(i: u32, n: u32) -> (f32, f32) {
    (i as f32 / n.max(1) as f32, i.reverse_bits() as f32 * 2.328_306_4e-10)
}

/// An orthonormal basis (tangent, bitangent) around the unit vector `n`.
pub(crate) fn basis(n: V) -> (V, V) {
    let up = if n[1].abs() < 0.99 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let t = norm(cross(up, n));
    (t, cross(n, t))
}

/// `count` unit directions around `axis` within `spread` degrees of opening (180 = hemisphere), distributed as
/// asked, rotated about the axis by `turn` (0..1 of a full turn).
pub(crate) struct Cone {
    axis: V,
    t: V,
    b: V,
    count: u32,
    cos_max: f32,
    distribution: RayDistribution,
    turn: f32,
}

impl Cone {
    pub fn new(axis: V, count: u32, spread: f32, distribution: RayDistribution, turn: f32) -> Cone {
        let (t, b) = basis(axis);
        let half = (spread.clamp(0.0, 180.0) * 0.5).to_radians();
        Cone { axis, t, b, count: count.max(1), cos_max: half.cos().max(0.0), distribution, turn }
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// Direction `i` and its cosine to the axis.
    pub fn direction(&self, i: u32) -> (V, f32) {
        let (a, u) = hammersley(i, self.count);
        let phi = (a + self.turn) * std::f32::consts::TAU;
        let cos = match self.distribution {
            // uniform over the solid angle of the cap
            RayDistribution::Uniform => 1.0 - u * (1.0 - self.cos_max),
            // cosine-weighted over the cap: sin² uniform between 0 and sin²(max)
            RayDistribution::Cosine => (1.0 - u * (1.0 - self.cos_max * self.cos_max)).max(0.0).sqrt(),
        };
        let sin = (1.0 - cos * cos).max(0.0).sqrt();
        let d = add(add(mul(self.t, sin * phi.cos()), mul(self.b, sin * phi.sin())), mul(self.axis, cos));
        (d, cos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_stay_in_the_cone() {
        let axis = norm([0.3, -0.2, 0.9]);
        for dist in [RayDistribution::Uniform, RayDistribution::Cosine] {
            let cone = Cone::new(axis, 64, 90.0, dist, 0.3);
            let mut mean = 0.0;
            for i in 0..cone.count() {
                let (d, c) = cone.direction(i);
                assert!((len(d) - 1.0).abs() < 1e-4);
                assert!((dot(d, axis) - c).abs() < 1e-4);
                assert!(c >= 45f32.to_radians().cos() - 1e-4, "{c}");
                mean += c;
            }
            assert!(mean / 64.0 > 0.8);
        }
        let (t, b) = basis(axis);
        assert!(dot(t, axis).abs() < 1e-5 && dot(b, axis).abs() < 1e-5 && dot(t, b).abs() < 1e-5);
    }
}
