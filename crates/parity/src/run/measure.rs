//! [`Case`]: one subject measured on one set: run ours, bring both images to the compared size, and measure
//! where both have something to compare (inside the islands, and where Painter's rays hit the high mesh).

use super::case::Measured;
use crate::image_ops::resized;
use crate::inputs::Inputs;
use crate::metrics::Metrics;
use crate::reference::Reference;
use crate::settings::Params;
use crate::settings::mapping::baked_usage;
use crate::subject::{CaseError, ReferenceKind, Subject};
use materialcraft::tex::Tex;
use std::time::Instant;

/// Painter's alpha below this marks texels its rays did not cover (e.g. no high-poly surface in reach).
const COVERED: f32 = 0.5;

pub struct Case<'a> {
    pub subject: &'a dyn Subject,
    pub inputs: &'a Inputs,
    pub params: &'a Params,
    pub size: (usize, usize),
    /// Inside the set's UV islands (border texels removed).
    pub islands: &'a [bool],
    /// Where Painter's rays hit the high mesh (alpha of its baked normal map), if known. Maps without
    /// alpha (AO, curvature, …) are black where rays missed, so this applies to every baked map.
    pub coverage: Option<&'a [bool]>,
}

/// A measured case with its images, at the compared size.
pub struct Done {
    pub measured: Measured,
    pub ours: Tex,
    pub theirs: Tex,
    pub mask: Vec<bool>,
    pub errors: Vec<f32>,
}

impl Case<'_> {
    pub fn measure(&self, reference: Reference) -> Result<Done, CaseError> {
        let started = Instant::now();
        let (w, h) = self.size;
        let ours = resized(self.subject.run(self.inputs, self.params)?, w, h);
        let theirs = resized(reference.image, w, h);
        let mask: Vec<bool> = self
            .islands
            .iter()
            .zip(&theirs.px)
            .enumerate()
            .map(|(i, (&inside, p))| inside && p[3] >= COVERED && self.coverage.is_none_or(|c| c.get(i) == Some(&true)))
            .collect();
        let (metrics, errors) = Metrics::measure(&ours, &theirs, self.subject.components(), &mask);
        let mut notes = reference.notes;
        let islands = self.islands.iter().filter(|b| **b).count();
        let uncovered = islands.saturating_sub(metrics.overall.texels);
        if uncovered > 0 {
            notes.push(format!(
                "{:.1} % of island texels left out: Painter's rays found no high-poly surface there",
                100.0 * uncovered as f64 / islands.max(1) as f64
            ));
        }
        notes.extend(self.inputs.high.notes.iter().cloned());
        let measured = Measured {
            metrics,
            size: self.size,
            reference: reference.name,
            reference_size: reference.stored_size,
            high: self.inputs.high.source.label(),
            notes,
            unmapped: self
                .params
                .unmapped_for(self.usage())
                .into_iter()
                .map(String::from)
                .collect(),
            settings_hash: self.params.hash(),
            seconds: started.elapsed().as_secs_f64(),
        };
        Ok(Done {
            measured,
            ours,
            theirs,
            mask,
            errors,
        })
    }

    /// Painter's baker usage for the subject's reference (selects its unmapped settings).
    fn usage(&self) -> Option<u64> {
        match self.subject.reference() {
            ReferenceKind::Baked(kind) => baked_usage(&kind),
        }
    }
}
