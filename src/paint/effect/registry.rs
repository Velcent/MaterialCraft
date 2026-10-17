//! The evaluation interfaces generators and filters implement, and [`EffectRegistry`], which maps a
//! [`super::Procedural`] id to its implementation. The paint model defines these traits and names no
//! concrete procedural; implementations live in their own modules and are registered by the application.
//!
//! Values are in the channel's working space (linear light for sRGB channels, see
//! [`crate::paint::working_space`]), straight RGBA with alpha = coverage. For a mask the grey value is red.

use crate::paint::channel::Channel;
use crate::paint::image::Rect;
use crate::paint::texture_set::TextureSet;
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
}

/// What an effect may read from the layer stack it sits in, over any rectangle of the texture set. Heights
/// are the Height channel's stored values.
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
    /// How many texels beyond the output the kernel reads on each side (0 for per-texel filters).
    fn radius(&self, _params: &[(String, String)]) -> u32 {
        0
    }

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
}

impl fmt::Debug for EffectRegistry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let generators: Vec<&str> = self.generators.iter().map(|(k, _)| k.as_str()).collect();
        let filters: Vec<&str> = self.filters.iter().map(|(k, _)| k.as_str()).collect();
        f.debug_struct("EffectRegistry").field("generators", &generators).field("filters", &filters).finish()
    }
}
