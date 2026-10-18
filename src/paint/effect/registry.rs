//! The evaluation interfaces generators and filters implement, and [`EffectRegistry`], which maps a
//! [`super::Procedural`] id to its implementation. The paint model defines these traits and names no
//! concrete procedural; implementations live in their own modules and are registered by the application.
//!
//! Values are in the channel's working space (linear light for sRGB channels, see
//! [`crate::paint::working_space`]), straight RGBA with alpha = coverage. For a mask the grey value is red.

use super::reach::Reach;
use crate::mesh::SurfaceMap;
use crate::paint::channel::Channel;
use crate::paint::image::Rect;
use crate::paint::texture_set::TextureSet;
use crate::projection::{SceneFrame, Tap};
use std::fmt;

/// What an effect is evaluated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalTarget {
    /// A material channel of a layer's content.
    Channel(Channel),
    /// A layer's grey mask.
    Mask,
}

/// One evaluation: the texture set (for its resolution and baked maps), the target, the pixel rectangle and
/// the effect's parameters.
#[derive(Debug, Clone, Copy)]
pub struct EvalRequest<'a> {
    pub set: &'a TextureSet,
    pub target: EvalTarget,
    pub rect: Rect,
    pub params: &'a [(String, String)],
    /// The layer stack around the effect, for generators that compare against it (height-comparison masks);
    /// `None` when the effect is evaluated on its own.
    pub stack: Option<&'a dyn StackContext>,
    /// The texture set's surface map (UV islands, positions), when the compositor has one.
    pub surface: Option<&'a SurfaceMap>,
    /// The whole scene's frame, which volume projections are placed in; `None` without a mesh.
    pub frame: Option<SceneFrame>,
}

/// What an effect may read from the layer stack it sits in, over any rectangle of the texture set. Heights
/// are the Height channel's stored values. A generator reads it over its request's rectangle only: the compositor
/// relies on a generator's texel depending on the stack at that texel alone (a brush stroke's changed tiles are
/// updated on their own, [`crate::paint::stack::Compositor::reach`]); a generator that needs neighbours has to be a
/// filter, whose reach is declared.
pub trait StackContext: fmt::Debug + Sync {
    /// The height the effect's layer itself writes: (value, coverage) per texel, row by row.
    fn layer_height(&self, rect: Rect) -> Vec<[f32; 2]>;

    /// The composited height of everything below the effect's layer, row by row.
    fn height_below(&self, rect: Rect) -> Vec<f32>;
}

/// Produces values from nothing but its parameters and the texture set (baked maps, resolution).
pub trait GeneratorSource: Send + Sync {
    /// Writes `request.rect.area()` pixels, row by row. An error leaves the effect out.
    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String>;

    /// True if the generator is a function of image coordinates alone that [`Self::sample_points`] evaluates
    /// (a procedural texture): such a generator can also fill channels ([`crate::paint::TextureFill`]), laid on
    /// by the fill's projection.
    fn samples_points(&self) -> bool {
        false
    }

    /// The generator's value at each tap's image coordinates (`taps[i].uv`, repeating with period 1) into `out`
    /// (same length): grey in RGB, alpha = coverage. Only for generators that [`Self::samples_points`].
    fn sample_points(&self, _params: &[(String, String)], _taps: &[Tap], _out: &mut [[f32; 4]]) -> Result<(), String> {
        Err("this generator does not evaluate at points".into())
    }

    /// How far around a texel the generator reads the layer stack through [`EvalRequest::stack`] (the heights of
    /// [`StackContext`]); `None` if it never does. Baked maps, the surface map and the parameters don't count:
    /// they are not the stack. Required, with no default: every generator states its reach when it is written,
    /// proves it with a test and is entered in the reach ledger ([`crate::generators::reach_ledger::LEDGER`]).
    fn stack_reach(&self, request: &EvalRequest) -> Option<Reach>;
}

/// The input of a filter: what is below the filter in its stack, over `rect` (the output rectangle grown by
/// the kernel's radius and clipped to the texture).
#[derive(Debug, Clone, Copy)]
pub struct FilterInput<'a> {
    pub rect: Rect,
    pub pixels: &'a [[f32; 4]],
}

/// Transforms what is below it in its stack.
pub trait FilterKernel: Send + Sync {
    /// How many texels beyond the output the kernel reads on each side (0 for per-texel filters). The request
    /// gives the parameters and the texture set (its resolution: kernels sized in UV units need it).
    fn radius(&self, _request: &EvalRequest) -> u32 {
        0
    }

    /// Which texels of its input (what is below it in its stack) one output texel depends on, at most: texels
    /// beyond it never change that output, however the compositor cuts the texture into rectangles. Required,
    /// with no default: every filter states its reach when it is written, proves it with a test and is entered in
    /// the reach ledger ([`crate::generators::reach_ledger::LEDGER`]).
    fn reach(&self, request: &EvalRequest) -> Reach;

    /// Writes `request.rect.area()` pixels, row by row. An error leaves the effect out.
    fn apply(&self, request: &EvalRequest, input: &FilterInput, out: &mut [[f32; 4]]) -> Result<(), String>;
}

/// Generator and filter implementations by id. The default registry is empty: every generator and filter
/// is then not computable.
#[derive(Default)]
pub struct EffectRegistry {
    generators: Vec<(String, Box<dyn GeneratorSource>)>,
    filters: Vec<(String, Box<dyn FilterKernel>)>,
}

/// A registry without implementations.
pub static EMPTY_REGISTRY: EffectRegistry = EffectRegistry::new();

impl EffectRegistry {
    pub const fn new() -> EffectRegistry {
        EffectRegistry { generators: Vec::new(), filters: Vec::new() }
    }

    /// Registers (or replaces) the generator with `id`.
    pub fn add_generator(&mut self, id: impl Into<String>, generator: Box<dyn GeneratorSource>) {
        let id = id.into();
        self.generators.retain(|(k, _)| *k != id);
        self.generators.push((id, generator));
    }

    /// Registers (or replaces) the filter with `id`.
    pub fn add_filter(&mut self, id: impl Into<String>, filter: Box<dyn FilterKernel>) {
        let id = id.into();
        self.filters.retain(|(k, _)| *k != id);
        self.filters.push((id, filter));
    }

    pub fn generator(&self, id: &str) -> Option<&dyn GeneratorSource> {
        self.generators.iter().find(|(k, _)| k == id).map(|(_, g)| g.as_ref())
    }

    pub fn filter(&self, id: &str) -> Option<&dyn FilterKernel> {
        self.filters.iter().find(|(k, _)| k == id).map(|(_, f)| f.as_ref())
    }

    /// The ids of every registered generator, in registration order.
    pub fn generator_ids(&self) -> impl Iterator<Item = &str> {
        self.generators.iter().map(|(k, _)| k.as_str())
    }

    /// The ids of every registered filter, in registration order.
    pub fn filter_ids(&self) -> impl Iterator<Item = &str> {
        self.filters.iter().map(|(k, _)| k.as_str())
    }
}

impl fmt::Debug for EffectRegistry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let generators: Vec<&str> = self.generators.iter().map(|(k, _)| k.as_str()).collect();
        let filters: Vec<&str> = self.filters.iter().map(|(k, _)| k.as_str()).collect();
        f.debug_struct("EffectRegistry").field("generators", &generators).field("filters", &filters).finish()
    }
}
