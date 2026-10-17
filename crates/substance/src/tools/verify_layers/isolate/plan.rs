//! [`CasePlan`]: one candidate effect in one channel, with the nuisances it has to be separated from, and
//! whether it can be measured: strictly (nothing else unknown), on the texels the nuisances do not reach, or
//! not at all (and which roadmap step would change that).

use super::analysis::SetAnalysis;
use super::slots::{Candidate, Family, LayerAddress, Nuisance, NuisanceKind, Place};
use materialcraft::paint::Channel;
use std::collections::BTreeSet;

/// At most this many combined nuisance states are tried (each costs two composites).
pub const MAX_STATES: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CaseStatus {
    /// Nothing but the candidate is unknown in the channel (today's criterion).
    Strict,
    /// Measurable on the texels where the nuisances make no difference.
    WithNuisances,
    /// More nuisance states than [`MAX_STATES`].
    TooManyNuisances,
    /// Another unknown sits in the candidate's own layer content.
    SameLayer,
    /// A filter next to other unknowns: what it filters may be unknown (its output alone says nothing about
    /// ours, which needs the same input).
    FilterInput,
    /// Something that cannot be swapped for constants (see [`CasePlan::blockers`]).
    Blocked,
}

impl CaseStatus {
    pub fn label(self) -> &'static str {
        match self {
            CaseStatus::Strict => "strict",
            CaseStatus::WithNuisances => "with nuisances",
            CaseStatus::TooManyNuisances => "too many nuisances",
            CaseStatus::SameLayer => "same layer",
            CaseStatus::FilterInput => "filter input unknown",
            CaseStatus::Blocked => "blocked",
        }
    }

    pub fn measurable(self) -> bool {
        matches!(self, CaseStatus::Strict | CaseStatus::WithNuisances)
    }
}

/// The roadmap step that removes a nuisance or blocker (plan.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unblocker {
    /// 13: levels, polygon fill, anchors, colour selection, blend values not identified yet.
    NativeEffects,
    /// 22: filters.
    Filters,
    /// 24: 3D fill projections.
    Projections,
    /// 32: replay of Painter's strokes.
    StrokeReplay,
    /// 16, 23, 27: other procedurals (generators, textures, materials) computed or replaced.
    Procedurals,
    Other,
}

impl Unblocker {
    pub fn label(self) -> &'static str {
        match self {
            Unblocker::NativeEffects => "13 native effects/blend modes",
            Unblocker::Filters => "22 filters",
            Unblocker::Projections => "24 3D projections",
            Unblocker::StrokeReplay => "32 stroke replay",
            Unblocker::Procedurals => "16/23/27 other procedurals",
            Unblocker::Other => "other",
        }
    }

    fn of_kind(kind: NuisanceKind) -> Unblocker {
        match kind {
            NuisanceKind::Strokes => Unblocker::StrokeReplay,
            NuisanceKind::Procedural => Unblocker::Procedurals,
            NuisanceKind::Filter => Unblocker::Filters,
            NuisanceKind::Levels | NuisanceKind::PolygonFill | NuisanceKind::ColorSelection | NuisanceKind::Anchor => {
                Unblocker::NativeEffects
            }
            NuisanceKind::Unknown => Unblocker::Other,
        }
    }

    /// The step a verdict reason (`translate::layers` wording) belongs to.
    pub fn of_reason(reason: &str) -> Unblocker {
        let has = |s: &str| reason.contains(s);
        if has("strokes") {
            Unblocker::StrokeReplay
        } else if has("projection") || has("texture-space") {
            Unblocker::Projections
        } else if has("filter \"") {
            Unblocker::Filters
        } else if has("Painter blend")
            || has("levels")
            || has("polygon fill")
            || has("anchor")
            || has("colour selection")
        {
            Unblocker::NativeEffects
        } else if has("procedural") || has("generator") {
            Unblocker::Procedurals
        } else {
            Unblocker::Other
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CasePlan {
    pub channel: Channel,
    pub candidate: Candidate,
    /// Every unknown part the channel depends on besides the candidate, with its kind.
    pub nuisances: Vec<(Nuisance, NuisanceKind)>,
    /// What cannot be swapped, from the verdict with every slot swapped (empty unless blocked).
    pub blockers: Vec<String>,
    pub status: CaseStatus,
}

impl CasePlan {
    /// The nuisances as they are swapped: in each layer other than the candidate's, an unknown mask (several
    /// unknown mask effects) becomes one, and unknown content with an unknown mask becomes the whole layer.
    pub fn swapped(&self) -> Vec<Nuisance> {
        let own = &self.candidate.address.layer;
        let mut layers: Vec<&LayerAddress> = Vec::new();
        for (n, _) in &self.nuisances {
            if !layers.contains(&n.layer()) {
                layers.push(n.layer());
            }
        }
        let mut out = Vec::new();
        for layer in layers {
            let here: Vec<&Nuisance> = self
                .nuisances
                .iter()
                .map(|(n, _)| n)
                .filter(|n| n.layer() == layer)
                .collect();
            let masks = here.iter().filter(|n| matches!(n, Nuisance::MaskEffect(_))).count();
            let content = here.iter().any(|n| matches!(n, Nuisance::Layer(_)));
            let mask_is_free = layer != own || !self.candidate.in_mask();
            if content && masks > 0 && layer != own {
                out.push(Nuisance::LayerWithMask(layer.clone()));
            } else if masks > 1 && mask_is_free {
                out.extend(
                    here.iter()
                        .filter(|n| matches!(n, Nuisance::Layer(_)))
                        .map(|n| (*n).clone()),
                );
                out.push(Nuisance::Mask(layer.clone()));
            } else {
                out.extend(here.into_iter().cloned());
            }
        }
        out
    }

    /// Combined nuisance states (1 without nuisances).
    pub fn states(&self) -> usize {
        self.swapped().iter().map(Nuisance::states).product()
    }

    /// The steps that would make this case strict.
    pub fn unblockers(&self) -> BTreeSet<Unblocker> {
        let mut out: BTreeSet<Unblocker> = self.nuisances.iter().map(|(_, k)| Unblocker::of_kind(*k)).collect();
        out.extend(self.blockers.iter().map(|r| Unblocker::of_reason(r)));
        if matches!(self.status, CaseStatus::SameLayer | CaseStatus::FilterInput) && out.is_empty() {
            out.insert(Unblocker::Other);
        }
        out
    }
}

/// One plan per candidate a non-exact channel depends on.
pub fn plans(analysis: &SetAnalysis) -> Vec<CasePlan> {
    let mut out = Vec::new();
    for ch in &analysis.channels {
        for &i in &ch.relevant {
            let Some(candidate) = analysis.slots[i].candidate() else {
                continue;
            };
            let mut nuisances: Vec<(Nuisance, NuisanceKind)> = Vec::new();
            for &j in ch.relevant.iter().filter(|j| **j != i) {
                let form = analysis.slots[j].nuisance();
                if !nuisances.iter().any(|(n, _)| *n == form.0) {
                    nuisances.push(form);
                }
            }
            let own_layer = Nuisance::Layer(candidate.address.layer.clone());
            let same_layer =
                matches!(candidate.address.place, Place::Content(_)) && nuisances.iter().any(|(n, _)| *n == own_layer);
            let blockers = ch.blockers.reasons().to_vec();
            let mut plan = CasePlan {
                channel: ch.channel,
                candidate: candidate.clone(),
                nuisances,
                blockers,
                status: CaseStatus::Strict,
            };
            plan.status = if !ch.blockers.is_exact() {
                CaseStatus::Blocked
            } else if same_layer {
                CaseStatus::SameLayer
            } else if plan.candidate.family == Family::Filter && !plan.nuisances.is_empty() {
                CaseStatus::FilterInput
            } else if plan.states() > MAX_STATES {
                CaseStatus::TooManyNuisances
            } else if plan.nuisances.is_empty() {
                CaseStatus::Strict
            } else {
                CaseStatus::WithNuisances
            };
            out.push(plan);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::verify_layers::isolate::slots::EffectAddress;

    #[test]
    fn reasons_map_to_roadmap_steps() {
        assert_eq!(Unblocker::of_reason("painted mask (strokes)"), Unblocker::StrokeReplay);
        assert_eq!(
            Unblocker::of_reason("Painter blend mode 29 imported as subtract (not identified yet)"),
            Unblocker::NativeEffects
        );
        assert_eq!(
            Unblocker::of_reason("projection 2 (tri-planar) not evaluated yet; imported as texture space"),
            Unblocker::Projections
        );
        assert_eq!(Unblocker::of_reason("mask filter \"blur\""), Unblocker::Filters);
        assert_eq!(Unblocker::of_reason("effects on a folder"), Unblocker::Other);
        assert_eq!(Unblocker::of_kind(NuisanceKind::Levels), Unblocker::NativeEffects);
    }

    #[test]
    fn unknowns_of_one_layer_are_swapped_together() {
        let at = |layer: usize, place: Place| EffectAddress {
            layer: LayerAddress(vec![layer]),
            place,
        };
        let candidate = Candidate {
            address: at(0, Place::Mask(1)),
            graph: "mg_dirt".into(),
            ours: Some("dirt"),
            family: Family::Generator,
            path: "Dirt".into(),
            projection: 0,
        };
        let strokes = NuisanceKind::Strokes;
        let nuisances = vec![
            (Nuisance::MaskEffect(at(0, Place::Mask(0))), strokes),
            (Nuisance::MaskEffect(at(0, Place::Mask(2))), strokes),
            (Nuisance::MaskEffect(at(1, Place::Mask(0))), strokes),
            (Nuisance::MaskEffect(at(1, Place::Mask(1))), NuisanceKind::Levels),
            (Nuisance::Layer(LayerAddress(vec![2])), strokes),
            (Nuisance::MaskEffect(at(2, Place::Mask(0))), strokes),
        ];
        let plan = CasePlan {
            channel: Channel::Roughness,
            candidate,
            nuisances,
            blockers: Vec::new(),
            status: CaseStatus::WithNuisances,
        };
        let swapped = plan.swapped();
        // the candidate's own mask keeps its effects apart; layer 1's mask is one; layer 2 is one
        assert_eq!(swapped.len(), 4, "{swapped:?}");
        assert_eq!(swapped[2], Nuisance::Mask(LayerAddress(vec![1])));
        assert_eq!(swapped[3], Nuisance::LayerWithMask(LayerAddress(vec![2])));
        assert_eq!(plan.states(), 2 * 2 * 2 * 3);
        assert_eq!(plan.unblockers().len(), 2);
    }
}
