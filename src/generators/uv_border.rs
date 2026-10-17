//! `uv_border_distance`: white along the borders of the UV islands, fading towards their inside.
//!
//! The islands come from the set's surface map when the compositor has one, else from the coverage of the
//! baked position map (alpha, or any non-black texel); without either the whole texture is one island.
//!
//! Parameters: `distance` (width of the fade as a fraction of the texture width ÷ 4), `position` (where the
//! fade starts, same unit), `contrast` (0..1), `intensity` (−1..1, brightens), `invert` (0/1).

use super::cache::{self, Field};
use super::params::Params;
use super::texel::copy;
use super::tone::{brightness_contrast, invert_if, scan, unit};
use crate::mesh::{SurfaceMap, TexelKind};
use crate::ops::distance_field;
use crate::paint::effect::{EvalRequest, GeneratorSource};
use crate::paint::{BakedKind, TextureSet};

/// `distance` 1 = a quarter of the texture width.
const UNIT: f32 = 0.25;

pub struct UvBorder;

impl GeneratorSource for UvBorder {
    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        let (w, h) = (request.set.resolution.0 as usize, request.set.resolution.1 as usize);
        let mut key = cache::field_key(cache::fingerprint(request.set), "uv_border_distance", request.params);
        if let Some(s) = request.surface {
            key ^= (s.width() as u64) << 32 | s.island_count() as u64;
        }
        let field = cache::field(key, || {
            let inside = islands(request.set, request.surface, w, h);
            let distance = distance_field(&inside, w, h);
            let p = Params::new(request.params);
            let reach = p.range("distance", 0.13, 0.001, 4.0) * UNIT * w as f32;
            let start = p.range("position", 0.0, 0.0, 4.0) * UNIT * w as f32;
            let contrast = p.range("contrast", 0.0, 0.0, 1.0);
            let intensity = p.range("intensity", 0.0, -1.0, 1.0);
            let invert = p.on("invert", false);
            let values: Vec<f32> = distance
                .iter()
                .zip(&inside)
                .map(|(d, inside)| {
                    if !*inside {
                        return invert_if(1.0, invert);
                    }
                    let x = unit((d - start) / reach.max(1e-3));
                    let v = scan(1.0 - x, 0.5, contrast);
                    invert_if(brightness_contrast(v, intensity, 0.0), invert)
                })
                .collect();
            Field::from_values(w, h, &values)
        });
        copy(field.get(), request, out)
    }
}

/// Which texels lie inside an island, at the set's resolution.
fn islands(set: &TextureSet, surface: Option<&SurfaceMap>, w: usize, h: usize) -> Vec<bool> {
    if let Some(s) = surface.filter(|s| s.width() > 0 && s.height() > 0) {
        return (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) * s.width() / w, (i / w) * s.height() / h);
                s.sample(x, y).is_some_and(|t| t.kind == TexelKind::Covered)
            })
            .collect();
    }
    let Some(position) = set.baked(&BakedKind::Position) else {
        return vec![true; w * h];
    };
    let (pw, ph) = (position.width() as usize, position.height() as usize);
    let has_alpha = position.format().layout.has_alpha();
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) * pw / w.max(1), (i / w) * ph / h.max(1));
            let c = position.get(x as u32, y as u32);
            if has_alpha { c[3] >= 0.5 } else { c[0] + c[1] + c[2] > 0.0 }
        })
        .collect()
}
