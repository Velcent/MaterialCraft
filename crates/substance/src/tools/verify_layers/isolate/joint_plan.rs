//! [`JointPlan`]: one candidate mask effect measured across every channel its layer writes, with the other
//! unknown masks of the stack solved together with it ([`super::joint_solve`]) instead of being separated by
//! texels. Only masks are joint unknowns (one grey value shared by all channels of a layer); unknown layer
//! content is still swapped for its extremes ([`super::frames::Independence`]).

use super::joint_solve::MAX_UNKNOWNS;
use super::plan::{CasePlan, CaseStatus, MAX_STATES};
use super::slots::{Candidate, Family, Nuisance};
use materialcraft::paint::Channel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JointStatus {
    /// Measurable jointly.
    Feasible,
    /// More unknown masks than [`MAX_UNKNOWNS`] − 1 besides the candidate.
    TooManyMasks,
    /// The other unknowns have more combined states than [`MAX_STATES`].
    TooManyStates,
    /// Fewer observed components than unknowns.
    TooFewChannels,
    /// Every channel is blocked (or the candidate is not a mask generator or texture).
    NotApplicable,
}

impl JointStatus {
    pub fn label(self) -> &'static str {
        match self {
            JointStatus::Feasible => "feasible",
            JointStatus::TooManyMasks => "too many masks",
            JointStatus::TooManyStates => "too many states",
            JointStatus::TooFewChannels => "too few channels",
            JointStatus::NotApplicable => "not applicable",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JointPlan {
    pub candidate: Candidate,
    /// Channels observed (not blocked; normals are not measured).
    pub channels: Vec<Channel>,
    /// The other unknown masks, solved with the candidate.
    pub masks: Vec<Nuisance>,
    /// The other unknowns, separated by texels.
    pub others: Vec<Nuisance>,
    pub status: JointStatus,
    /// Whether one of the single-channel cases was measurable already (strict or with nuisances).
    pub single_measurable: bool,
}

impl JointPlan {
    /// The unknowns: the candidate and the other masks.
    pub fn unknowns(&self) -> usize {
        1 + self.masks.len()
    }

    pub fn states(&self) -> usize {
        self.others.iter().map(Nuisance::states).product()
    }

    /// Observed values per texel.
    pub fn components(&self) -> usize {
        self.channels.iter().map(|c| components(*c)).sum()
    }
}

/// Observed values of a channel per texel.
pub fn components(channel: Channel) -> usize {
    if channel.is_color() { 3 } else { 1 }
}

/// One joint plan per candidate mask generator or texture that some single-channel plan names.
pub fn joint_plans(plans: &[CasePlan]) -> Vec<JointPlan> {
    let mut seen: Vec<&Candidate> = Vec::new();
    let mut out = Vec::new();
    for p in plans {
        if seen.contains(&&p.candidate) {
            continue;
        }
        seen.push(&p.candidate);
        let mine: Vec<&CasePlan> = plans.iter().filter(|q| q.candidate == p.candidate).collect();
        out.push(joint_plan(&mine));
    }
    out
}

fn joint_plan(plans: &[&CasePlan]) -> JointPlan {
    let candidate = plans[0].candidate.clone();
    let single_measurable = plans.iter().any(|p| p.status.measurable());
    let usable: Vec<&&CasePlan> = plans
        .iter()
        .filter(|p| p.status != CaseStatus::Blocked && p.channel != Channel::Normal)
        .collect();
    let mut plan = JointPlan {
        candidate: candidate.clone(),
        channels: usable.iter().map(|p| p.channel).collect(),
        masks: Vec::new(),
        others: Vec::new(),
        status: JointStatus::NotApplicable,
        single_measurable,
    };
    let applicable = candidate.in_mask() && candidate.family != Family::Filter && !usable.is_empty();
    if !applicable {
        return plan;
    }
    // the union of every channel's unknowns, merged per layer as a single case would merge them
    let mut union = Vec::new();
    for p in &usable {
        for n in &p.nuisances {
            if !union.contains(n) {
                union.push(n.clone());
            }
        }
    }
    let merged = CasePlan {
        nuisances: union,
        ..(*usable[0]).clone()
    }
    .swapped();
    for n in merged {
        match n {
            Nuisance::MaskEffect(_) | Nuisance::Mask(_) => plan.masks.push(n),
            _ => plan.others.push(n),
        }
    }
    plan.status = if plan.unknowns() > MAX_UNKNOWNS {
        JointStatus::TooManyMasks
    } else if plan.states() > MAX_STATES {
        JointStatus::TooManyStates
    } else if plan.components() < plan.unknowns() {
        JointStatus::TooFewChannels
    } else {
        JointStatus::Feasible
    };
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::verify_layers::isolate::slots::{EffectAddress, LayerAddress, NuisanceKind, Place};

    fn at(layer: usize, place: Place) -> EffectAddress {
        EffectAddress {
            layer: LayerAddress(vec![layer]),
            place,
        }
    }

    fn plan(channel: Channel, nuisances: Vec<Nuisance>, status: CaseStatus) -> CasePlan {
        CasePlan {
            channel,
            candidate: Candidate {
                address: at(0, Place::Mask(0)),
                graph: "mg_dirt".into(),
                ours: Some("dirt"),
                family: Family::Generator,
                path: "Dirt".into(),
                projection: 0,
            },
            nuisances: nuisances.into_iter().map(|n| (n, NuisanceKind::Procedural)).collect(),
            blockers: Vec::new(),
            status,
        }
    }

    #[test]
    fn masks_of_all_channels_become_joint_unknowns() {
        let m1 = Nuisance::MaskEffect(at(1, Place::Mask(0)));
        let m2 = Nuisance::MaskEffect(at(2, Place::Mask(0)));
        let content = Nuisance::Layer(LayerAddress(vec![3]));
        let plans = vec![
            plan(
                Channel::BaseColor,
                vec![m1.clone(), content.clone()],
                CaseStatus::WithNuisances,
            ),
            plan(Channel::Roughness, vec![m2.clone()], CaseStatus::WithNuisances),
            plan(Channel::Normal, vec![m1.clone()], CaseStatus::WithNuisances),
            plan(Channel::Height, vec![m1.clone()], CaseStatus::Blocked),
        ];
        let joint = joint_plans(&plans);
        assert_eq!(joint.len(), 1);
        let j = &joint[0];
        assert_eq!(j.channels, vec![Channel::BaseColor, Channel::Roughness]);
        assert_eq!(j.masks, vec![m1.clone(), m2.clone()]);
        assert_eq!(j.others, vec![content]);
        assert_eq!((j.unknowns(), j.components(), j.states()), (3, 4, 3));
        assert_eq!(j.status, JointStatus::Feasible);
        assert!(j.single_measurable);
        let m3 = Nuisance::MaskEffect(at(4, Place::Mask(0)));
        let m4 = Nuisance::MaskEffect(at(5, Place::Mask(0)));
        let many = vec![plan(
            Channel::BaseColor,
            vec![m1, m2, m3, m4],
            CaseStatus::WithNuisances,
        )];
        assert_eq!(joint_plans(&many)[0].status, JointStatus::TooManyMasks);
        let grey = vec![plan(
            Channel::Roughness,
            vec![Nuisance::Mask(LayerAddress(vec![5]))],
            CaseStatus::WithNuisances,
        )];
        assert_eq!(joint_plans(&grey)[0].status, JointStatus::TooFewChannels);
    }
}
