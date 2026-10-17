//! [`Subject`]: one of our implementations whose output is compared with one of Painter's results.
//! A subject says which reference it is measured against, what inputs it needs and how to compute its
//! output from them; everything else (finding cases, loading references, metrics, reports) is shared.

use crate::inputs::Inputs;
use crate::settings::Params;
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;
use std::fmt;

/// Painter's result a subject is compared against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceKind {
    /// A map Painter baked from the mesh (an image resource `<Map> from Mesh <set>`).
    Baked(BakedKind),
}

impl fmt::Display for ReferenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReferenceKind::Baked(kind) => write!(f, "baked map '{}'", kind.id()),
        }
    }
}

/// An input a subject cannot run without.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// The texture set's own (low-poly) mesh from the project.
    LowMesh,
    /// The mesh Painter baked from: a high-poly file, or the low mesh when Painter baked "low as high".
    HighMesh,
}

/// One compared component of an image: its index (0 = red …) and label.
pub type Component = (usize, &'static str);

pub const GRAY: &[Component] = &[(0, "v")];
pub const RGB: &[Component] = &[(0, "r"), (1, "g"), (2, "b")];
pub const XYZ: &[Component] = &[(0, "x"), (1, "y"), (2, "z")];

pub trait Subject {
    /// Stable id used on the command line and in the history (`baked.ao`).
    fn id(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn reference(&self) -> ReferenceKind;
    fn needs(&self) -> &'static [Need];
    /// Components compared, with their labels.
    fn components(&self) -> &'static [Component];
    /// Our output for one case, the size of `params.size`, rows top first, in the reference's conventions.
    fn run(&self, inputs: &Inputs, params: &Params) -> Result<Tex, CaseError>;
}

/// Why a case produced no numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseError {
    /// The case cannot be run with what is available (no reference, no high-poly mesh, …).
    Skipped(String),
    /// It should have run but failed.
    Failed(String),
}

impl CaseError {
    pub fn skipped(reason: impl Into<String>) -> CaseError {
        CaseError::Skipped(reason.into())
    }

    pub fn failed(reason: impl Into<String>) -> CaseError {
        CaseError::Failed(reason.into())
    }

    pub fn reason(&self) -> &str {
        match self {
            CaseError::Skipped(r) | CaseError::Failed(r) => r,
        }
    }
}

impl fmt::Display for CaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaseError::Skipped(r) => write!(f, "skipped: {r}"),
            CaseError::Failed(r) => write!(f, "failed: {r}"),
        }
    }
}
