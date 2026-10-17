//! The soft edges of volume projections, one curve family for all of them (ADR 0008: one falloff model shaped
//! by the hardness): a smooth step whose width shrinks to nothing at hardness 1.
//!
//! - [`depth`]: along the projection depth (planar: the volume's local z; warp: the projection depth).
//! - [`facing`]: by the angle between the surface normal and the projection direction.
//! - [`tri_planar_weights`]: the three planes' shares by the local normal.

use super::modes::{FacingFalloff, Falloff};

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn hardness(h: f32) -> f32 {
    if h.is_finite() { h.clamp(0.0, 1.0) } else { 0.5 }
}

/// Coverage at signed distance `d` from the middle of a depth of `half` on either side: full within
/// `hardness × half`, fading to 0 at `half` (a hard cut at hardness 1). Off: 1 everywhere.
#[inline]
pub fn depth(f: &Falloff, d: f32, half: f32) -> f32 {
    if !f.on {
        return 1.0;
    }
    let h = hardness(f.hardness);
    1.0 - smoothstep(h * half, half, d.abs())
}

/// Coverage of a surface whose normal makes an angle with cosine `cos` with the direction towards the
/// projector: full up to the cut-off angle less a band, none beyond it plus the band; the band is
/// `(1 − hardness)` × the distance from the angle to 0° or 180°, whichever is nearer. Off: 1 everywhere.
#[inline]
pub fn facing(f: &FacingFalloff, cos: f32) -> f32 {
    if !f.on {
        return 1.0;
    }
    let a = if f.angle.is_finite() { f.angle.clamp(0.0, 180.0) } else { 90.0 };
    let t = cos.clamp(-1.0, 1.0).acos().to_degrees();
    let w = (1.0 - hardness(f.hardness)) * a.min(180.0 - a);
    1.0 - smoothstep(a - w, a + w, t)
}

/// The shares of the planes facing x, y and z for a unit local normal `n` (sum 1): each plane's share is how
/// far its component of `n` exceeds `hardness` × the largest component, so hardness 0 blends by the plain
/// components and hardness 1 keeps only the plane the normal faces most (ties share). A zero normal faces z.
#[inline]
pub fn tri_planar_weights(n: [f32; 3], hardness_value: f32) -> [f32; 3] {
    let a = n.map(f32::abs);
    let m = a[0].max(a[1]).max(a[2]);
    if m <= 1e-12 || !m.is_finite() {
        return [0.0, 0.0, 1.0];
    }
    let h = hardness(hardness_value);
    let floor = h * m;
    let w = a.map(|c| if h >= 1.0 { if c >= m { 1.0 } else { 0.0 } } else { (c - floor).max(0.0) });
    let sum = w[0] + w[1] + w[2];
    if sum <= 0.0 { [0.0, 0.0, 1.0] } else { w.map(|v| v / sum) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_fades_by_hardness() {
        let off = Falloff { on: false, hardness: 1.0 };
        assert_eq!(depth(&off, 9.0, 0.5), 1.0);
        let hard = Falloff { on: true, hardness: 1.0 };
        assert_eq!(depth(&hard, 0.49, 0.5), 1.0);
        assert_eq!(depth(&hard, 0.51, 0.5), 0.0);
        let soft = Falloff { on: true, hardness: 0.5 };
        assert_eq!(depth(&soft, 0.2, 0.5), 1.0);
        let mid = depth(&soft, 0.375, 0.5);
        assert!((mid - 0.5).abs() < 1e-5);
        assert_eq!(depth(&soft, -0.6, 0.5), 0.0);
    }

    #[test]
    fn facing_cuts_at_the_angle() {
        let hard = FacingFalloff { on: true, angle: 90.0, hardness: 1.0 };
        assert_eq!(facing(&hard, 0.1), 1.0);
        assert_eq!(facing(&hard, -0.1), 0.0);
        let soft = FacingFalloff { on: true, angle: 90.0, hardness: 0.5 };
        assert!((facing(&soft, 0.0) - 0.5).abs() < 1e-5);
        assert_eq!(facing(&soft, 1.0), 1.0);
        assert_eq!(facing(&soft, -1.0), 0.0);
    }

    #[test]
    fn tri_planar_weights_sum_to_one_and_sharpen() {
        let n = [0.6, 0.0, 0.8];
        let soft = tri_planar_weights(n, 0.0);
        assert!((soft[0] - 0.6 / 1.4).abs() < 1e-5);
        let mid = tri_planar_weights(n, 0.5);
        assert!((mid[0] - 0.2 / 0.6).abs() < 1e-5 && mid[1] == 0.0);
        assert_eq!(tri_planar_weights(n, 1.0), [0.0, 0.0, 1.0]);
        assert_eq!(tri_planar_weights([0.0; 3], 0.5), [0.0, 0.0, 1.0]);
        for h in [0.0, 0.3, 0.9, 1.0] {
            let w = tri_planar_weights([-0.5, 0.5, 0.7], h);
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        }
    }
}
