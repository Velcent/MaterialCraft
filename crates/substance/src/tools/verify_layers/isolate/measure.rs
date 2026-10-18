//! [`Measurer`]: runs one [`CasePlan`]: composites the probes and the nuisance states, reads Painter's value
//! back through the probes ([`Probes::invert`]) and, when we have a replacement, ours the same way, so both
//! sides are the candidate's own output on the same texels.
//!
//! Composites come from a [`Composer`] (the importer and compositor at the set's resolution) and are shrunk
//! onto a [`FrameGrid`] at once, so only a few frames of the compared size stay in memory.

use super::frames::{Frame, FrameGrid, Independence};
use super::inverse::{Coupling, Inversion, InversionSettings, Probes, TexelState};
use super::plan::CasePlan;
use super::slots::{Family, Nuisance};
use super::substitute::{Swap, apply};
use crate::model::PainterTextureSet;
use crate::translate::layers::replay::ReplayChoice;
use crate::translate::layers::{Computability, SetChannels};
use crate::translate::replacements::{Choice, ProceduralPlan};
use materialcraft::paint::Channel;
use materialcraft::tex::Tex;

/// Values of the candidate's output that are composited: 0 … 1, or −1 … 1 for a fill writing height
/// (Painter's height values are signed).
pub fn probe_values(plan: &CasePlan) -> [f32; 5] {
    match plan.channel == Channel::Height && !plan.candidate.in_mask() {
        true => [-1.0, -0.5, 0.0, 0.5, 1.0],
        false => [0.0, 0.25, 0.5, 0.75, 1.0],
    }
}

/// Imports a (swapped) Painter set and composites one channel.
pub trait Composer {
    /// The channel's final look (stored encoding, over Painter's defaults) at the set's resolution, and the
    /// import's verdict on the channel.
    /// Strokes are replayed with `replay` (only our side of a stroke candidate needs them).
    fn composite(
        &self,
        painter: &PainterTextureSet,
        procedurals: &ProceduralPlan,
        replay: ReplayChoice,
        channel: Channel,
    ) -> (Tex, Computability);
}

/// Texel counts of one measured case, at the compared size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TexelStats {
    /// On the mesh.
    pub island: usize,
    /// … and independent of every nuisance.
    pub clean: usize,
    /// … and solved (steep, monotone response, on both sides): the texels compared.
    pub solved: usize,
    /// Solved texels where Painter's value lay outside what the stack can give (clamped).
    pub outside: usize,
    /// Clean texels whose response is not monotone.
    pub not_monotone: usize,
    pub composites: usize,
}

pub struct Measurement {
    /// Painter's output of the candidate, read back (grey repeated for a mask), compared size.
    pub theirs: Tex,
    /// Ours read back the same way, when we replace the candidate.
    pub ours: Option<Tex>,
    /// Texels compared: on the mesh, clean, solved on both sides.
    pub mask: Vec<bool>,
    /// Components that carry information (1 for a mask or a grey channel).
    pub components: usize,
    pub stats: TexelStats,
    pub notes: Vec<String>,
}

pub struct Measurer<'a> {
    pub composer: &'a dyn Composer,
    pub painter: &'a PainterTextureSet,
    pub channels: &'a SetChannels,
    pub grid: &'a FrameGrid,
    pub settings: InversionSettings,
}

impl Measurer<'_> {
    /// `observed`: Painter's final look on the grid. `with_ours` also reads back our replacement.
    pub fn measure(&self, plan: &CasePlan, observed: &[[f32; 3]], with_ours: bool) -> Measurement {
        let components = if plan.channel.is_color() { 3 } else { 1 };
        let coupling = if plan.candidate.in_mask() {
            Coupling::Joint
        } else {
            Coupling::PerComponent
        };
        let mut stats = TexelStats::default();
        let mut notes = Vec::new();
        let states = state_combinations(plan);
        let base = states.first().cloned().unwrap_or_default();
        let keep = ProceduralPlan::default();
        let mut probes = Probes::new(components);
        let values = probe_values(plan);
        for v in values {
            probes.push(
                v,
                self.frame(plan, Some(v), &base, &keep, ReplayChoice::Keep, &mut stats)
                    .0,
            );
        }
        let ends = [values[0], values[values.len() - 1]];
        let reference: Vec<Frame> = ends
            .iter()
            .filter_map(|v| probes.frame(*v).map(<[_]>::to_vec))
            .collect();
        let mut independence = Independence::new(reference, components);
        for state in states.iter().skip(1) {
            for (i, v) in ends.iter().enumerate() {
                independence.compare(
                    i,
                    &self
                        .frame(plan, Some(*v), state, &keep, ReplayChoice::Keep, &mut stats)
                        .0,
                );
            }
        }
        let theirs = probes.invert(observed, coupling, &self.settings);
        let ours = match (with_ours, plan.candidate.ours) {
            (true, Some(_)) => {
                let replace = ProceduralPlan::all(Choice::Replace);
                let replay = match plan.candidate.family {
                    Family::Strokes => ReplayChoice::Replay,
                    _ => ReplayChoice::Keep,
                };
                let (frame, verdict) = self.frame(plan, None, &base, &replace, replay, &mut stats);
                // replayed strokes may also leave the channel exact (nothing replaced reaches it)
                let strokes = plan.candidate.family == Family::Strokes;
                match verdict {
                    Computability::Replaced(_) => Some(probes.invert(&frame, coupling, &self.settings)),
                    Computability::Exact if strokes => Some(probes.invert(&frame, coupling, &self.settings)),
                    other => {
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
        let mask = self.compared(&independence.clean(), &theirs, ours.as_ref(), &mut stats);
        let size = self.grid.size;
        Measurement {
            theirs: tex(&theirs, size),
            ours: ours.as_ref().map(|o| tex(o, size)),
            mask,
            components: if coupling == Coupling::Joint { 1 } else { components },
            stats,
            notes,
        }
    }

    /// Texels on the mesh, clean and solved on both sides; counts them into `stats`.
    fn compared(
        &self,
        clean: &[bool],
        theirs: &Inversion,
        ours: Option<&Inversion>,
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
            stats.not_monotone += usize::from(theirs.states[i] == TexelState::NotMonotone);
            *m = theirs.solved(i) && ours.is_none_or(|o| o.solved(i));
            if *m {
                stats.solved += 1;
                stats.outside += usize::from(theirs.outside[i]);
            }
        }
        mask
    }

    /// The candidate at `v` (left as it is for `None`), the nuisances in `state`, composited onto the grid.
    fn frame(
        &self,
        plan: &CasePlan,
        v: Option<f32>,
        state: &[usize],
        procedurals: &ProceduralPlan,
        replay: ReplayChoice,
        stats: &mut TexelStats,
    ) -> (Frame, Computability) {
        let mut swaps: Vec<Swap> = plan
            .swapped()
            .into_iter()
            .zip(state)
            .map(|(n, s)| Swap::Nuisance(n, *s))
            .collect();
        if let Some(v) = v {
            swaps.push(Swap::Output(plan.candidate.address.clone(), v));
        }
        let swapped = apply(self.painter, &swaps, self.channels);
        let (tex, verdict) = self.composer.composite(&swapped, procedurals, replay, plan.channel);
        stats.composites += 1;
        (self.grid.frame(&tex), verdict)
    }
}

/// Every combination of nuisance states, the base one first: each nuisance in its first state, except the
/// content of the candidate's own layer, which starts shown (white) — switched off, the layer would hide a
/// mask candidate in every probe.
fn state_combinations(plan: &CasePlan) -> Vec<Vec<usize>> {
    let own = &plan.candidate.address.layer;
    let mut out = vec![Vec::new()];
    for n in plan.swapped() {
        let shown_first =
            plan.candidate.in_mask() && matches!(&n, Nuisance::Layer(l) | Nuisance::LayerWithMask(l) if l == own);
        let order: Vec<usize> = match shown_first {
            true => (0..n.states()).rev().collect(),
            false => (0..n.states()).collect(),
        };
        out = out
            .into_iter()
            .flat_map(|prefix| {
                order.clone().into_iter().map(move |s| {
                    let mut p = prefix.clone();
                    p.push(s);
                    p
                })
            })
            .collect();
    }
    out
}

fn tex(inv: &Inversion, (w, h): (usize, usize)) -> Tex {
    Tex {
        w,
        h,
        px: inv.values.iter().map(|v| [v[0], v[1], v[2], 1.0]).collect(),
    }
}
