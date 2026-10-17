//! [`SetRun`]: every selected subject on one texture set: loads the references, picks the compared size,
//! loads the inputs once, runs each subject and measures it inside the set's UV islands.

use super::case::{CaseImages, CaseResult, CaseSink, Outcome};
use super::measure::Case;
use crate::image_ops::resized;
use crate::inputs::HighSource;
use crate::project::{OpenedProject, SetCase};
use crate::reference::{Reference, References};
use crate::subject::{CaseError, ReferenceKind, Subject};
use materialcraft::mesh::Mesh;
use materialcraft::paint::BakedKind;
use materialcraft_substance::tools::compare::UvMask;

/// Texels next to island borders left out (Painter dilates there; as in `mcsub compare`).
const BORDER: usize = 1;

pub struct SetRun<'a> {
    pub project: &'a OpenedProject,
    pub references: &'a References<'a>,
    pub set: &'a SetCase,
    pub max_size: usize,
}

impl SetRun<'_> {
    /// Runs `subjects` (all of which have a reference here) with the high mesh from `source`.
    pub fn run(&self, subjects: &[&dyn Subject], source: &HighSource, sink: &mut dyn CaseSink) -> Vec<CaseResult> {
        let refs: Vec<Result<Reference, CaseError>> = subjects
            .iter()
            .map(|s| self.references.load(self.set, &s.reference()))
            .map(|r| r.map(|r| self.capped(r)))
            .collect();
        let Some(size) = refs.iter().flatten().map(|r| (r.image.w, r.image.h)).next() else {
            return self.skip_all(
                subjects,
                refs.into_iter().map(|r| r.err().unwrap_or(CaseError::failed("?"))),
                sink,
            );
        };
        let inputs = match self.project.inputs(self.set, source) {
            Ok(i) => i,
            Err(e) => return self.skip_all(subjects, std::iter::repeat(e), sink),
        };
        let params = self.project.params(self.set, size.0.max(size.1));
        let islands = self.mask(size);
        let coverage = self.coverage(size);
        let mut out = Vec::new();
        for (subject, reference) in subjects.iter().zip(refs) {
            let case = Case {
                subject: *subject,
                inputs: &inputs,
                params: &params,
                size,
                islands: &islands,
                coverage: coverage.as_deref(),
            };
            let result = match reference.and_then(|r| case.measure(r)) {
                Ok(done) => {
                    let result = self.result(*subject, Outcome::Measured(Box::new(done.measured)));
                    let images = CaseImages {
                        ours: &done.ours,
                        theirs: &done.theirs,
                        mask: &done.mask,
                        errors: &done.errors,
                    };
                    sink.case(&result, Some(&images));
                    result
                }
                Err(e) => {
                    let result = self.result(*subject, outcome_of(e));
                    sink.case(&result, None);
                    result
                }
            };
            out.push(result);
        }
        out
    }

    /// The reference shrunk to at most `max_size` on its longer side (keeping its aspect).
    fn capped(&self, mut r: Reference) -> Reference {
        let (w, h) = (r.image.w, r.image.h);
        let longest = w.max(h).max(1);
        if longest > self.max_size {
            let scale = |v: usize| (v * self.max_size / longest).max(1);
            r.image = resized(r.image, scale(w), scale(h));
        }
        r
    }

    /// Where Painter's rays hit the high mesh: the alpha of its baked normal map, at `size`.
    fn coverage(&self, (w, h): (usize, usize)) -> Option<Vec<bool>> {
        let normal = ReferenceKind::Baked(BakedKind::Normal);
        let image = resized(self.references.load(self.set, &normal).ok()?.image, w, h);
        Some(image.px.iter().map(|p| p[3] >= 0.5).collect())
    }

    /// Inside the islands of all the set's slots, minus the border texels.
    fn mask(&self, (w, h): (usize, usize)) -> Vec<bool> {
        let mut mask = UvMask::rasterise(&Mesh::default(), 0, 0, w, h);
        for &slot in &self.set.slots {
            let one = UvMask::rasterise(&self.project.mesh, slot, 0, w, h);
            mask.inside.iter_mut().zip(one.inside).for_each(|(a, b)| *a |= b);
        }
        mask.shrink(BORDER);
        mask.inside
    }

    /// The same failure or skip for every subject (no images).
    pub fn skip_all(
        &self,
        subjects: &[&dyn Subject],
        errors: impl Iterator<Item = CaseError>,
        sink: &mut dyn CaseSink,
    ) -> Vec<CaseResult> {
        subjects
            .iter()
            .zip(errors)
            .map(|(s, e)| {
                let r = self.result(*s, outcome_of(e));
                sink.case(&r, None);
                r
            })
            .collect()
    }

    fn result(&self, subject: &dyn Subject, outcome: Outcome) -> CaseResult {
        CaseResult {
            subject: subject.id(),
            project: self.project.path.clone(),
            project_name: self.project.name(),
            set: self.set.name.clone(),
            outcome,
        }
    }
}

pub fn outcome_of(e: CaseError) -> Outcome {
    match e {
        CaseError::Skipped(r) => Outcome::Skipped(r),
        CaseError::Failed(r) => Outcome::Failed(r),
    }
}
