//! ID map: one flat colour per high-mesh part (its group), so parts can be masked by colour.

use super::context::Ctx;
use crate::noise::rnd;

pub(crate) fn bake(ctx: &Ctx) -> Vec<[f32; 4]> {
    ctx.map([0.0, 0.0, 0.0, 1.0], |_, s| match &s.hit {
        Some(h) => colour(h.group),
        None => [0.0, 0.0, 0.0, 1.0],
    })
}

/// A stable, fairly saturated colour for group `g`.
pub(crate) fn colour(g: u32) -> [f32; 4] {
    let g = g as i32;
    [0.2 + 0.8 * rnd(g, 1, 11), 0.2 + 0.8 * rnd(g, 2, 11), 0.2 + 0.8 * rnd(g, 3, 11), 1.0]
}
