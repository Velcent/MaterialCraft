//! Isolated effect references: what one Painter effect (a mask generator, a procedural texture, a filter)
//! wrote, read back from Painter's texture cache although the rest of the channel's stack is in the way.
//!
//! Painter saves no per-layer or per-effect images (only the flattened channels), so the effect's output is
//! solved for: the effect is swapped for constants and the stack composited for each ([`inverse`]); the
//! cache value is then read back through that response wherever it is steep and monotone. Other parts we
//! cannot compute (*nuisances*: strokes, levels, other procedurals …) are swapped for their extremes, and
//! only texels whose composite does not change with them are kept ([`frames::Independence`]). Our
//! replacement is read back through the same response, so both sides are the effect's own output on the same
//! texels, without solving any blend formula by hand.
//!
//! - [`slots`]: the parts of a stack that can be swapped; [`substitute`]: the swapped copies.
//! - [`analysis`] and [`plan`]: which slots each channel depends on, and the case per candidate effect.
//! - [`inverse`], [`frames`], [`measure`]: probes, read-back, compared texels.
//! - [`cases`]: per project; [`census`]: counts per effect id (`mcsub isolate`).
//!
//! Used by `mcparity` (`generator.<id>`, `texture.<id>` subjects, isolated cases) and `mcsub isolate`.

pub mod analysis;
mod cached_images;
pub mod cases;
pub mod census;
pub mod corpus;
pub mod frames;
pub mod inverse;
pub mod measure;
pub mod plan;
pub mod slots;
pub mod substitute;
#[cfg(test)]
mod tests;

pub use cases::{IsolateOptions, IsolatedCase, SetPlans, isolated_cases, project_plans};
pub use census::Census;
pub use inverse::{Coupling, InversionSettings, Probes};
pub use measure::{Measurement, TexelStats};
pub use plan::{CasePlan, CaseStatus, Unblocker};
pub use slots::{Candidate, Family};
