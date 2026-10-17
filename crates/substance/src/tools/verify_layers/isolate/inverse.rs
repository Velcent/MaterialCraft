//! [`Probes`]: recovers what one effect wrote at each texel from a composite that depends on it.
//!
//! The rest of the stack is fixed; the effect's output `v` (0..1) is replaced by constants `v₀ < v₁ < …` and
//! the stack composited each time, which gives per texel and component the response `F(vₖ)`. Painter's final
//! value `c` is then read back through the piecewise-linear response: the `v` with `F(v) = c`. Every blend mode
//! that is monotone in its top value (normal, multiply, add, subtract, screen …, and the sRGB encoding on top)
//! gives a monotone response; the probes find out per texel, so no blend formula is inverted by hand.
//!
//! A texel is usable when the response is monotone and steep enough: `|F(1) − F(0)|` of at least
//! [`InversionSettings::min_sensitivity`] (a mask at 5 % opacity or a layer whose content equals what is below
//! says almost nothing about the mask). The decision depends on the probes only, never on Painter's value, so
//! it does not select texels by the value being measured. Values outside the response's range are clamped and
//! counted ([`Inversion::outside`]): many of them mean the rest of the stack does not match Painter there.

/// One 8-bit step of the stored encoding.
pub const STEP: f32 = 1.0 / 255.0;

/// How the components of a texel relate to the unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coupling {
    /// One grey unknown drives every component (a mask): solved jointly, weighted by each component's slope.
    Joint,
    /// Each component has its own unknown (a colour fill): solved one by one.
    PerComponent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InversionSettings {
    /// Least change of the composite over the whole 0..1 range of the unknown (stored units) for a texel to be
    /// solved. 64 steps: one step of Painter's 8-bit value moves the solution by at most 1/64.
    pub min_sensitivity: f32,
    /// Changes below this count as none (float noise of the compositor).
    pub tolerance: f32,
}

impl Default for InversionSettings {
    fn default() -> Self {
        InversionSettings {
            min_sensitivity: 64.0 * STEP,
            tolerance: 0.25 * STEP,
        }
    }
}

/// Why a texel was or was not solved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexelState {
    Solved,
    /// The composite hardly changes with the unknown here.
    Insensitive,
    /// The response goes up and down: no unique solution.
    NotMonotone,
}

/// The solution: per texel the unknown (one value repeated for [`Coupling::Joint`]) and its state.
#[derive(Debug, Clone, PartialEq)]
pub struct Inversion {
    pub values: Vec<[f32; 3]>,
    pub states: Vec<TexelState>,
    /// Solved texels whose observed value lay more than a step outside the response (clamped).
    pub outside: Vec<bool>,
}

impl Inversion {
    pub fn solved(&self, i: usize) -> bool {
        self.states.get(i) == Some(&TexelState::Solved)
    }

    pub fn count(&self, state: TexelState) -> usize {
        self.states.iter().filter(|s| **s == state).count()
    }
}

/// The composites of one stack for several values of the unknown, at the same texels.
#[derive(Debug, Clone, Default)]
pub struct Probes {
    /// Ascending values of the unknown with their frames (per texel, first `components` used).
    samples: Vec<(f32, Vec<[f32; 3]>)>,
    components: usize,
}

/// One component of one texel.
struct ComponentSolve {
    value: f32,
    sensitivity: f32,
    monotone: bool,
    outside: bool,
}

impl Probes {
    pub fn new(components: usize) -> Probes {
        Probes {
            samples: Vec::new(),
            components: components.clamp(1, 3),
        }
    }

    /// Adds the composite made with the unknown set to `value`.
    pub fn push(&mut self, value: f32, frame: Vec<[f32; 3]>) {
        let at = self.samples.partition_point(|(v, _)| *v < value);
        self.samples.insert(at, (value, frame));
    }

    pub fn values(&self) -> Vec<f32> {
        self.samples.iter().map(|(v, _)| *v).collect()
    }

    /// The frame made with `value`, if probed.
    pub fn frame(&self, value: f32) -> Option<&[[f32; 3]]> {
        self.samples
            .iter()
            .find(|(v, _)| *v == value)
            .map(|(_, f)| f.as_slice())
    }

    /// Solves every texel of `observed` (same layout as the frames).
    pub fn invert(&self, observed: &[[f32; 3]], coupling: Coupling, settings: &InversionSettings) -> Inversion {
        let n = observed.len();
        let mut out = Inversion {
            values: vec![[0.0; 3]; n],
            states: vec![TexelState::Insensitive; n],
            outside: vec![false; n],
        };
        if self.samples.len() < 2 || self.samples.iter().any(|(_, f)| f.len() != n) {
            return out;
        }
        let xs = self.values();
        let mut ys = vec![0.0f32; xs.len()];
        for (i, c) in observed.iter().enumerate() {
            let mut solves = Vec::with_capacity(self.components);
            for k in 0..self.components {
                for (y, (_, frame)) in ys.iter_mut().zip(&self.samples) {
                    *y = frame[i][k];
                }
                solves.push(solve(&xs, &ys, c[k], settings.tolerance));
            }
            let (value, state, outside) = match coupling {
                Coupling::Joint => joint(&solves, settings),
                Coupling::PerComponent => per_component(&solves, settings),
            };
            out.values[i] = value;
            out.states[i] = state;
            out.outside[i] = outside && state == TexelState::Solved;
        }
        out
    }
}

fn joint(solves: &[ComponentSolve], settings: &InversionSettings) -> ([f32; 3], TexelState, bool) {
    let total = solves.iter().map(|s| s.sensitivity * s.sensitivity).sum::<f32>().sqrt();
    if total < settings.min_sensitivity {
        return ([0.0; 3], TexelState::Insensitive, false);
    }
    // A component that barely moves says nothing; one that moves both ways makes the texel ambiguous.
    let useful = |s: &&ComponentSolve| s.sensitivity >= settings.tolerance * 4.0;
    if solves.iter().filter(useful).any(|s| !s.monotone) {
        return ([0.0; 3], TexelState::NotMonotone, false);
    }
    let (mut sum, mut weight, mut outside) = (0.0f32, 0.0f32, false);
    for s in solves.iter().filter(useful) {
        let w = s.sensitivity * s.sensitivity;
        sum += s.value * w;
        weight += w;
        outside |= s.outside;
    }
    let v = if weight > 0.0 { sum / weight } else { 0.0 };
    ([v; 3], TexelState::Solved, outside)
}

fn per_component(solves: &[ComponentSolve], settings: &InversionSettings) -> ([f32; 3], TexelState, bool) {
    let mut value = [0.0; 3];
    let mut outside = false;
    for (k, s) in solves.iter().enumerate() {
        if s.sensitivity < settings.min_sensitivity {
            return ([0.0; 3], TexelState::Insensitive, false);
        }
        if !s.monotone {
            return ([0.0; 3], TexelState::NotMonotone, false);
        }
        value[k] = s.value;
        outside |= s.outside;
    }
    if solves.len() == 1 {
        value = [value[0]; 3];
    }
    (value, TexelState::Solved, outside)
}

/// The `x` where the piecewise-linear curve through `(xs, ys)` reaches `c`.
fn solve(xs: &[f32], ys: &[f32], c: f32, tolerance: f32) -> ComponentSolve {
    let (first, last) = (ys[0], ys[ys.len() - 1]);
    let sensitivity = (last - first).abs();
    let rising = last >= first;
    let monotone = ys.windows(2).all(|w| match rising {
        true => w[1] - w[0] >= -tolerance,
        false => w[0] - w[1] >= -tolerance,
    });
    let (lo, hi) = if rising { (first, last) } else { (last, first) };
    let outside = c < lo - STEP || c > hi + STEP;
    let c = c.clamp(lo, hi);
    let mut value = if rising == (c <= first) {
        xs[0]
    } else {
        xs[xs.len() - 1]
    };
    for (i, w) in ys.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        if (a.min(b)..=a.max(b)).contains(&c) && (b - a).abs() > f32::EPSILON {
            value = xs[i] + (c - a) / (b - a) * (xs[i + 1] - xs[i]);
            break;
        }
    }
    ComponentSolve {
        value: if value.is_finite() { value } else { 0.0 },
        sensitivity: if sensitivity.is_finite() { sensitivity } else { 0.0 },
        monotone,
        outside,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALUES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

    /// Probes of a response `f` (per component) at the usual values, for the given texels' parameters.
    fn probes(params: &[f32], f: impl Fn(f32, f32) -> [f32; 3]) -> Probes {
        let mut p = Probes::new(3);
        for v in VALUES.iter().rev() {
            p.push(*v, params.iter().map(|q| f(*v, *q)).collect());
        }
        p
    }

    #[test]
    fn a_mask_under_normal_blending_is_read_back() {
        // final = lerp(below, top, v) per component, below/top differ per texel
        let p = probes(&[0.0, 1.0], |v, q| {
            let (below, top) = ([0.2, 0.3, 0.9], [0.8 * q + 0.1, 0.9, 0.1]);
            [0, 1, 2].map(|k| below[k] + (top[k] - below[k]) * v)
        });
        let truth = [0.37, 0.81];
        let observed: Vec<[f32; 3]> = (0..2)
            .map(|i| {
                let q = i as f32;
                let (below, top) = ([0.2, 0.3, 0.9], [0.8 * q + 0.1, 0.9, 0.1]);
                [0, 1, 2].map(|k| below[k] + (top[k] - below[k]) * truth[i])
            })
            .collect();
        let inv = p.invert(&observed, Coupling::Joint, &InversionSettings::default());
        for (i, t) in truth.iter().enumerate() {
            assert!(inv.solved(i));
            assert!((inv.values[i][0] - t).abs() < 1e-4, "{:?}", inv.values[i]);
        }
        assert_eq!(p.values(), VALUES);
    }

    #[test]
    fn a_curved_response_is_followed_and_flat_or_folded_ones_refused() {
        // sRGB-like encode of a linear mix: monotone, not linear
        let p = probes(&[0.0, 1.0, 2.0], |v, q| match q as u8 {
            0 => [(0.1 + 0.8 * v).powf(1.0 / 2.2); 3],
            1 => [0.5 + 0.1 * STEP * v; 3],
            _ => [(v - 0.5).abs(); 3],
        });
        let observed = vec![[(0.1f32 + 0.8 * 0.6).powf(1.0 / 2.2); 3], [0.5; 3], [0.2; 3]];
        let inv = p.invert(&observed, Coupling::Joint, &InversionSettings::default());
        assert!(
            inv.solved(0) && (inv.values[0][0] - 0.6).abs() < 0.02,
            "{:?}",
            inv.values[0]
        );
        assert_eq!(inv.states[1], TexelState::Insensitive);
        assert_eq!(inv.states[2], TexelState::Insensitive, "folded: equal ends");
        let folded = probes(&[0.0], |v, _| [if v < 0.6 { v } else { 1.2 - v * 0.4 }; 3]);
        let inv = folded.invert(&[[0.5; 3]], Coupling::Joint, &InversionSettings::default());
        assert_eq!(inv.states[0], TexelState::NotMonotone);
    }

    #[test]
    fn colours_solve_per_component_and_values_outside_are_clamped() {
        // multiply: final = below × v per component
        let p = probes(&[0.0], |v, _| [0.9 * v, 0.6 * v, 0.3 * v]);
        let inv = p.invert(
            &[[0.45, 0.06, 0.3]],
            Coupling::PerComponent,
            &InversionSettings::default(),
        );
        let v = inv.values[0];
        assert!(inv.solved(0));
        assert!(
            (v[0] - 0.5).abs() < 1e-4 && (v[1] - 0.1).abs() < 1e-4 && (v[2] - 1.0).abs() < 1e-4,
            "{v:?}"
        );
        let inv = p.invert(
            &[[0.95, 0.06, 0.3]],
            Coupling::PerComponent,
            &InversionSettings::default(),
        );
        assert!(inv.outside[0] && (inv.values[0][0] - 1.0).abs() < 1e-6);
        // a component too flat to read makes the colour unsolved
        let weak = probes(&[0.0], |v, _| [0.9 * v, 0.6 * v, 0.01 * v]);
        let inv = weak.invert(
            &[[0.45, 0.06, 0.0]],
            Coupling::PerComponent,
            &InversionSettings::default(),
        );
        assert_eq!(inv.states[0], TexelState::Insensitive);
        assert!(
            !Probes::new(1)
                .invert(&[[0.0; 3]], Coupling::Joint, &InversionSettings::default())
                .solved(0)
        );
    }
}
