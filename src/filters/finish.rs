//! Finish rough: breaks up the roughness with a grunge ([`crate::proc_textures::grunge_look`]): roughness +
//! (grunge − 0.5) × 2 × `intensity` (default 0.2). `tile` (default 4) repeats the grunge over the texture,
//! `grunge` picks the map (1..15, default 2), `seed` varies it. Other channels and masks are left unchanged.

use super::kernel::{Ctx, PointFilter};
use crate::paint::Channel;
use crate::paint::effect::Params;
use crate::proc_textures::{ProcTexture, grunge_look};

pub struct FinishRough;

pub struct FinishSettings {
    intensity: f32,
    tile: f32,
    grunge: ProcTexture,
}

impl PointFilter for FinishRough {
    type Settings = FinishSettings;

    fn settings(&self, p: &Params, _: &Ctx) -> FinishSettings {
        FinishSettings {
            intensity: p.range("intensity", 0.2, 0.0, 1.0),
            tile: p.range("tile", 4.0, 1.0, 64.0).round(),
            grunge: grunge_look(p.int("grunge", 2).clamp(1, 15) as u32, p.seed("seed")),
        }
    }

    fn texel(&self, s: &FinishSettings, ctx: &Ctx, v: [f32; 4], uv: (f32, f32)) -> [f32; 4] {
        if ctx.channel() != Some(Channel::Roughness) {
            return v;
        }
        let d = (s.grunge.value(uv.0 * s.tile, uv.1 * s.tile) - 0.5) * 2.0 * s.intensity;
        let r = ctx.limit(v[0] + d);
        [r, r, r, v[3]]
    }
}
