//! [`Ctx`]: what every baker reads: the projected samples, the high mesh, the settings and the scale relative
//! distances are measured in.

use super::cancel::CancelToken;
use super::high::HighScene;
use super::project::{Sample, Samples};
use super::settings::BakeSettings;
use crate::mesh::Bounds;
use crate::noise::rnd;
use crate::tex::par_map;

pub(crate) struct Ctx<'a> {
    pub samples: &'a Samples,
    pub high: &'a HighScene,
    pub settings: &'a BakeSettings,
    /// Scene units per relative distance unit.
    pub scale: f32,
    /// Sub-samples per output texel side.
    pub sub: u32,
    pub set_bounds: Option<Bounds>,
    pub scene_bounds: Option<Bounds>,
    pub cancel: &'a CancelToken,
}

impl Ctx<'_> {
    /// One value per sample: `f` where the set covers it, `background` elsewhere (and once cancelled).
    pub fn map(&self, background: [f32; 4], f: impl Fn(usize, &Sample) -> [f32; 4] + Sync) -> Vec<[f32; 4]> {
        par_map(self.samples.texels.len(), |i| match self.samples.texels.get(i) {
            Some(Some(s)) if !self.cancel.is_cancelled() => f(i, s),
            _ => background,
        })
    }

    /// Secondary rays per sample, so that one output texel gets about `per_texel` in total.
    pub fn rays_per_sample(&self, per_texel: u32) -> u32 {
        per_texel.max(1).div_ceil(self.sub * self.sub).max(1)
    }

    /// A per-sample rotation (0..1 of a turn) for ray patterns.
    pub fn turn(&self, i: usize, seed: u32) -> f32 {
        let w = self.samples.width.max(1);
        rnd((i % w) as i32, (i / w) as i32, seed)
    }
}
