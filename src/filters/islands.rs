//! Running a neighbourhood kernel per UV island, so nothing bleeds from one island into another.
//!
//! The output rectangle grown by the kernel's radius is laid out as a [`Plane`]; every texel gets a class: its
//! UV island from the texture set's surface map (gutter texels belong to the island they extend), one class for
//! texels outside every island and its gutter, and "void" for texels outside the texture. Without a surface map
//! the whole texture is one class. For each class present in the output, the kernel runs on the class's part of
//! the plane with every other texel replaced by the nearest texel of the class ([`super::pad`]), and only the
//! class's own output texels are kept. Texels outside the texture are padded the same way (clamping at the
//! texture's edges).

use super::pad::pad;
use super::plane::Plane;
use crate::mesh::SurfaceMap;
use crate::paint::effect::{EvalRequest, FilterInput};
use crate::paint::image::Rect;
use std::collections::BTreeMap;

/// Texels on no island (beyond the gutter).
const OFF_SET: u32 = u32::MAX - 1;
/// Texels outside the texture (or not provided).
const VOID: u32 = u32::MAX;

/// The class of every texel of the texture set.
struct Classes<'a> {
    surface: Option<&'a SurfaceMap>,
    size: (u32, u32),
}

impl Classes<'_> {
    fn at(&self, x: i64, y: i64) -> u32 {
        let (w, h) = (self.size.0 as i64, self.size.1 as i64);
        if x < 0 || y < 0 || x >= w || y >= h {
            return VOID;
        }
        let Some(s) = self.surface else { return 0 };
        let (sw, sh) = (s.width() as i64, s.height() as i64);
        // the surface map normally has the set's resolution; otherwise the nearest texel
        let (sx, sy) = if (sw, sh) == (w, h) { (x, y) } else { (x * sw / w.max(1), y * sh / h.max(1)) };
        s.island_index((sy * sw + sx) as usize).unwrap_or(OFF_SET)
    }
}

/// Inclusive bounds of a class's output texels, in plane coordinates.
#[derive(Clone, Copy)]
struct Bounds {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Bounds {
    fn add(&mut self, x: usize, y: usize) {
        self.x0 = self.x0.min(x);
        self.y0 = self.y0.min(y);
        self.x1 = self.x1.max(x);
        self.y1 = self.y1.max(y);
    }
}

/// Fills `out` (the request's rectangle) by running `kernel` once per class of texels in it.
pub fn run(
    request: &EvalRequest,
    input: &FilterInput,
    radius: u32,
    out: &mut [[f32; 4]],
    mut kernel: impl FnMut(&Plane) -> Vec<[f32; 4]>,
) -> Result<(), String> {
    let rect = request.rect;
    if out.len() < rect.area() || input.pixels.len() < input.rect.area() {
        return Err("filter buffers have the wrong size".into());
    }
    let classes = Classes { surface: request.surface, size: request.set.resolution };
    let (plane, class) = gather(rect, input, radius, &classes);
    let r = radius as usize;
    let mut found: BTreeMap<u32, Bounds> = BTreeMap::new();
    for y in r..r + rect.h as usize {
        for x in r..r + rect.w as usize {
            let c = class.get(y * plane.w + x).copied().unwrap_or(VOID);
            found.entry(c).or_insert(Bounds { x0: x, y0: y, x1: x, y1: y }).add(x, y);
        }
    }
    for (c, b) in found {
        // the class's part of the plane: its output texels and the radius around them
        let (sx0, sy0) = (b.x0 - r, b.y0 - r);
        let (sw, sh) = (b.x1 - b.x0 + 1 + 2 * r, b.y1 - b.y0 + 1 + 2 * r);
        let mut px = Vec::with_capacity(sw * sh);
        let mut keep = Vec::with_capacity(sw * sh);
        for y in sy0..sy0 + sh {
            let row = y * plane.w;
            px.extend(plane.px.get(row + sx0..row + sx0 + sw).unwrap_or_default());
            keep.extend(class.get(row + sx0..row + sx0 + sw).unwrap_or_default().iter().map(|k| *k == c));
        }
        if px.len() != sw * sh || !pad(&mut px, &keep, sw, sh) {
            continue;
        }
        let part = Plane { x0: plane.x0 + sx0 as i64, y0: plane.y0 + sy0 as i64, w: sw, h: sh, px };
        let result = kernel(&part);
        if result.len() != sw * sh {
            return Err("filter result has the wrong size".into());
        }
        for y in b.y0..=b.y1 {
            for x in b.x0..=b.x1 {
                if class.get(y * plane.w + x) != Some(&c) {
                    continue;
                }
                let value = result.get((y - sy0) * sw + (x - sx0)).copied().unwrap_or([0.0; 4]);
                if let Some(o) = out.get_mut((y - r) * rect.w as usize + (x - r)) {
                    *o = value;
                }
            }
        }
    }
    Ok(())
}

/// The output rectangle grown by `radius` (beyond the texture too) with the input's texels and every texel's
/// class; texels the input does not cover are void.
fn gather(rect: Rect, input: &FilterInput, radius: u32, classes: &Classes) -> (Plane, Vec<u32>) {
    let r = radius as i64;
    let (x0, y0) = (rect.x as i64 - r, rect.y as i64 - r);
    let (w, h) = (rect.w as usize + 2 * radius as usize, rect.h as usize + 2 * radius as usize);
    let mut plane = Plane::filled(x0, y0, w, h, [0.0; 4]);
    let mut class = vec![VOID; w * h];
    let i = input.rect;
    for (k, (p, c)) in plane.px.iter_mut().zip(class.iter_mut()).enumerate() {
        let (x, y) = (x0 + (k % w) as i64, y0 + (k / w) as i64);
        let (ix, iy) = (x - i.x as i64, y - i.y as i64);
        if ix < 0 || iy < 0 || ix >= i.w as i64 || iy >= i.h as i64 {
            continue;
        }
        let Some(v) = input.pixels.get(iy as usize * i.w as usize + ix as usize) else { continue };
        *p = *v;
        *c = classes.at(x, y);
    }
    (plane, class)
}
