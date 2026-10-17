//! Calls into registered generators and filters for one rectangle. A missing implementation, an error or a
//! wrong output size leaves the effect out (`None`); [`super::Compositor::warnings`] reports unregistered ones.

use super::Compositor;
use super::pass::grow;
use crate::paint::effect::{EvalRequest, EvalTarget, FilterInput, Procedural};
use crate::paint::image::Rect;

/// The generator's values over `r`.
pub(super) fn generate(c: &Compositor, p: &Procedural, target: EvalTarget, r: Rect) -> Option<Vec<[f32; 4]>> {
    let generator = c.effects.generator(&p.id)?;
    let mut out = vec![[0.0; 4]; r.area()];
    let request = EvalRequest { set: c.set, target, rect: r, params: &p.params };
    generator.generate(&request, &mut out).ok()?;
    Some(out)
}

/// One filter evaluation over `r`.
pub(super) struct FilterRun<'c, 'a> {
    pub compositor: &'c Compositor<'a>,
    pub procedural: &'c Procedural,
    pub target: EvalTarget,
    pub r: Rect,
}

impl FilterRun<'_, '_> {
    /// Runs the filter; `input` gives what is below the filter over a rectangle (`r` grown by the kernel's
    /// radius, clipped to the texture).
    pub fn run(&self, input: impl FnOnce(Rect) -> Vec<[f32; 4]>) -> Option<Vec<[f32; 4]>> {
        let kernel = self.compositor.effects.filter(&self.procedural.id)?;
        let params = &self.procedural.params;
        let outer = grow(self.r, kernel.radius(params), self.compositor.set.resolution);
        let pixels = input(outer);
        let mut out = vec![[0.0; 4]; self.r.area()];
        let request = EvalRequest { set: self.compositor.set, target: self.target, rect: self.r, params };
        kernel.apply(&request, &FilterInput { rect: outer, pixels: &pixels }, &mut out).ok()?;
        Some(out)
    }
}
