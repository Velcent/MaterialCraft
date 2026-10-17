//! [`TextureSource`]: a procedural texture as a generator of the paint model. Every texel of the requested
//! rectangle becomes a [`SurfacePoint`], the projection described by the parameters turns it into taps, and the
//! texture is evaluated at the taps (weighted by the tap weights, alpha = the projection's coverage).

use super::texture;
use crate::paint::effect::{EvalRequest, GeneratorSource, Params};
use crate::projection::{ImagePlacement, Prepared, Projection, Projector, SurfacePoint};

/// The texture registered under `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureSource {
    pub id: &'static str,
}

impl TextureSource {
    pub fn new(id: &'static str) -> TextureSource {
        TextureSource { id }
    }

    /// How the parameters place the texture on the texture set: texture space, or a UV placement when
    /// `tiling`, `offset_x`, `offset_y` or `rotation` (degrees) is set.
    pub fn projection(params: &[(String, String)]) -> Projection {
        let p = Params::new(params);
        let tiling = p.range("tiling", 1.0, -1024.0, 1024.0);
        let image = ImagePlacement {
            tiling: [tiling, tiling],
            rotation: p.range("rotation", 0.0, -3600.0, 3600.0),
            offset: [p.range("offset_x", 0.0, -1024.0, 1024.0), p.range("offset_y", 0.0, -1024.0, 1024.0)],
            ..ImagePlacement::default()
        };
        if image.is_identity() { Projection::default() } else { Projection::uv(image) }
    }
}

impl GeneratorSource for TextureSource {
    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        let r = request.rect;
        if r.area() != out.len() {
            return Err("output size does not match the rectangle".into());
        }
        let tex = texture(self.id, request.params).ok_or_else(|| format!("unknown texture {}", self.id))?;
        let prepared: Prepared = TextureSource::projection(request.params).prepare(None, request.surface.is_some()).map_err(|e| e.to_string())?;
        let (w, h) = (request.set.resolution.0.max(1) as f32, request.set.resolution.1.max(1) as f32);
        let texel_len = 1.0 / w.max(h);
        let row_len = r.w.max(1) as usize;
        let mut rows: Vec<&mut [[f32; 4]]> = out.chunks_mut(row_len).collect();
        crate::paint::par::for_each_mut(&mut rows, |row, pixels| {
            let y = r.y as usize + row;
            for (i, o) in pixels.iter_mut().enumerate() {
                let x = r.x as usize + i;
                let point = SurfacePoint { uv: [(x as f32 + 0.5) / w, (y as f32 + 0.5) / h], texel_len, ..SurfacePoint::default() };
                let taps = prepared.project(&point);
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
}
