//! [`TextureSource`]: a procedural texture as a generator of the paint model. Every texel of the requested
//! rectangle becomes a [`SurfacePoint`], the projection described by the parameters turns it into taps, and the
//! texture is evaluated at the taps (weighted by the tap weights, alpha = the projection's coverage). The volume
//! projections read the surface map's positions and vertex normals; texels without a surface point get no
//! coverage there.
//!
//! As a channel fill's texture ([`crate::paint::TextureFill`]) the fill's projection places it instead:
//! [`GeneratorSource::sample_points`] evaluates the texture at the taps the compositor projected.

use super::{placement, texture};
use crate::paint::effect::{EvalRequest, GeneratorSource, Reach};
use crate::projection::{Projection, Projector, SurfacePoint, Tap, Taps};

/// The texture registered under `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureSource {
    pub id: &'static str,
}

impl TextureSource {
    pub fn new(id: &'static str) -> TextureSource {
        TextureSource { id }
    }

    /// How the parameters place the texture on the texture set (see [`placement`]).
    pub fn projection(params: &[(String, String)]) -> Projection {
        placement::read(params)
    }
}

impl GeneratorSource for TextureSource {
    /// Reads baked maps, the surface map and its parameters, never the layer stack.
    fn stack_reach(&self, _: &EvalRequest) -> Option<Reach> {
        None
    }

    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        let r = request.rect;
        if r.area() != out.len() {
            return Err("output size does not match the rectangle".into());
        }
        let tex = texture(self.id, request.params).ok_or_else(|| format!("unknown texture {}", self.id))?;
        let projection = TextureSource::projection(request.params);
        let prepared = projection.prepare_mirrored(request.frame.as_ref(), request.surface.is_some()).map_err(|e| e.to_string())?;
        let (w, h) = (request.set.resolution.0.max(1) as f32, request.set.resolution.1.max(1) as f32);
        let texel_len = 1.0 / w.max(h);
        let uv_only = prepared.inner().uv_only();
        let surface = request.surface.filter(|s| !uv_only && (s.width() as u32, s.height() as u32) == request.set.resolution);
        let world_len = surface.map_or(0.0, |s| s.world_per_uv() * texel_len);
        let row_len = r.w.max(1) as usize;
        let mut rows: Vec<&mut [[f32; 4]]> = out.chunks_mut(row_len).collect();
        crate::paint::par::for_each_mut(&mut rows, |row, pixels| {
            let y = r.y as usize + row;
            for (i, o) in pixels.iter_mut().enumerate() {
                let x = r.x as usize + i;
                let mut point = SurfacePoint { uv: [(x as f32 + 0.5) / w, (y as f32 + 0.5) / h], texel_len, world_len, ..SurfacePoint::default() };
                let taps = match surface {
                    Some(s) => match s.point_index(y * s.width() + x) {
                        Some((pos, normal, _)) => {
                            (point.pos, point.normal) = (pos, normal);
                            prepared.project(&point)
                        }
                        None => Taps::default(),
                    },
                    None if uv_only => prepared.project(&point),
                    None => Taps::default(),
                };
                let (mut sum, mut weight) = (0.0f32, 0.0f32);
                for tap in taps.used() {
                    sum += tap.weight * tex.value(tap.uv[0], tap.uv[1]);
                    weight += tap.weight;
                }
                let v = if weight > 0.0 { sum / weight } else { 0.0 };
                *o = [v, v, v, taps.coverage.clamp(0.0, 1.0)];
            }
        });
        Ok(())
    }

    fn samples_points(&self) -> bool {
        true
    }

    /// The texture at the taps (its placement parameters are not used: the fill's projection placed the taps).
    fn sample_points(&self, params: &[(String, String)], taps: &[Tap], out: &mut [[f32; 4]]) -> Result<(), String> {
        if taps.len() != out.len() {
            return Err("output size does not match the taps".into());
        }
        let tex = texture(self.id, params).ok_or_else(|| format!("unknown texture {}", self.id))?;
        tex.sample_taps(taps, out);
        Ok(())
    }
}
