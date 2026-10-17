//! [`Recipe`]: a mask generator written as a value per texel from the set's maps, and [`Cached`], which turns
//! a recipe into a [`GeneratorSource`]: the whole texture is computed once (rows in parallel) into the field
//! cache, and every request copies its rectangle from there.

use super::cache::{self, Field};
use super::maps::SetMaps;
use super::params::Params;
use crate::paint::effect::{EvalRequest, GeneratorSource};

/// One texel of the set: its index and its UV centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Texel {
    pub x: usize,
    pub y: usize,
    pub u: f32,
    pub v: f32,
}

impl Texel {
    pub fn at(x: usize, y: usize, w: usize, h: usize) -> Texel {
        Texel { x, y, u: (x as f32 + 0.5) / w.max(1) as f32, v: (y as f32 + 0.5) / h.max(1) as f32 }
    }
}

/// A mask generator as a function of the texel.
pub trait Recipe: Send + Sync {
    /// The parameters read once per result.
    type Settings: Sync;

    fn settings(&self, params: &Params) -> Self::Settings;

    /// The mask value 0..1 at `t`.
    fn value(&self, s: &Self::Settings, t: &Texel, maps: &SetMaps) -> f32;
}

/// A recipe registered under `id`, with its results cached.
pub struct Cached<R: Recipe> {
    pub id: &'static str,
    pub recipe: R,
}

impl<R: Recipe> GeneratorSource for Cached<R> {
    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        let (w, h) = (request.set.resolution.0 as usize, request.set.resolution.1 as usize);
        let set_key = cache::fingerprint(request.set);
        let key = cache::field_key(set_key, self.id, request.params);
        let field = cache::field(key, || {
            let maps = cache::set_maps(request.set, set_key);
            let empty = SetMaps::of(&crate::paint::TextureSet::new("", "", (1, 1)));
            let maps = maps.get().unwrap_or(&empty);
            let settings = self.recipe.settings(&Params::new(request.params));
            Field::from_values(w, h, &compute(w, h, |t| self.recipe.value(&settings, &t, maps)))
        });
        copy(field.get(), request, out)
    }
}

/// Copies the request's rectangle out of a field as grey values with full coverage.
pub fn copy(field: Option<&Field>, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
    let field = field.ok_or("generator result missing")?;
    let r = request.rect;
    if r.area() != out.len() {
        return Err("output size does not match the rectangle".into());
    }
    for (i, o) in out.iter_mut().enumerate() {
        let (x, y) = (r.x as usize + i % r.w as usize, r.y as usize + i / r.w as usize);
        let v = if x < field.w && y < field.h { field.at(x, y) } else { 0.0 };
        *o = [v, v, v, 1.0];
    }
    Ok(())
}

/// `value` at every texel of a `w × h` texture, rows in parallel.
pub fn compute(w: usize, h: usize, value: impl Fn(Texel) -> f32 + Sync) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    if w == 0 || h == 0 {
        return out;
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).max(1);
    let rows_per = h.div_ceil(threads).max(1);
    let value = &value;
    std::thread::scope(|s| {
        for (chunk_index, chunk) in out.chunks_mut(rows_per * w).enumerate() {
            s.spawn(move || {
                for (i, o) in chunk.iter_mut().enumerate() {
                    let y = chunk_index * rows_per + i / w;
                    *o = value(Texel::at(i % w, y, w, h));
                }
            });
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::TextureSet;
    use crate::paint::effect::EvalTarget;
    use crate::paint::image::Rect;

    struct U;
    impl Recipe for U {
        type Settings = f32;
        fn settings(&self, p: &Params) -> f32 {
            p.f("k", 1.0)
        }
        fn value(&self, k: &f32, t: &Texel, _: &SetMaps) -> f32 {
            t.u * k
        }
    }

    #[test]
    fn recipes_fill_the_requested_rectangle() {
        let set = TextureSet::new("texel-test", "S", (10, 3));
        let params = vec![("k".to_string(), "0.5".to_string())];
        let rect = Rect { x: 4, y: 1, w: 2, h: 2 };
        let request = EvalRequest { set: &set, target: EvalTarget::Mask, rect, params: &params, stack: None, surface: None, frame: None };
        let mut out = vec![[0.0; 4]; 4];
        Cached { id: "u", recipe: U }.generate(&request, &mut out).unwrap();
        assert!((out[0][0] - 0.45 * 0.5).abs() < 1e-4 && (out[1][0] - 0.55 * 0.5).abs() < 1e-4 && out[3][3] == 1.0);
        let mut wrong = vec![[0.0; 4]; 3];
        assert!(Cached { id: "u", recipe: U }.generate(&request, &mut wrong).is_err());
        assert_eq!(compute(3, 2, |t| t.y as f32), vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    }
}
