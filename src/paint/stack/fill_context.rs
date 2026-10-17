//! The compositor's hook for fills: [`FillContext`] (what a fill may need besides the texture set: the mesh's
//! surface, the scene frame, and evaluators for sources the paint model cannot evaluate itself) and
//! [`FillSources`], the interface those evaluators implement.
//!
//! The compositor evaluates uniform and image fills itself. For every other source (materials, and the generator
//! fills of the procedural replacements) it prepares the fill's projection, turns each texel of the rectangle into
//! [`Tap`]s and asks the context's [`FillSources`] for the values at the taps. Implementations are chained with
//! [`ChainedSources`]: the first one that evaluates a source answers. Generators and filters that work on whole
//! rectangles in texture space stay behind [`super::super::effect::GeneratorSource`] and
//! [`super::super::effect::FilterKernel`]; a [`FillSources`] implementation may wrap one of them for fills.

use crate::mesh::SurfaceMap;
use crate::paint::effect::EvalTarget;
use crate::paint::layer::FillSource;
use crate::paint::texture_set::TextureSet;
use crate::projection::{Filter, Repeat, SceneFrame, Tap};

/// One evaluation of a source at a list of taps.
#[derive(Debug, Clone, Copy)]
pub struct FillRequest<'a> {
    pub set: &'a TextureSet,
    pub source: &'a FillSource,
    pub target: EvalTarget,
    pub filter: Filter,
    pub repeat: Repeat,
}

/// Evaluates fill sources at projected image coordinates.
pub trait FillSources: Sync {
    /// True if this implementation evaluates `source` (used for warnings without evaluating).
    fn handles(&self, source: &FillSource) -> bool;

    /// Writes one value per tap into `out` (same length as `taps`): working space, straight RGBA, alpha =
    /// coverage (0 where the source has no value, e.g. outside a non-repeating image). Returns false if this
    /// implementation does not evaluate the source or failed; the fill is then left out and reported.
    fn sample(&self, request: &FillRequest, taps: &[Tap], out: &mut [[f32; 4]]) -> bool;
}

/// Evaluates nothing: every fill source except uniform values and images is not computable.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFillSources;

impl FillSources for NoFillSources {
    fn handles(&self, _source: &FillSource) -> bool {
        false
    }

    fn sample(&self, _request: &FillRequest, _taps: &[Tap], _out: &mut [[f32; 4]]) -> bool {
        false
    }
}

/// Two implementations in a row: `first` answers what it handles, `then` the rest.
#[derive(Clone, Copy)]
pub struct ChainedSources<'a> {
    pub first: &'a dyn FillSources,
    pub then: &'a dyn FillSources,
}

impl FillSources for ChainedSources<'_> {
    fn handles(&self, source: &FillSource) -> bool {
        self.first.handles(source) || self.then.handles(source)
    }

    fn sample(&self, request: &FillRequest, taps: &[Tap], out: &mut [[f32; 4]]) -> bool {
        match self.first.handles(request.source) {
            true => self.first.sample(request, taps, out),
            false => self.then.sample(request, taps, out),
        }
    }
}

/// What fills may need besides the texture set.
#[derive(Clone, Copy)]
pub struct FillContext<'a> {
    /// The texture set's surface map at its resolution; `None`: volume-based projections are reported as
    /// needing the mesh instead of being guessed.
    pub surface: Option<&'a SurfaceMap>,
    /// The whole scene's frame (volume-based projections are placed in it); `None` without a mesh.
    pub frame: Option<SceneFrame>,
    /// Evaluators for sources the compositor does not evaluate itself.
    pub sources: &'a dyn FillSources,
}

/// No surface, no frame, no evaluators: uniform and texture-space/UV image fills only.
pub static NO_FILL_CONTEXT: FillContext<'static> = FillContext { surface: None, frame: None, sources: &NoFillSources };

impl<'a> FillContext<'a> {
    /// A context with `sources` and nothing else.
    pub fn with_sources(sources: &'a dyn FillSources) -> FillContext<'a> {
        FillContext { surface: None, frame: None, sources }
    }

    /// Builder: the surface map and the scene frame.
    pub fn with_surface(mut self, surface: &'a SurfaceMap, frame: Option<SceneFrame>) -> FillContext<'a> {
        self.surface = Some(surface);
        self.frame = frame;
        self
    }
}

impl std::fmt::Debug for FillContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FillContext").field("surface", &self.surface.map(|s| (s.width(), s.height()))).field("frame", &self.frame).finish_non_exhaustive()
    }
}
