//! [`JointMeasurer`]: runs one [`JointPlan`]: composites every observed channel on the probe grid of the
//! unknown masks ([`JointResponse`]), separates the other unknowns by texels as [`super::measure`] does, and
//! solves Painter's channels and, with a replacement, ours for the candidate's mask.

use super::frames::{Frame, FrameGrid};
use super::inverse::STEP;
use super::joint_plan::{JointPlan, components};
use super::joint_solve::{JointResponse, JointSettings, JointSolution, JointState};
use super::measure::{Measurement, TexelStats};
use super::slots::Nuisance;
use super::substitute::{Swap, apply};
use crate::model::PainterTextureSet;
use crate::translate::layers::{Computability, SetChannels};
use crate::translate::replacements::{Choice, ProceduralPlan};
use materialcraft::paint::working_space::srgb_to_linear;
use materialcraft::paint::{Channel, ColorSpace};
use materialcraft::tex::Tex;

/// Imports a (swapped) Painter set once and composites several channels.
pub trait MultiComposer {
    /// Per channel the final look at the set's resolution and the import's verdict.
    fn composite_channels(
        &self,
        painter: &PainterTextureSet,
        procedurals: &ProceduralPlan,
        channels: &[Channel],
    ) -> Vec<(Tex, Computability)>;
}

/// The grey value the other masks get when our replacement is composited (anything works: the joint
/// read-back recovers them too).
const OURS_MASK_VALUE: f32 = 0.0;

pub struct JointMeasurer<'a> {
    pub composer: &'a dyn MultiComposer,
    pub painter: &'a PainterTextureSet,
    pub channels: &'a SetChannels,
    pub grid: &'a FrameGrid,
    pub settings: JointSettings,
}

impl JointMeasurer<'_> {
    /// `observed`: Painter's final look of each of the plan's channels on the grid.
    pub fn measure(&self, plan: &JointPlan, observed: &[Frame], with_ours: bool) -> Measurement {
        let mut stats = TexelStats::default();
        let mut notes = Vec::new();
        let texels = self.grid.size.0 * self.grid.size.1;
        let k = plan.unknowns();
        let m = plan.components();
        let others = plan.others.clone();
        let base = vec![0usize; others.len()];
        let keep = ProceduralPlan::default();
        let mut response = JointResponse::new(k, m, texels);
        let points = response.node_points();
        let is_corner = |p: &[f32]| p.iter().all(|v| *v == 0.0 || *v == 1.0);
        // the corners first: when no texel is clear of the other unknowns, the rest of the grid is not needed
        let mut corners: Vec<(usize, Vec<f32>)> = Vec::new();
        for (n, point) in points.iter().enumerate().filter(|(_, p)| is_corner(p)) {
            let (frames, _) = self.frames(plan, Some(point), &base, &keep, &mut stats);
            // stored values: independence is judged in steps of the stored encoding
            corners.push((n, pack(&frames, &plan.channels, &[])));
            response.set_node(n, self.pack(&frames, plan));
        }
        let clean = self.independence(plan, &response, &corners, &mut stats);
        let valid = self.grid.valid();
        let any_clean = clean.iter().zip(&valid).any(|(c, v)| *c && *v);
        if any_clean {
            for (n, point) in points.iter().enumerate().filter(|(_, p)| !is_corner(p)) {
                let (frames, _) = self.frames(plan, Some(point), &base, &keep, &mut stats);
                response.set_node(n, self.pack(&frames, plan));
            }
            for (c, point) in response.check_points().iter().enumerate() {
                let (frames, _) = self.frames(plan, Some(point), &base, &keep, &mut stats);
                response.set_check(c, self.pack(&frames, plan));
            }
        } else {
            notes.push("no texel clear of the other unknowns: the rest of the probe grid was skipped".into());
        }
        // an incomplete grid solves nothing
        let theirs = response.solve(&self.pack(observed, plan), &self.settings);
        let ours = match (with_ours && any_clean, plan.candidate.ours) {
            (true, Some(_)) => {
                let replace = ProceduralPlan::all(Choice::Replace);
                let (frames, verdicts) = self.frames(plan, None, &base, &replace, &mut stats);
                match verdicts
                    .iter()
                    .find(|v| !matches!(v, Computability::Replaced(_) | Computability::Exact))
                {
                    None => Some(response.solve(&self.pack(&frames, plan), &self.settings)),
                    Some(other) => {
                        notes.push(format!(
                            "our replacement did not import as replaced: {:?}",
                            other.reasons()
                        ));
                        None
                    }
                }
            }
            _ => None,
        };
        notes.push(format!(
            "joint read-back: {k} unknown masks over {} ({m} components); theirs: {} solved, {} model, {} ill-conditioned, {} folded",
            plan.channels.iter().map(|c| c.id()).collect::<Vec<_>>().join("+"),
            theirs.count(JointState::Solved),
            theirs.count(JointState::Model),
            theirs.count(JointState::IllConditioned),
            theirs.count(JointState::Folded),
        ));
        let mask = self.compared(&clean, &theirs, ours.as_ref(), &mut stats);
        let size = self.grid.size;
        Measurement {
            theirs: tex(&theirs, size),
            ours: ours.as_ref().map(|o| tex(o, size)),
            mask,
            components: 1,
            stats,
            notes,
        }
    }

    fn pack(&self, frames: &[Frame], plan: &JointPlan) -> Vec<f32> {
        let decode: Vec<bool> = plan
            .channels
            .iter()
            .map(|c| {
                self.channels
                    .spec(*c)
                    .is_some_and(|s| s.color_space == ColorSpace::Srgb)
            })
            .collect();
        pack(frames, &plan.channels, &decode)
    }

    /// Texels whose composite does not depend on the other unknowns' states, at every corner of the masks.
    fn independence(
        &self,
        plan: &JointPlan,
        response: &JointResponse,
        corners: &[(usize, Vec<f32>)],
        stats: &mut TexelStats,
    ) -> Vec<bool> {
        let texels = self.grid.size.0 * self.grid.size.1;
        let m = plan.components();
        let mut dirty = vec![false; texels];
        let states = state_combinations(&plan.others);
        let points = response.node_points();
        let keep = ProceduralPlan::default();
        for state in states.iter().skip(1) {
            for (n, reference) in corners {
                let (frames, _) = self.frames(plan, Some(&points[*n]), state, &keep, stats);
                let packed = pack(&frames, &plan.channels, &[]);
                for (i, d) in dirty.iter_mut().enumerate() {
                    let range = i * m..(i + 1) * m;
                    if reference[range.clone()]
                        .iter()
                        .zip(&packed[range])
                        .any(|(a, b)| (a - b).abs() > 0.5 * STEP || !b.is_finite())
                    {
                        *d = true;
                    }
                }
            }
        }
        dirty.iter().map(|d| !d).collect()
    }

    fn compared(
        &self,
        clean: &[bool],
        theirs: &JointSolution,
        ours: Option<&JointSolution>,
        stats: &mut TexelStats,
    ) -> Vec<bool> {
        let valid = self.grid.valid();
        let mut mask = vec![false; valid.len()];
        for (i, m) in mask.iter_mut().enumerate() {
            if !valid[i] {
                continue;
            }
            stats.island += 1;
            if !clean.get(i).copied().unwrap_or(false) {
                continue;
            }
            stats.clean += 1;
            stats.not_monotone += usize::from(theirs.states[i] == JointState::Folded);
            *m = theirs.solved(i) && ours.is_none_or(|o| o.solved(i));
            if *m {
                stats.solved += 1;
                stats.outside += usize::from(theirs.outside[i]);
            }
        }
        mask
    }

    /// The candidate at `point[0]` and the other masks at the rest (both left as they are for `None`), the
    /// other unknowns in `state`, every channel composited onto the grid.
    fn frames(
        &self,
        plan: &JointPlan,
        point: Option<&[f32]>,
        state: &[usize],
        procedurals: &ProceduralPlan,
        stats: &mut TexelStats,
    ) -> (Vec<Frame>, Vec<Computability>) {
        let mut swaps: Vec<Swap> = plan
            .others
            .iter()
            .zip(state)
            .map(|(n, s)| Swap::Nuisance(n.clone(), *s))
            .collect();
        match point {
            Some(p) => {
                swaps.push(Swap::Output(plan.candidate.address.clone(), p[0]));
                swaps.extend(plan.masks.iter().zip(&p[1..]).map(|(n, v)| Swap::Grey(n.clone(), *v)));
            }
            None => swaps.extend(plan.masks.iter().map(|n| Swap::Grey(n.clone(), OURS_MASK_VALUE))),
        }
        let swapped = apply(self.painter, &swaps, self.channels);
        let out = self.composer.composite_channels(&swapped, procedurals, &plan.channels);
        stats.composites += 1;
        out.into_iter().map(|(t, v)| (self.grid.frame(&t), v)).unzip()
    }
}

/// The observed components of every channel, texel by texel; channels flagged in `decode` (sRGB-encoded) are
/// decoded to linear values, where blending is multilinear in the masks.
pub fn pack(frames: &[Frame], channels: &[Channel], decode: &[bool]) -> Vec<f32> {
    let texels = frames.first().map_or(0, Vec::len);
    let m: usize = channels.iter().map(|c| components(*c)).sum();
    let mut out = Vec::with_capacity(texels * m);
    for i in 0..texels {
        for (k, (frame, c)) in frames.iter().zip(channels).enumerate() {
            let p = frame.get(i).copied().unwrap_or([0.0; 3]);
            let linear = decode.get(k).copied().unwrap_or(false);
            out.extend(
                p[..components(*c)]
                    .iter()
                    .map(|v| if linear { srgb_to_linear(*v) } else { *v }),
            );
        }
    }
    out
}

fn state_combinations(others: &[Nuisance]) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new()];
    for n in others {
        out = out
            .into_iter()
            .flat_map(|prefix| {
                (0..n.states()).map(move |s| {
                    let mut p = prefix.clone();
                    p.push(s);
                    p
                })
            })
            .collect();
    }
    out
}

/// The candidate's value (the first unknown) as a grey image.
fn tex(s: &JointSolution, (w, h): (usize, usize)) -> Tex {
    Tex {
        w,
        h,
        px: s
            .values
            .iter()
            .map(|v| {
                let g = v.first().copied().unwrap_or(0.0);
                [g, g, g, 1.0]
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_keeps_colour_and_grey_components() {
        let colour = vec![[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]];
        let rough = vec![[0.7, 0.0, 0.0], [0.8, 0.0, 0.0]];
        let packed = pack(
            &[colour.clone(), rough.clone()],
            &[Channel::BaseColor, Channel::Roughness],
            &[false, false],
        );
        assert_eq!(packed, vec![0.1, 0.2, 0.3, 0.7, 0.4, 0.5, 0.6, 0.8]);
        let decoded = pack(
            &[colour, rough],
            &[Channel::BaseColor, Channel::Roughness],
            &[true, false],
        );
        assert!((decoded[2] - srgb_to_linear(0.3)).abs() < 1e-6 && decoded[3] == 0.7);
        let others = vec![Nuisance::Layer(super::super::slots::LayerAddress(vec![1]))];
        assert_eq!(state_combinations(&others).len(), 3);
    }
}
