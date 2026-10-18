//! [`JointResponse`]: several grey unknowns (layer masks) solved at once per texel from every channel they
//! show in. A mask is shared by all channels its layer writes, so with k unknown masks and at least k channel
//! components that respond differently, all k are determined where the one-unknown read-back
//! ([`super::inverse`]) has to give up.
//!
//! The unknowns `x ∈ [0, 1]^k` are probed on a regular grid ([`nodes_per_axis`]: {0, ¼, ½, ¾, 1} per unknown
//! for up to two unknowns, as the one-unknown read-back does, {0, ½, 1} beyond); per texel and component the
//! response is the
//! tensor-product piecewise-linear interpolation of those composites (exact for normal, multiply, add …
//! blending, which are multilinear in the masks; [`super::joint_measure`] decodes sRGB channels to the linear
//! values they are blended in, so all units here are those values). Two more composites on the diagonal
//! (⅜ and ⅝, inside cells) check the model per texel. A texel is solvable when, from the probes alone (never from Painter's
//! value):
//!
//! - the model reproduces both check composites within [`JointSettings::model_tolerance`];
//! - along every grid line of every unknown the two segments do not point against each other, so the
//!   response does not fold back (the k-dimensional form of the monotone-response test).
//!
//! Painter's value is then solved for by damped Gauss–Newton from the best grid node, clamped to the box. The
//! solution counts where the least singular value of the Jacobian *there* is at least
//! [`JointSettings::min_sensitivity`] (the k-dimensional form of the steep-response test). Unlike the
//! one-unknown case this depends on the solution: a lower mask is invisible wherever an opaque mask above it is
//! white, so identifiability is local by nature. The residual is only counted ([`JointSolution::outside`]).

use super::inverse::STEP;

/// The check points on the diagonal.
pub const CHECKS: [f32; 2] = [0.375, 0.625];
/// At most this many unknowns (3^4 composites).
pub const MAX_UNKNOWNS: usize = 4;
/// Segments along a grid line whose dot product is more negative than this (squared units) fold back.
const FOLD_TOLERANCE: f32 = 1e-4;

/// Probe values per unknown: 5 for up to two unknowns (curved responses such as a levels gamma after the mask
/// are followed closely), 3 beyond (3^k composites).
pub fn nodes_per_axis(k: usize) -> usize {
    if k <= 2 { 5 } else { 3 }
}

/// The value of node `d` of `n` on one axis.
fn node_value(d: usize, n: usize) -> f32 {
    d as f32 / (n - 1).max(1) as f32
}

/// The values of grid node `index` (mixed radix `n`, unknown 0 the fastest).
fn node_point(index: usize, k: usize, n: usize) -> Vec<f32> {
    let mut rest = index;
    (0..k)
        .map(|_| {
            let d = rest % n;
            rest /= n;
            node_value(d, n)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointSettings {
    /// Least singular value of the response's Jacobian (stored units per unit of the unknowns).
    pub min_sensitivity: f32,
    /// Largest difference between a check composite and the model, per component.
    pub model_tolerance: f32,
    /// Residual (root mean square over the components) above which a solved texel counts as outside.
    pub outside: f32,
}

impl Default for JointSettings {
    fn default() -> Self {
        JointSettings {
            min_sensitivity: 64.0 * STEP,
            model_tolerance: 1.5 * STEP,
            outside: 1.5 * STEP,
        }
    }
}

/// Why a texel was or was not solved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointState {
    Solved,
    /// The model misses a check composite (a curved response).
    Model,
    /// Some direction of the unknowns hardly changes the composite.
    IllConditioned,
    /// The response folds back.
    Folded,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JointSolution {
    /// Per texel the k unknowns (meaningful where solved).
    pub values: Vec<Vec<f32>>,
    pub states: Vec<JointState>,
    /// Solved texels whose residual exceeded [`JointSettings::outside`].
    pub outside: Vec<bool>,
}

impl JointSolution {
    pub fn solved(&self, i: usize) -> bool {
        self.states.get(i) == Some(&JointState::Solved)
    }

    pub fn count(&self, state: JointState) -> usize {
        self.states.iter().filter(|s| **s == state).count()
    }
}

/// The composites on the probe grid: per grid node, per texel `components` values; plus the two checks.
#[derive(Debug, Clone)]
pub struct JointResponse {
    k: usize,
    /// Nodes per axis.
    n: usize,
    components: usize,
    texels: usize,
    /// `nodes[i][texel * components + j]`, `i` in mixed radix `n` (unknown 0 the fastest).
    nodes: Vec<Vec<f32>>,
    checks: Vec<Vec<f32>>,
}

impl JointResponse {
    /// An empty response for `k` unknowns, `components` observed values per texel, `texels` texels.
    pub fn new(k: usize, components: usize, texels: usize) -> JointResponse {
        let k = k.clamp(1, MAX_UNKNOWNS);
        let n = nodes_per_axis(k);
        JointResponse {
            k,
            n,
            components,
            texels,
            nodes: vec![Vec::new(); n.pow(k as u32)],
            checks: vec![Vec::new(); CHECKS.len()],
        }
    }

    pub fn unknowns(&self) -> usize {
        self.k
    }

    /// The grid nodes as values of the unknowns, in node order.
    pub fn node_points(&self) -> Vec<Vec<f32>> {
        (0..self.nodes.len()).map(|i| node_point(i, self.k, self.n)).collect()
    }

    /// The check points.
    pub fn check_points(&self) -> Vec<Vec<f32>> {
        CHECKS.iter().map(|c| vec![*c; self.k]).collect()
    }

    /// Sets the composite of node `n` (`texels * components` values).
    pub fn set_node(&mut self, n: usize, values: Vec<f32>) {
        if let Some(slot) = self.nodes.get_mut(n) {
            *slot = values;
        }
    }

    pub fn set_check(&mut self, i: usize, values: Vec<f32>) {
        if let Some(slot) = self.checks.get_mut(i) {
            *slot = values;
        }
    }

    fn complete(&self) -> bool {
        let len = self.texels * self.components;
        self.nodes.iter().chain(&self.checks).all(|v| v.len() == len)
    }

    /// Solves every texel of `observed` (`texels * components` values).
    pub fn solve(&self, observed: &[f32], settings: &JointSettings) -> JointSolution {
        let mut out = JointSolution {
            values: vec![vec![0.0; self.k]; self.texels],
            states: vec![JointState::IllConditioned; self.texels],
            outside: vec![false; self.texels],
        };
        if !self.complete() || observed.len() != self.texels * self.components {
            return out;
        }
        let mut texel = TexelModel::new(self.k, self.n, self.components);
        for i in 0..self.texels {
            texel.load(self, i);
            let state = texel.state(self, i, settings);
            out.states[i] = state;
            if state != JointState::Solved {
                continue;
            }
            let y = &observed[i * self.components..(i + 1) * self.components];
            let (x, residual) = texel.solve(y);
            if texel.sensitivity(&x) < settings.min_sensitivity {
                out.states[i] = JointState::IllConditioned;
                continue;
            }
            out.outside[i] = residual > settings.outside;
            out.values[i] = x;
        }
        out
    }
}

/// One texel's grid values, reused across texels.
struct TexelModel {
    k: usize,
    /// Nodes per axis.
    n: usize,
    m: usize,
    /// `values[n * m + j]`.
    values: Vec<f32>,
}

impl TexelModel {
    fn new(k: usize, n: usize, m: usize) -> TexelModel {
        TexelModel {
            k,
            n,
            m,
            values: vec![0.0; n.pow(k as u32) * m],
        }
    }

    fn load(&mut self, r: &JointResponse, i: usize) {
        for (n, node) in r.nodes.iter().enumerate() {
            self.values[n * self.m..(n + 1) * self.m].copy_from_slice(&node[i * self.m..(i + 1) * self.m]);
        }
    }

    /// The cell of `x` per axis (0 .. n − 2) and the local coordinate in it.
    fn cell(&self, x: &[f32]) -> (Vec<usize>, Vec<f32>) {
        let cells = (self.n - 1).max(1);
        x.iter()
            .map(|v| {
                let at = v.clamp(0.0, 1.0) * cells as f32;
                let c = (at.floor() as usize).min(cells - 1);
                (c, at - c as f32)
            })
            .unzip()
    }

    /// The model and its Jacobian (`jac[a * m + j]`) at `x`.
    fn eval(&self, x: &[f32], f: &mut [f32], jac: &mut [f32]) {
        let (cell, t) = self.cell(x);
        let scale = (self.n - 1).max(1) as f32;
        f.iter_mut().for_each(|v| *v = 0.0);
        jac.iter_mut().for_each(|v| *v = 0.0);
        for corner in 0..(1usize << self.k) {
            let mut node = 0;
            let mut stride = 1;
            let mut weight = 1.0f32;
            let mut partial = vec![1.0f32; self.k];
            for a in 0..self.k {
                let bit = (corner >> a) & 1;
                node += (cell[a] + bit) * stride;
                stride *= self.n;
                let w = if bit == 1 { t[a] } else { 1.0 - t[a] };
                let dw = if bit == 1 { scale } else { -scale };
                weight *= w;
                for (b, p) in partial.iter_mut().enumerate() {
                    *p *= if a == b { dw } else { w };
                }
            }
            let v = &self.values[node * self.m..(node + 1) * self.m];
            for j in 0..self.m {
                f[j] += weight * v[j];
                for a in 0..self.k {
                    jac[a * self.m + j] += partial[a] * v[j];
                }
            }
        }
    }

    /// Whether the model holds and the response does not fold back, from the probes only.
    fn state(&self, r: &JointResponse, i: usize, settings: &JointSettings) -> JointState {
        let mut f = vec![0.0; self.m];
        let mut jac = vec![0.0; self.k * self.m];
        for (c, point) in r.check_points().iter().enumerate() {
            self.eval(point, &mut f, &mut jac);
            let check = &r.checks[c][i * self.m..(i + 1) * self.m];
            if f.iter()
                .zip(check)
                .any(|(a, b)| (a - b).abs() > settings.model_tolerance)
            {
                return JointState::Model;
            }
        }
        // along every grid line of every unknown no two segments may point against each other
        let nodes = self.n.pow(self.k as u32);
        let v = |node: usize| &self.values[node * self.m..(node + 1) * self.m];
        for a in 0..self.k {
            let stride = self.n.pow(a as u32);
            for start in (0..nodes).filter(|i| (i / stride).is_multiple_of(self.n)) {
                for d in 0..self.n - 1 {
                    let (p0, p1) = (v(start + d * stride), v(start + (d + 1) * stride));
                    for e in d + 1..self.n - 1 {
                        let (q0, q1) = (v(start + e * stride), v(start + (e + 1) * stride));
                        let dot: f32 = (0..self.m).map(|j| (p1[j] - p0[j]) * (q1[j] - q0[j])).sum();
                        if dot < -FOLD_TOLERANCE {
                            return JointState::Folded;
                        }
                    }
                }
            }
        }
        JointState::Solved
    }

    /// The least singular value of the Jacobian at `x`.
    fn sensitivity(&self, x: &[f32]) -> f32 {
        let mut f = vec![0.0; self.m];
        let mut jac = vec![0.0; self.k * self.m];
        self.eval(x, &mut f, &mut jac);
        min_eigen(&product(&jac, &jac, self.k, self.m), self.k).max(0.0).sqrt()
    }

    /// Damped Gauss–Newton from the best grid node; the solution and its residual (RMS).
    fn solve(&self, y: &[f32]) -> (Vec<f32>, f32) {
        let nodes = self.n.pow(self.k as u32);
        let sq = |n: usize| -> f32 { (0..self.m).map(|j| (self.values[n * self.m + j] - y[j]).powi(2)).sum() };
        let best = (0..nodes).min_by(|a, b| sq(*a).total_cmp(&sq(*b))).unwrap_or(0);
        let mut x = node_point(best, self.k, self.n);
        let mut f = vec![0.0; self.m];
        let mut jac = vec![0.0; self.k * self.m];
        let cost = |f: &[f32]| -> f32 { f.iter().zip(y).map(|(a, b)| (a - b).powi(2)).sum() };
        self.eval(&x, &mut f, &mut jac);
        let mut current = cost(&f);
        let mut lambda = 1e-3f32;
        for _ in 0..30 {
            let mut gram = product(&jac, &jac, self.k, self.m);
            let grad: Vec<f32> = (0..self.k)
                .map(|a| (0..self.m).map(|j| jac[a * self.m + j] * (f[j] - y[j])).sum())
                .collect();
            for a in 0..self.k {
                gram[a * self.k + a] *= 1.0 + lambda;
                gram[a * self.k + a] += 1e-9;
            }
            let Some(step) = solve_small(&gram, &grad, self.k) else {
                break;
            };
            let trial: Vec<f32> = x.iter().zip(&step).map(|(v, s)| (v - s).clamp(0.0, 1.0)).collect();
            let mut tf = vec![0.0; self.m];
            let mut tj = vec![0.0; self.k * self.m];
            self.eval(&trial, &mut tf, &mut tj);
            let c = cost(&tf);
            if c < current {
                let moved = trial.iter().zip(&x).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
                x = trial;
                f = tf;
                jac = tj;
                current = c;
                lambda = (lambda * 0.3).max(1e-6);
                if moved < 1e-5 {
                    break;
                }
            } else {
                lambda *= 10.0;
                if lambda > 1e6 {
                    break;
                }
            }
        }
        (x, (current / self.m.max(1) as f32).sqrt())
    }
}

/// `Aᵀ B` for two k × m (row per unknown) matrices.
fn product(a: &[f32], b: &[f32], k: usize, m: usize) -> Vec<f32> {
    let mut out = vec![0.0; k * k];
    for p in 0..k {
        for q in 0..k {
            out[p * k + q] = (0..m).map(|j| a[p * m + j] * b[q * m + j]).sum();
        }
    }
    out
}

/// Least eigenvalue of a symmetric k × k matrix (k ≤ 3), by Jacobi rotations.
fn min_eigen(s: &[f32], k: usize) -> f32 {
    let mut a: Vec<f64> = s.iter().map(|v| f64::from(*v)).collect();
    for _ in 0..32 {
        let mut off = 0.0;
        for p in 0..k {
            for q in p + 1..k {
                off += a[p * k + q] * a[p * k + q];
            }
        }
        if off < 1e-24 {
            break;
        }
        for p in 0..k {
            for q in p + 1..k {
                let apq = a[p * k + q];
                if apq.abs() < 1e-30 {
                    continue;
                }
                let theta = (a[q * k + q] - a[p * k + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for r in 0..k {
                    let arp = a[r * k + p];
                    let arq = a[r * k + q];
                    a[r * k + p] = c * arp - s * arq;
                    a[r * k + q] = s * arp + c * arq;
                }
                for r in 0..k {
                    let apr = a[p * k + r];
                    let aqr = a[q * k + r];
                    a[p * k + r] = c * apr - s * aqr;
                    a[q * k + r] = s * apr + c * aqr;
                }
            }
        }
    }
    let min = (0..k).map(|i| a[i * k + i]).fold(f64::INFINITY, f64::min);
    if min.is_finite() { min as f32 } else { 0.0 }
}

/// `x` with `A x = b` for a small system (Gaussian elimination with pivoting); `None` when singular.
fn solve_small(a: &[f32], b: &[f32], k: usize) -> Option<Vec<f32>> {
    let mut m: Vec<f64> = a.iter().map(|v| f64::from(*v)).collect();
    let mut r: Vec<f64> = b.iter().map(|v| f64::from(*v)).collect();
    for col in 0..k {
        let pivot = (col..k).max_by(|x, y| m[x * k + col].abs().total_cmp(&m[y * k + col].abs()))?;
        if m[pivot * k + col].abs() < 1e-20 {
            return None;
        }
        if pivot != col {
            for c in 0..k {
                m.swap(pivot * k + c, col * k + c);
            }
            r.swap(pivot, col);
        }
        for row in col + 1..k {
            let f = m[row * k + col] / m[col * k + col];
            for c in col..k {
                m[row * k + c] -= f * m[col * k + c];
            }
            r[row] -= f * r[col];
        }
    }
    let mut x = vec![0.0f64; k];
    for row in (0..k).rev() {
        let s: f64 = (row + 1..k).map(|c| m[row * k + c] * x[c]).sum();
        x[row] = (r[row] - s) / m[row * k + row];
    }
    x.iter()
        .all(|v| v.is_finite())
        .then(|| x.iter().map(|v| *v as f32).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response built from `f(x) -> components` at the probe grid and checks, for one texel per entry.
    type Model<'a> = &'a dyn Fn(&[f32]) -> Vec<f32>;

    fn response(k: usize, m: usize, fs: &[Model]) -> JointResponse {
        let mut r = JointResponse::new(k, m, fs.len());
        for (n, p) in r.node_points().iter().enumerate() {
            r.set_node(n, fs.iter().flat_map(|f| f(p)).collect());
        }
        for (c, p) in r.check_points().iter().enumerate() {
            r.set_check(c, fs.iter().flat_map(|f| f(p)).collect());
        }
        r
    }

    #[test]
    fn two_masks_under_normal_blending_are_solved_from_two_channels() {
        // layer A (mask a) and B above it (mask b) over a base, writing colour and roughness differently
        let lerp = |below: f32, top: f32, t: f32| below + (top - below) * t;
        let f = |x: &[f32]| {
            let (a, b) = (x[0], x[1]);
            vec![
                lerp(lerp(0.1, 0.9, a), 0.2, b),
                lerp(lerp(0.5, 0.1, a), 0.95, b),
                lerp(lerp(0.3, 0.3, a), 0.8, b),
            ]
        };
        let r = response(2, 3, &[&f]);
        let truth = [0.37, 0.62];
        let s = r.solve(&f(&truth), &JointSettings::default());
        assert!(s.solved(0), "{:?}", s.states);
        assert!(
            (s.values[0][0] - truth[0]).abs() < 1e-3 && (s.values[0][1] - truth[1]).abs() < 1e-3,
            "{:?}",
            s.values
        );
        assert!(!s.outside[0]);
    }

    #[test]
    fn dependent_curved_or_folded_responses_are_refused() {
        // both masks act only through their sum: no way to tell them apart
        let same = |x: &[f32]| vec![0.1 + 0.4 * (x[0] + x[1]), 0.9 - 0.35 * (x[0] + x[1])];
        // a curve the grid cannot follow
        let curved = |x: &[f32]| vec![x[0].powf(3.0), x[1].powf(0.3)];
        // goes up and back down in x0
        let folded = |x: &[f32]| vec![1.0 - (2.0 * x[0] - 1.0).abs(), x[1]];
        let r = response(2, 2, &[&same, &curved, &folded]);
        let s = r.solve(&[0.5; 6], &JointSettings::default());
        assert_eq!(s.states[0], JointState::IllConditioned);
        assert_eq!(s.states[1], JointState::Model);
        assert_eq!(s.states[2], JointState::Folded);
        assert_eq!(s.count(JointState::Solved), 0);
    }

    #[test]
    fn small_linear_algebra() {
        assert!((min_eigen(&[2.0, 1.0, 1.0, 2.0], 2) - 1.0).abs() < 1e-5);
        assert!((min_eigen(&[4.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 9.0], 3) - 1.0).abs() < 1e-5);
        let x = solve_small(&[2.0, 1.0, 1.0, 3.0], &[3.0, 5.0], 2).unwrap();
        assert!((x[0] - 0.8).abs() < 1e-5 && (x[1] - 1.4).abs() < 1e-5);
        assert!(solve_small(&[1.0, 2.0, 2.0, 4.0], &[1.0, 1.0], 2).is_none());
        // one unknown: an observation outside the response is clamped and counted
        let r = response(1, 1, &[&|x: &[f32]| vec![0.2 + 0.6 * x[0]]]);
        let s = r.solve(&[0.95], &JointSettings::default());
        assert!(s.solved(0) && s.outside[0] && (s.values[0][0] - 1.0).abs() < 1e-6);
    }
}
