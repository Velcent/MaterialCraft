//! [`SetAnalysis`]: per channel of one texture set, which slots it depends on and what else keeps it from being
//! computable, decided by the importer's own verdict on swapped copies of the set:
//!
//! - every slot swapped for constants: the channel's verdict then lists what cannot be swapped (blend modes
//!   not identified yet, effects on folders, unknown layer classes …), the *blockers*;
//! - every slot but one swapped: where the verdict changes, the channel depends on that slot.
//!
//! One import per slot plus two; nothing is composited here.

use super::slots::{Nuisance, Slot, slots};
use super::substitute::{Swap, apply};
use crate::model::PainterTextureSet;
use crate::translate::layers::{Computability, SetChannels, StackVerdict};
use materialcraft::paint::Channel;

/// One channel that is not computable as imported.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelAnalysis {
    pub channel: Channel,
    /// The channel's verdict with every slot swapped (exact = nothing but the slots stands in the way).
    pub blockers: Computability,
    /// Indices into [`SetAnalysis::slots`] of the slots the channel depends on.
    pub relevant: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetAnalysis {
    pub slots: Vec<Slot>,
    pub channels: Vec<ChannelAnalysis>,
}

impl SetAnalysis {
    /// Analyses `set`; `verdict` imports a (swapped) copy and returns the import's verdict.
    pub fn new(
        set: &PainterTextureSet,
        channels: &SetChannels,
        verdict: &dyn Fn(&PainterTextureSet) -> StackVerdict,
    ) -> Self {
        let slots = slots(set);
        let original = verdict(set);
        let open: Vec<Channel> = original
            .channels()
            .into_iter()
            .filter(|(_, s)| !s.is_exact())
            .map(|(c, _)| c)
            .collect();
        if open.is_empty() {
            return SetAnalysis {
                slots,
                channels: Vec::new(),
            };
        }
        let forms: Vec<Nuisance> = slots.iter().map(|s| s.nuisance().0).collect();
        let swaps_without = |skip: Option<&Nuisance>| -> Vec<Swap> {
            let mut out: Vec<Swap> = Vec::new();
            for n in forms.iter().filter(|n| Some(*n) != skip) {
                let swap = Swap::Nuisance(n.clone(), 1);
                if !out.contains(&swap) {
                    out.push(swap);
                }
            }
            out
        };
        let all = verdict(&apply(set, &swaps_without(None), channels));
        let mut analyses: Vec<ChannelAnalysis> = open
            .iter()
            .map(|c| ChannelAnalysis {
                channel: *c,
                blockers: all.channel(*c),
                relevant: Vec::new(),
            })
            .collect();
        for (i, form) in forms.iter().enumerate() {
            let without = verdict(&apply(set, &swaps_without(Some(form)), channels));
            for a in &mut analyses {
                if without.channel(a.channel) != a.blockers {
                    a.relevant.push(i);
                }
            }
        }
        SetAnalysis {
            slots,
            channels: analyses,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::verify_layers::isolate::tests::{fixture, import_verdict};

    #[test]
    fn channels_depend_on_the_slots_that_write_them() {
        let (set, channels) = fixture();
        let a = SetAnalysis::new(&set, &channels, &|s| import_verdict(s, &channels));
        // fixture: base fill, a dirt-masked fill writing roughness, a painted layer writing base colour
        assert_eq!(a.slots.len(), 2);
        let rough = a.channels.iter().find(|c| c.channel == Channel::Roughness).unwrap();
        assert!(rough.blockers.is_exact(), "{:?}", rough.blockers);
        assert_eq!(rough.relevant, vec![0]);
        let colour = a.channels.iter().find(|c| c.channel == Channel::BaseColor).unwrap();
        assert_eq!(colour.relevant, vec![0, 1]);
    }
}
