// Copied from PhotoCraft, crates/geom/src/warp.rs (`BezierMesh`: its layout, `bernstein`, `locate`, `eval` and
// `param_at`), revision 5f65579, and generalised to 3D control points; derivatives added. See ATTRIBUTION.md.
//
// Copyright (c) 2026 ArtCraft Team and the PhotoCraft contributors.
// Licensed under either of the Apache License, Version 2.0 or the MIT license, at your option.

//! [`BezierMesh`]: a grid of bicubic Bézier patches with 3D control points over the unit square of its
//! parameters `(s, t)`.
//!
//! `us`/`vs` are the patch boundaries in `0..=1` (increasing, first 0 and last 1). The control points are a
//! `(3·(us.len()−1)+1) × (3·(vs.len()−1)+1)` grid in row-major order (rows along `t`); neighbouring patches share
//! their edge row. Indexing never panics: a malformed mesh evaluates to the origin.

/// Cubic Bernstein weights at `t`.
fn bernstein(t: f64) -> [f64; 4] {
    let s = 1.0 - t;
    [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t]
}

/// Derivatives of the cubic Bernstein weights at `t`.
fn bernstein_d(t: f64) -> [f64; 4] {
    let s = 1.0 - t;
    [-3.0 * s * s, 3.0 * s * s - 6.0 * s * t, 6.0 * s * t - 3.0 * t * t, 3.0 * t * t]
}

/// A point on the surface with its partial derivatives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSample {
    pub pos: [f64; 3],
    /// ∂/∂s and ∂/∂t over the whole parameter square (0…1).
    pub ds: [f64; 3],
    pub dt: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct BezierMesh {
    pub us: Vec<f64>,
    pub vs: Vec<f64>,
    pub points: Vec<[f64; 3]>,
}

impl BezierMesh {
    /// Control points per row.
    pub fn nx(&self) -> usize {
        3 * self.us.len().saturating_sub(1) + 1
    }

    /// Control point rows.
    pub fn ny(&self) -> usize {
        3 * self.vs.len().saturating_sub(1) + 1
    }

    /// Checks the invariants (knots and point count).
    pub fn is_valid(&self) -> bool {
        let knots_ok = |k: &[f64]| k.len() >= 2 && k.first() == Some(&0.0) && k.last() == Some(&1.0) && k.windows(2).all(|w| matches!(w, [a, b] if b > a));
        knots_ok(&self.us) && knots_ok(&self.vs) && self.points.len() == self.nx() * self.ny() && self.points.iter().all(|p| p.iter().all(|v| v.is_finite()))
    }

    /// The patch containing `s` and the position inside it (0…1), with the patch's width in `s`.
    fn locate(knots: &[f64], s: f64) -> (usize, f64, f64) {
        let n = knots.len().saturating_sub(1);
        let s = s.clamp(0.0, 1.0);
        let knot = |i: usize| knots.get(i).copied().unwrap_or(1.0);
        let mut i = 0;
        while i + 1 < n && s > knot(i + 1) {
            i += 1;
        }
        let w = knot(i + 1) - knot(i);
        (i, if w > 0.0 { (s - knot(i)) / w } else { 0.0 }, w)
    }

    fn point(&self, i: usize, j: usize) -> [f64; 3] {
        self.points.get(j * self.nx() + i).copied().unwrap_or([0.0; 3])
    }

    /// The surface at `(s, t)` (clamped to the unit square).
    pub fn eval(&self, s: f64, t: f64) -> [f64; 3] {
        self.sample(s, t).pos
    }

    /// The surface and its derivatives at `(s, t)` (clamped to the unit square).
    pub fn sample(&self, s: f64, t: f64) -> SurfaceSample {
        let (pi, a, wa) = Self::locate(&self.us, s);
        let (pj, b, wb) = Self::locate(&self.vs, t);
        let (ba, bb, da, db) = (bernstein(a), bernstein(b), bernstein_d(a), bernstein_d(b));
        let mut out = SurfaceSample { pos: [0.0; 3], ds: [0.0; 3], dt: [0.0; 3] };
        for j in 0..4 {
            for i in 0..4 {
                let p = self.point(3 * pi + i, 3 * pj + j);
                let (wi, wj, wdi, wdj) = (
                    ba.get(i).copied().unwrap_or(0.0),
                    bb.get(j).copied().unwrap_or(0.0),
                    da.get(i).copied().unwrap_or(0.0),
                    db.get(j).copied().unwrap_or(0.0),
                );
                for k in 0..3 {
                    let c = p.get(k).copied().unwrap_or(0.0);
                    if let (Some(o), Some(ds), Some(dt)) = (out.pos.get_mut(k), out.ds.get_mut(k), out.dt.get_mut(k)) {
                        *o += c * wi * wj;
                        *ds += c * wdi * wj;
                        *dt += c * wi * wdj;
                    }
                }
            }
        }
        // per-patch derivatives → over the whole parameter square
        let (ka, kb) = (if wa > 0.0 { 1.0 / wa } else { 0.0 }, if wb > 0.0 { 1.0 / wb } else { 0.0 });
        out.ds = out.ds.map(|v| v * ka);
        out.dt = out.dt.map(|v| v * kb);
        out
    }

    /// Normalised `(s, t)` whose surface point is nearest `p` in x and y (a coarse search over the whole square,
    /// then Newton steps with difference quotients). The slow reference the inverse lookup is tested against.
    pub fn param_at_xy(&self, p: [f64; 2], n: usize) -> (f64, f64) {
        let n = n.max(1);
        let mut best = (0.5, 0.5, f64::MAX);
        for j in 0..=n {
            for i in 0..=n {
                let (s, t) = (i as f64 / n as f64, j as f64 / n as f64);
                let q = self.eval(s, t);
                let d = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2);
                if d < best.2 {
                    best = (s, t, d);
                }
            }
        }
        let (mut s, mut t) = (best.0, best.1);
        for _ in 0..24 {
            let h = 1e-6;
            let (hs, ht) = (if s + h <= 1.0 { h } else { -h }, if t + h <= 1.0 { h } else { -h });
            let q = self.eval(s, t);
            let (qs, qt) = (self.eval(s + hs, t), self.eval(s, t + ht));
            let (a, b, c, d) = ((qs[0] - q[0]) / hs, (qt[0] - q[0]) / ht, (qs[1] - q[1]) / hs, (qt[1] - q[1]) / ht);
            let det = a * d - b * c;
            if det.abs() < 1e-14 {
                break;
            }
            let (ex, ey) = (p[0] - q[0], p[1] - q[1]);
            s = (s + (d * ex - b * ey) / det).clamp(0.0, 1.0);
            t = (t + (a * ey - c * ex) / det).clamp(0.0, 1.0);
        }
        (s, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bilinear patch over −1…1 (x along s, y along t), its control points at the thirds.
    fn flat() -> BezierMesh {
        let mut points = Vec::new();
        for j in 0..4 {
            for i in 0..4 {
                points.push([-1.0 + 2.0 * i as f64 / 3.0, -1.0 + 2.0 * j as f64 / 3.0, 0.0]);
            }
        }
        BezierMesh { us: vec![0.0, 1.0], vs: vec![0.0, 1.0], points }
    }

    #[test]
    fn evaluates_with_derivatives() {
        let m = flat();
        assert!(m.is_valid());
        let s = m.sample(0.25, 0.75);
        assert!((s.pos[0] + 0.5).abs() < 1e-12 && (s.pos[1] - 0.5).abs() < 1e-12);
        assert!((s.ds[0] - 2.0).abs() < 1e-12 && s.ds[1].abs() < 1e-12);
        assert!((s.dt[1] - 2.0).abs() < 1e-12 && s.dt[0].abs() < 1e-12);
        let (u, v) = m.param_at_xy([0.2, -0.4], 16);
        assert!((u - 0.6).abs() < 1e-9 && (v - 0.3).abs() < 1e-9, "{u} {v}");
    }

    #[test]
    fn malformed_meshes_do_not_panic() {
        let m = BezierMesh { us: vec![0.0], vs: Vec::new(), points: Vec::new() };
        assert!(!m.is_valid());
        assert_eq!(m.eval(0.5, 0.5), [0.0; 3]);
    }
}
