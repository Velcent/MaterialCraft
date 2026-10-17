//! Sampling a projected image: [`Texels`] (anything with addressable texels), nearest and bilinear lookups with a
//! [`Repeat`] per axis, and [`ImageLevels`], the prefiltered level chain behind [`Filter::Smooth`].
//!
//! Image coordinates run 0…1 across the image; texel `i` covers `i / n … (i + 1) / n`, so its centre is at
//! `(i + 0.5) / n`. On a repeating axis coordinates wrap; on a non-repeating one texel indices clamp at the edge
//! and coordinates outside 0…1 have no value (`None`: the fill has no coverage there).

use super::data::{Filter, Repeat};
use crate::tex::{Tex, ffloor};

/// An image the sampler can read texel by texel.
pub trait Texels: Sync {
    fn size(&self) -> (u32, u32);
    /// Straight RGBA of texel `(x, y)`; both are inside the image.
    fn texel(&self, x: u32, y: u32) -> [f32; 4];
}

impl Texels for Tex {
    fn size(&self) -> (u32, u32) {
        (self.w as u32, self.h as u32)
    }

    fn texel(&self, x: u32, y: u32) -> [f32; 4] {
        self.px.get(y as usize * self.w + x as usize).copied().unwrap_or([0.0; 4])
    }
}

/// The texel index for integer coordinate `i` on an axis of `n` texels (`n` > 0).
#[inline]
fn index(i: i64, n: u32, wrap: bool) -> u32 {
    let n = i64::from(n);
    let i = if wrap { i.rem_euclid(n) } else { i.clamp(0, n - 1) };
    i as u32
}

/// True if `t` lies inside 0…1 or the axis repeats.
#[inline]
fn covered(t: f32, wrap: bool) -> bool {
    wrap || (0.0..=1.0).contains(&t)
}

/// The nearest texel to `uv`.
pub fn nearest<T: Texels + ?Sized>(img: &T, uv: [f32; 2], repeat: Repeat) -> Option<[f32; 4]> {
    let (w, h) = img.size();
    let [u, v] = finite(uv)?;
    if w == 0 || h == 0 || !covered(u, repeat.u()) || !covered(v, repeat.v()) {
        return None;
    }
    let x = index(ffloor(u * w as f32) as i64, w, repeat.u());
    let y = index(ffloor(v * h as f32) as i64, h, repeat.v());
    Some(img.texel(x, y))
}

/// Bilinear interpolation of the four texels around `uv`.
pub fn bilinear<T: Texels + ?Sized>(img: &T, uv: [f32; 2], repeat: Repeat) -> Option<[f32; 4]> {
    let (w, h) = img.size();
    let [u, v] = finite(uv)?;
    if w == 0 || h == 0 || !covered(u, repeat.u()) || !covered(v, repeat.v()) {
        return None;
    }
    let x = u * w as f32 - 0.5;
    let y = v * h as f32 - 0.5;
    let (x0, y0) = (ffloor(x), ffloor(y));
    let (fx, fy) = (x - x0, y - y0);
    let (xa, xb) = (index(x0 as i64, w, repeat.u()), index(x0 as i64 + 1, w, repeat.u()));
    let (ya, yb) = (index(y0 as i64, h, repeat.v()), index(y0 as i64 + 1, h, repeat.v()));
    let (a, b, c, d) = (img.texel(xa, ya), img.texel(xb, ya), img.texel(xa, yb), img.texel(xb, yb));
    let mut o = [0.0; 4];
    for (i, o) in o.iter_mut().enumerate() {
        let (a, b, c, d) =
            (a.get(i).copied().unwrap_or(0.0), b.get(i).copied().unwrap_or(0.0), c.get(i).copied().unwrap_or(0.0), d.get(i).copied().unwrap_or(0.0));
        let top = a + (b - a) * fx;
        let bottom = c + (d - c) * fx;
        *o = top + (bottom - top) * fy;
    }
    Some(o)
}

fn finite(uv: [f32; 2]) -> Option<[f32; 2]> {
    (uv[0].is_finite() && uv[1].is_finite()).then_some(uv)
}

/// The prefiltered levels below an image: each halves both sides of the one above with a box filter (odd sides
/// round up; the last texel then repeats the edge) down to 1×1. Owned apart from the image, so it can be built
/// once and kept next to it.
#[derive(Debug, Clone, Default)]
pub struct LevelChain {
    coarser: Vec<Tex>,
}

impl LevelChain {
    /// The levels below `base` (none for a 1×1 or empty image).
    pub fn build(base: &dyn Texels) -> LevelChain {
        let mut coarser: Vec<Tex> = Vec::new();
        let (mut w, mut h) = base.size();
        while w > 1 || h > 1 {
            let (nw, nh) = (w.div_ceil(2), h.div_ceil(2));
            let above: &dyn Texels = match coarser.last() {
                Some(t) => t,
                None => base,
            };
            coarser.push(halve(above, nw, nh));
            (w, h) = (nw, nh);
        }
        LevelChain { coarser }
    }
}

/// An image with its prefiltered levels ([`LevelChain`]); level 0 is the image itself.
#[derive(Clone, Copy)]
pub struct ImageLevels<'a> {
    base: &'a dyn Texels,
    coarser: &'a [Tex],
}

impl<'a> ImageLevels<'a> {
    /// `chain` must have been built from `base`.
    pub fn new(base: &'a dyn Texels, chain: &'a LevelChain) -> ImageLevels<'a> {
        ImageLevels { base, coarser: &chain.coarser }
    }

    pub fn size(&self) -> (u32, u32) {
        self.base.size()
    }

    /// How many levels there are (at least 1).
    pub fn count(&self) -> usize {
        self.coarser.len() + 1
    }

    /// Level `i` (0 = the image).
    pub fn level(&self, i: usize) -> &'a dyn Texels {
        match i.checked_sub(1).and_then(|i| self.coarser.get(i)) {
            Some(t) => t,
            None => self.base,
        }
    }

    /// The value at `uv` for a texel whose footprint spans `footprint` image units, or `None` outside a
    /// non-repeating image.
    pub fn sample(&self, uv: [f32; 2], footprint: f32, filter: Filter, repeat: Repeat) -> Option<[f32; 4]> {
        match filter {
            Filter::Nearest => nearest(self.base, uv, repeat),
            Filter::Bilinear => bilinear(self.base, uv, repeat),
            Filter::Smooth => self.smooth(uv, footprint, repeat),
        }
    }

    /// Trilinear: bilinear in the two levels around the footprint, blended; plain bilinear when magnifying.
    fn smooth(&self, uv: [f32; 2], footprint: f32, repeat: Repeat) -> Option<[f32; 4]> {
        let (w, h) = self.size();
        let texels = footprint * w.max(h) as f32;
        if !texels.is_finite() || texels <= 1.0 || self.coarser.is_empty() {
            return bilinear(self.base, uv, repeat);
        }
        let lod = texels.log2().min((self.count() - 1) as f32);
        let lo = lod as usize;
        let t = lod - lo as f32;
        let a = bilinear(self.level(lo), uv, repeat)?;
        if t <= 1e-4 || lo + 1 >= self.count() {
            return Some(a);
        }
        let b = bilinear(self.level(lo + 1), uv, repeat)?;
        let mut out = a;
        for (o, b) in out.iter_mut().zip(b) {
            *o += (b - *o) * t;
        }
        Some(out)
    }
}

/// `src` box-filtered to `nw × nh` (half its size, rounded up).
fn halve(src: &dyn Texels, nw: u32, nh: u32) -> Tex {
    let (w, h) = src.size();
    let mut px = Vec::with_capacity(nw as usize * nh as usize);
    for y in 0..nh {
        for x in 0..nw {
            let xs = [(2 * x).min(w.saturating_sub(1)), (2 * x + 1).min(w.saturating_sub(1))];
            let ys = [(2 * y).min(h.saturating_sub(1)), (2 * y + 1).min(h.saturating_sub(1))];
            let mut sum = [0.0f32; 4];
            for yy in ys {
                for xx in xs {
                    let p = src.texel(xx, yy);
                    for (s, v) in sum.iter_mut().zip(p) {
                        *s += v;
                    }
                }
            }
            px.push(sum.map(|s| s * 0.25));
        }
    }
    Tex { w: nw as usize, h: nh as usize, px }
}
