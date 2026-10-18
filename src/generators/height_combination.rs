//! `height_combination`: the "mask with height combination" (MK-02): the layer shows where its own height
//! rises above the combined height of everything below it, with the node side's height-blend formula
//! ([`crate::ops::height_blend_factor`]). Where the layer writes no height the mask is black.
//!
//! Parameters: `offset` (−1..1, raises the layer's height; 0 compares as is), `contrast` (0..1, how sharp the
//! transition is), `invert` (0/1).
//!
//! It reads the stack around its layer, so it is computed per request and never cached; outside a stack
//! (no [`crate::paint::effect::StackContext`]) it is an error and the effect is left out.

use super::params::Params;
use super::tone::invert_if;
use crate::ops::height_blend_factor;
use crate::paint::effect::{EvalRequest, GeneratorSource, Reach};

pub struct HeightCombination;

impl GeneratorSource for HeightCombination {
    /// The layer's own height and the height below it, at the texel itself.
    fn stack_reach(&self, _: &EvalRequest) -> Option<Reach> {
        Some(Reach::Texel)
    }

    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        let stack = request.stack.ok_or("a height combination needs the layer stack")?;
        let p = Params::new(request.params);
        let offset = 0.5 + p.range("offset", 0.0, -1.0, 1.0) * 0.5;
        let contrast = p.range("contrast", 0.5, 0.0, 1.0);
        let invert = p.on("invert", false);
        let own = stack.layer_height(request.rect);
        let below = stack.height_below(request.rect);
        if own.len() != out.len() || below.len() != out.len() {
            return Err("height sizes do not match the rectangle".into());
        }
        for ((o, h), b) in out.iter_mut().zip(own).zip(below) {
            let k = if h[1] > 0.0 { height_blend_factor(b, h[0], offset, contrast) * h[1].min(1.0) } else { 0.0 };
            let v = invert_if(k, invert);
            *o = [v, v, v, 1.0];
        }
        Ok(())
    }
}
