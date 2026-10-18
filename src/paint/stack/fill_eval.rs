//! A fill's values over one rectangle: uniform values directly, images, textures (generators of the effect
//! registry that evaluate at points) and other sources through the fill's projection (texel → [`Taps`] → source
//! values), see [`super::fill_context`].

use super::Compositor;
use super::fill_context::FillRequest;
use super::pass::Pass;
use crate::paint::effect::{EvalTarget, Procedural};
use crate::paint::fill_image::FillImage;
use crate::paint::layer::{Fill, FillSource};
use crate::projection::{Mirrored, NotComputable, Prepared, Projector, SurfacePoint, Tap, Taps};

/// What a fill puts on top for one channel (working space, straight, alpha = coverage).
pub(super) enum FillTop {
    /// The same value on every texel.
    Constant([f32; 4]),
    /// One value per texel of the pass's rectangle.
    Pixels(Vec<[f32; 4]>),
}

impl FillTop {
    /// The value of texel `i`.
    pub fn at(&self, i: usize) -> [f32; 4] {
        match self {
            FillTop::Constant(v) => *v,
            FillTop::Pixels(px) => px.get(i).copied().unwrap_or([0.0; 4]),
        }
    }

    /// One value per texel of a rectangle of `area` texels.
    pub fn into_pixels(self, area: usize) -> Vec<[f32; 4]> {
        match self {
            FillTop::Constant(v) => vec![v; area],
            FillTop::Pixels(px) => px,
        }
    }
}

/// Why a fill that writes a channel cannot be evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FillProblem {
    /// A material (or another source) without an evaluator in the [`super::FillContext`].
    NoEvaluator,
    /// The projection cannot be evaluated.
    Projection(NotComputable),
    /// A texture fill names a texture without a point-evaluating generator in the effect registry.
    UnknownTexture(String),
}

impl Compositor<'_> {
    /// The fill's content for the pass's channel, or `None` where the fill does not write the channel or cannot
    /// be evaluated (see [`Self::fill_problem`]).
    pub(super) fn fill_top(&self, f: &Fill, pass: &Pass) -> Option<FillTop> {
        match &f.source {
            FillSource::Uniform(_) => f.uniform_value(pass.channel).map(|v| FillTop::Constant(pass.space.to_working([v[0], v[1], v[2], 1.0]))),
            FillSource::Image(fill) => match fill.image(pass.channel) {
                Some(image) => {
                    let prepared = self.prepare(f).ok()?;
                    let px = self.projected(&prepared, pass, |taps, out| sample_image(image, f, pass, taps, out));
                    px.map(FillTop::Pixels)
                }
                None => fill.value(pass.channel).map(|v| FillTop::Constant(pass.space.to_working([v[0], v[1], v[2], 1.0]))),
            },
            FillSource::Texture(fill) => match fill.texture(pass.channel) {
                Some(texture) => {
                    let generator = self.effects.generator(&texture.id).filter(|g| g.samples_points())?;
                    let prepared = self.prepare(f).ok()?;
                    let px = self.projected(&prepared, pass, |taps, out| {
                        let sampled = generator.sample_points(&texture.params, taps, out).is_ok();
                        out.iter_mut().for_each(|v| *v = pass.space.to_working(*v));
                        sampled
                    });
                    px.map(FillTop::Pixels)
                }
                None => fill.value(pass.channel).map(|v| FillTop::Constant(pass.space.to_working([v[0], v[1], v[2], 1.0]))),
            },
            source => {
                if !self.fills.sources.handles(source) {
                    return None;
                }
                let prepared = self.prepare(f).ok()?;
                let request = FillRequest {
                    set: self.set,
                    source,
                    target: EvalTarget::Channel(pass.channel),
                    filter: f.projection.filter,
                    repeat: f.projection.sampling_repeat(),
                };
                self.projected(&prepared, pass, |taps, out| self.fills.sources.sample(&request, taps, out)).map(FillTop::Pixels)
            }
        }
    }

    /// Why the fill cannot give `channel` its value, if it writes it and cannot.
    pub(super) fn fill_problem(&self, f: &Fill, channel: crate::paint::Channel) -> Option<FillProblem> {
        match &f.source {
            FillSource::Image(fill) if fill.image(channel).is_none() => None,
            FillSource::Texture(fill) => match fill.texture(channel) {
                Some(texture) if !self.texture_computable(texture) => Some(FillProblem::UnknownTexture(texture.id.clone())),
                Some(_) => self.prepare(f).err().map(FillProblem::Projection),
                None => None,
            },
            _ => self.fill_computable(f).err(),
        }
    }

    /// Whether the fill can be evaluated at all (every channel it has).
    pub(super) fn fill_computable(&self, f: &Fill) -> Result<(), FillProblem> {
        match &f.source {
            FillSource::Uniform(_) => Ok(()),
            FillSource::Image(_) => self.prepare(f).map(|_| ()).map_err(FillProblem::Projection),
            FillSource::Texture(fill) => match fill.textures().iter().find(|(_, t)| !self.texture_computable(t)) {
                Some((_, t)) => Err(FillProblem::UnknownTexture(t.id.clone())),
                None if fill.textures().is_empty() => Ok(()),
                None => self.prepare(f).map(|_| ()).map_err(FillProblem::Projection),
            },
            source if !self.fills.sources.handles(source) => Err(FillProblem::NoEvaluator),
            _ => self.prepare(f).map(|_| ()).map_err(FillProblem::Projection),
        }
    }

    /// True if the effect registry has a generator for `texture` that evaluates at points.
    fn texture_computable(&self, texture: &Procedural) -> bool {
        self.effects.generator(&texture.id).is_some_and(|g| g.samples_points())
    }

    fn prepare(&self, f: &Fill) -> Result<Mirrored<Prepared>, NotComputable> {
        f.projection.prepare_mirrored(self.fills.frame.as_ref(), self.fills.surface.is_some())
    }

    /// Projects every texel of the pass's rectangle, lets `sample` evaluate the taps and combines them per texel
    /// (premultiplied by tap weight and value coverage, times the projection's coverage). `None` if `sample`
    /// fails.
    fn projected(&self, prepared: &Mirrored<Prepared>, pass: &Pass, sample: impl FnOnce(&[Tap], &mut [[f32; 4]]) -> bool) -> Option<Vec<[f32; 4]>> {
        let r = pass.r;
        let (w, h) = self.set.resolution;
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        let texel_len = 1.0 / w.max(h);
        let uv_only = prepared.inner().uv_only();
        let surface = self.fills.surface.filter(|s| !uv_only && (s.width() as u32, s.height() as u32) == self.set.resolution);
        let world_len = surface.map_or(0.0, |s| s.world_per_uv() * texel_len);
        let mut per_texel: Vec<Taps> = Vec::with_capacity(r.area());
        let mut taps: Vec<Tap> = Vec::with_capacity(r.area());
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                let mut p = SurfacePoint { uv: [(x as f32 + 0.5) / w, (y as f32 + 0.5) / h], texel_len, world_len, ..SurfacePoint::default() };
                let t = match surface {
                    // volume projections have nothing to show where no surface lies under the texel
                    Some(s) => match s.point_index(y as usize * s.width() + x as usize) {
                        Some((pos, normal, _)) => {
                            (p.pos, p.normal) = (pos, normal);
                            prepared.project(&p)
                        }
                        None => Taps::default(),
                    },
                    None if uv_only => prepared.project(&p),
                    None => Taps::default(),
                };
                taps.extend_from_slice(t.used());
                per_texel.push(t);
            }
        }
        let mut values = vec![[0.0f32; 4]; taps.len()];
        if !sample(&taps, &mut values) {
            return None;
        }
        let mut next = values.iter();
        let mut out = Vec::with_capacity(r.area());
        for t in &per_texel {
            let (mut rgb, mut a) = ([0.0f32; 3], 0.0f32);
            for tap in t.used() {
                let v = next.next().copied().unwrap_or([0.0; 4]);
                let wa = tap.weight * v[3];
                for (c, s) in rgb.iter_mut().zip(v) {
                    *c += wa * s;
                }
                a += wa;
            }
            let coverage = (a * t.coverage).clamp(0.0, 1.0);
            out.push(match a > 0.0 {
                true => [rgb[0] / a, rgb[1] / a, rgb[2] / a, coverage],
                false => pass.empty(),
            });
        }
        Some(out)
    }
}

/// The image at each tap, filtered in its stored encoding and converted to working space.
fn sample_image(image: &FillImage, f: &Fill, pass: &Pass, taps: &[Tap], out: &mut [[f32; 4]]) -> bool {
    let levels = image.levels();
    let (filter, repeat) = (f.projection.filter, f.projection.sampling_repeat());
    for (tap, o) in taps.iter().zip(out.iter_mut()) {
        *o = match levels.sample(tap.uv, tap.footprint, filter, repeat) {
            Some(v) => pass.space.to_working(v),
            None => [0.0; 4],
        };
    }
    true
}
