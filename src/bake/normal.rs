//! Tangent-space normal map: the high normal written in the low surface's tangent frame, so that a renderer that
//! rebuilds the frame per pixel from interpolated vertex tangents and normals (MikkTSpace style, bitangent
//! `sign * cross(normal, tangent)`) gets the high normal back. OpenGL convention: green points up in the texture.

use super::context::Ctx;
use super::project::Sample;
use crate::v3::*;

/// A flat normal (0, 0, 1).
pub(crate) const FLAT: [f32; 4] = [0.5, 0.5, 1.0, 1.0];

pub(crate) fn bake(ctx: &Ctx) -> Vec<[f32; 4]> {
    ctx.map(FLAT, |_, s| encode(s))
}

fn encode(s: &Sample) -> [f32; 4] {
    let Some(hit) = &s.hit else {
        return FLAT;
    };
    let v = in_frame(s.low.frame, hit.normal);
    [v[0] * 0.5 + 0.5, v[1] * 0.5 + 0.5, v[2] * 0.5 + 0.5, 1.0]
}

/// The unit vector `(x, y, z)` with `normalize(x * t + y * b + z * n) = target` for the frame `[t, b, n]`, which
/// need not be orthonormal (interpolated vectors). Falls back to projecting onto the orthonormalised frame when the
/// frame is degenerate.
pub(crate) fn in_frame([t, b, n]: [V; 3], target: V) -> V {
    let det = dot(t, cross(b, n));
    if det.abs() > 1e-8 {
        // rows of the inverse of the column matrix [t b n]
        let inv = 1.0 / det;
        let x = dot(cross(b, n), target) * inv;
        let y = dot(cross(n, t), target) * inv;
        let z = dot(cross(t, b), target) * inv;
        return norm([x, y, z]);
    }
    let n = norm(n);
    let t = norm(sub(t, mul(n, dot(n, t))));
    let b = cross(n, t);
    norm([dot(target, t), dot(target, b), dot(target, n)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_a_skewed_frame() {
        let frame = [[1.0, 0.2, 0.1], [0.1, 0.9, -0.2], [0.0, 0.1, 1.1]];
        let target = norm([0.3, -0.4, 0.8]);
        let v = in_frame(frame, target);
        let back = norm(add(add(mul(frame[0], v[0]), mul(frame[1], v[1])), mul(frame[2], v[2])));
        assert!(len(sub(back, target)) < 1e-5, "{back:?}");
        // orthonormal frame: plain dot products
        let id = in_frame([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], target);
        assert!(len(sub(id, target)) < 1e-6);
        // degenerate frame still gives a unit vector
        assert!((len(in_frame([[1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], target)) - 1.0).abs() < 1e-5);
    }
}
