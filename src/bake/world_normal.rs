//! World-space normal map: the high surface's normal in object space, `n * 0.5 + 0.5`. Where no ray found the
//! high mesh, the low surface's own normal.

use super::context::Ctx;

pub(crate) fn bake(ctx: &Ctx) -> Vec<[f32; 4]> {
    ctx.map([0.5, 0.5, 1.0, 1.0], |_, s| {
        let (_, n) = s.surface();
        [n[0] * 0.5 + 0.5, n[1] * 0.5 + 0.5, n[2] * 0.5 + 0.5, 1.0]
    })
}
