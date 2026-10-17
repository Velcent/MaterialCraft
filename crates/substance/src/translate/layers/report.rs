//! The import report's wording for a layer import: one summary line per set (layers, strategy, channels)
//! and one line per layer that is not imported exactly (`kept raw` with the reason when its content is not
//! computable, `approximated` for guesses).

use super::cache_agreement::CacheAgreement;
use super::channels::SetChannels;
use super::computability::Computability;
use super::stack_builder::ImportedStack;
use super::strategy::StackStrategy;
use super::verdict::MaskFailure;
use crate::translate::report::Reporter;
use materialcraft::paint::Fidelity;

pub fn report_stack(
    reporter: &mut Reporter,
    set: &str,
    stack: &ImportedStack,
    strategy: StackStrategy,
    channels: &SetChannels,
) {
    let verdict = &stack.verdict;
    let per_channel: Vec<String> = verdict
        .channels()
        .iter()
        .map(|(c, s)| format!("{} {}", channels.label(*c), state(s)))
        .collect();
    let fidelity = match strategy {
        StackStrategy::RealStack => Fidelity::Exact,
        _ => Fidelity::Approximated,
    };
    reporter.add(
        format!("texture set \"{set}\" / layer stack"),
        fidelity,
        format!(
            "{} layers imported with names, folders, visibility, blending and masks; {}. Channels: {}",
            verdict.layer_count,
            strategy.describe(),
            per_channel.join(", ")
        ),
    );
    for note in &verdict.layers {
        let fidelity = match note.status.is_computable() {
            true => Fidelity::Approximated,
            false => Fidelity::KeptRaw,
        };
        let hidden = if note.visible { "" } else { " (hidden)" };
        reporter.add(
            format!("texture set \"{set}\" / layer \"{}\"", note.path),
            fidelity,
            format!("{}{hidden}", note.status.reasons().join("; ")),
        );
    }
}

/// Channels where a fully computable stack differs from Painter's cache.
pub fn report_cache_disagreement(
    reporter: &mut Reporter,
    set: &str,
    agreement: &CacheAgreement,
    channels: &SetChannels,
) {
    if agreement.agrees() {
        return;
    }
    let measured: Vec<String> = agreement
        .disagreeing
        .iter()
        .map(|(c, s)| format!("{} mean {:.1} steps (max {:.0})", channels.label(*c), s.mean, s.max))
        .collect();
    reporter.add(
        format!("texture set \"{set}\" / Painter's cache"),
        Fidelity::Approximated,
        format!(
            "Painter's cache disagrees with the stored layers ({}); probably not re-rendered after the last edit. \
             Painter's cached result is shown on top",
            measured.join(", ")
        ),
    );
}

/// Layers hidden because their mask is not computable (only when Painter's result is not on top).
pub fn report_hidden_masks(reporter: &mut Reporter, set: &str, failures: &[MaskFailure]) {
    for f in failures {
        reporter.add(
            format!("texture set \"{set}\" / layer \"{}\"", f.path),
            Fidelity::Approximated,
            format!(
                "mask not computable yet: {}; layer hidden so it doesn't cover the whole set",
                f.reasons.join("; ")
            ),
        );
    }
}

fn state(s: &Computability) -> &'static str {
    match s {
        Computability::Exact => "exact",
        Computability::Replaced(_) => "replaced",
        Computability::Approximated(_) => "approximated",
        Computability::NotComputable(_) => "not computable",
    }
}
