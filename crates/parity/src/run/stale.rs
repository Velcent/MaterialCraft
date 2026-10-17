//! [`Staleness`]: tells a reference that no longer matches what we bake from, so its numbers stay out of the medians
//! instead of hiding how close the baker gets. Two things go stale between Painter's bake and today's files, and both
//! show in the data alone:
//!
//! - the high-poly source: the file found on disk is not the one Painter baked from (or not where Painter had it),
//!   so our rays miss it where Painter's hit. Measured as the share of compared texels (all inside Painter's ray
//!   coverage) where our projection found the high mesh.
//! - the UV layout: the mesh was unwrapped or its islands moved after the bake, so Painter's map is offset from ours.
//!   Measured by sliding our map over Painter's: when some offset of several texels fits clearly better than none,
//!   the two layouts differ. Painter dilates its maps (and stores coverage in their alpha), so comparing island
//!   outlines directly is not reliable; the offset test reads only the compared values.

use materialcraft::tex::Tex;

/// Below this share of compared texels where our rays found the high mesh, the high source differs. Measured over
/// the corpus, shares fall into two groups: below 0.46 (a different or misplaced file) and above 0.8.
pub const MIN_HIT_SHARE: f64 = 0.5;
/// The offset test runs on maps shrunk to at most this size (offsets are searched in its texels).
const OFFSET_GRID: usize = 256;
/// Offsets searched, in texels of the shrunk map, each way.
const MAX_OFFSET: i64 = 5;
/// Offsets shorter than this (shrunk texels) are resampling, not another layout.
const MIN_OFFSET: i64 = 2;
/// An offset counts when it brings the error down to this share of the error without one…
const OFFSET_GAIN: f64 = 0.6;
/// …and by at least this much (8-bit steps), so near-perfect matches never count.
const OFFSET_MIN_STEPS: f64 = 3.0;
/// Fewer compared texels in the shrunk map than this make no verdict.
const MIN_TEXELS: usize = 200;

/// The verdict on one reference.
pub struct Staleness;

impl Staleness {
    /// Why the high source differs, if it does: our rays found the high mesh in less than [`MIN_HIT_SHARE`] of the
    /// texels in `mask` (where Painter's found one). `hits`: 1 where ours did, 0 where not.
    pub fn high_source(hits: &[f32], mask: &[bool]) -> Option<String> {
        let share = hit_share(hits, mask)?;
        (share < MIN_HIT_SHARE).then(|| {
            format!("stale reference: high-poly source differs (our rays found the high mesh in {:.1} % of the texels Painter's covered: another file, or placed elsewhere)", 100.0 * share)
        })
    }

    /// Why the UV layout differs, if it does: an offset of `ours` against `theirs` (inside `mask`, components
    /// `channels`) that fits clearly better than none.
    pub fn layout(ours: &Tex, theirs: &Tex, mask: &[bool], channels: &[usize]) -> Option<String> {
        let (dx, dy, at_zero, best) = best_offset(ours, theirs, mask, channels)?;
        let far = dx.abs().max(dy.abs()) >= MIN_OFFSET;
        (far && best <= OFFSET_GAIN * at_zero && at_zero - best >= OFFSET_MIN_STEPS).then(|| {
            let scale = ours.w.max(ours.h) as f64 / ours.w.max(ours.h).min(OFFSET_GRID).max(1) as f64;
            format!(
                "stale reference: UV layout differs from the mesh (moved by {:.0}, {:.0} texels the error drops from {at_zero:.1} to {best:.1})",
                dx as f64 * scale,
                dy as f64 * scale
            )
        })
    }
}

/// Share of masked texels where `hits` (1 hit, 0 missed) says our rays found the high mesh.
pub fn hit_share(hits: &[f32], mask: &[bool]) -> Option<f64> {
    let (total, hit) = hits
        .iter()
        .zip(mask)
        .filter(|(_, m)| **m)
        .fold((0usize, 0.0f64), |(t, h), (v, _)| {
            (t + 1, h + f64::from(v.clamp(0.0, 1.0)))
        });
    (total > 0).then(|| hit / total as f64)
}

/// A map shrunk for the offset search: values and which texels are fully inside the mask.
struct Grid {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
    inside: Vec<bool>,
}

impl Grid {
    fn of(tex: &Tex, mask: &[bool]) -> Option<Grid> {
        let factor = tex.w.max(tex.h).div_ceil(OFFSET_GRID).max(1);
        let (w, h) = (tex.w / factor, tex.h / factor);
        if w == 0 || h == 0 || mask.len() != tex.w * tex.h || tex.px.len() != tex.w * tex.h {
            return None;
        }
        let mut px = vec![[0.0f32; 4]; w * h];
        let mut inside = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let (mut sum, mut all) = ([0.0f32; 4], true);
                for sy in 0..factor {
                    for sx in 0..factor {
                        let i = (y * factor + sy) * tex.w + x * factor + sx;
                        all &= mask.get(i).copied().unwrap_or(false);
                        if let Some(p) = tex.px.get(i) {
                            (0..4).for_each(|c| sum[c] += p[c]);
                        }
                    }
                }
                let n = (factor * factor) as f32;
                if let (Some(o), Some(m)) = (px.get_mut(y * w + x), inside.get_mut(y * w + x)) {
                    *o = sum.map(|s| s / n);
                    *m = all;
                }
            }
        }
        Some(Grid { w, h, px, inside })
    }

    fn at(&self, x: i64, y: i64) -> Option<(&[f32; 4], bool)> {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return None;
        }
        let i = y as usize * self.w + x as usize;
        Some((self.px.get(i)?, self.inside.get(i).copied().unwrap_or(false)))
    }
}

/// The offset (dx, dy) in shrunk texels at which `ours` shifted fits `theirs` best, with the mean absolute error
/// (8-bit steps) without an offset and at the best one.
fn best_offset(ours: &Tex, theirs: &Tex, mask: &[bool], channels: &[usize]) -> Option<(i64, i64, f64, f64)> {
    let (a, b) = (Grid::of(ours, mask)?, Grid::of(theirs, mask)?);
    let error = |dx: i64, dy: i64| -> Option<f64> {
        let (mut sum, mut n) = (0.0f64, 0usize);
        for y in 0..b.h as i64 {
            for x in 0..b.w as i64 {
                let (Some((t, true)), Some((o, true))) = (b.at(x, y), a.at(x + dx, y + dy)) else {
                    continue;
                };
                sum += channels
                    .iter()
                    .filter_map(|&c| Some(f64::from((o.get(c)? - t.get(c)?).abs())))
                    .sum::<f64>();
                n += 1;
            }
        }
        (n >= MIN_TEXELS).then(|| 255.0 * sum / (n * channels.len().max(1)) as f64)
    };
    let at_zero = error(0, 0)?;
    let mut best = (0, 0, at_zero);
    for dy in -MAX_OFFSET..=MAX_OFFSET {
        for dx in -MAX_OFFSET..=MAX_OFFSET {
            if let Some(e) = error(dx, dy)
                && e < best.2
            {
                best = (dx, dy, e);
            }
        }
    }
    Some((best.0, best.1, at_zero, best.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64 x 64 map with a bright square at `x0`, `y0`, all inside the mask.
    fn square(x0: usize, y0: usize) -> Tex {
        let px = (0..64 * 64)
            .map(|i| {
                let (x, y) = (i % 64, i / 64);
                let v = if (x0..x0 + 16).contains(&x) && (y0..y0 + 16).contains(&y) {
                    1.0
                } else {
                    0.2
                };
                [v, v, v, 1.0]
            })
            .collect();
        Tex { w: 64, h: 64, px }
    }

    #[test]
    fn an_offset_layout_is_stale_the_same_one_is_not() {
        let mask = vec![true; 64 * 64];
        assert!(Staleness::layout(&square(20, 20), &square(20, 20), &mask, &[0]).is_none());
        // one texel apart: resampling, not another layout
        assert!(Staleness::layout(&square(21, 20), &square(20, 20), &mask, &[0]).is_none());
        let why = Staleness::layout(&square(24, 20), &square(20, 20), &mask, &[0]).unwrap_or_default();
        assert!(why.starts_with("stale reference: UV layout"), "{why}");
        // nothing to compare
        assert!(Staleness::layout(&square(24, 20), &square(20, 20), &[false; 64 * 64], &[0]).is_none());
    }

    #[test]
    fn misses_mean_another_high_source() {
        let mask = [true, true, true, false];
        assert_eq!(hit_share(&[1.0, 1.0, 0.0, 0.0], &mask), Some(2.0 / 3.0));
        assert!(Staleness::high_source(&[1.0, 1.0, 0.0, 0.0], &mask).is_none());
        let why = Staleness::high_source(&[0.0, 0.0, 1.0, 1.0], &mask).unwrap_or_default();
        assert!(why.contains("high-poly source differs"), "{why}");
        assert_eq!(hit_share(&[1.0], &[false]), None);
    }
}
