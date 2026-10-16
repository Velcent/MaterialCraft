//! [`Score`]: a composite of a channel against Painter's texture cache, as the final look: both over
//! Painter's channel defaults (the cache's premultiplied value plus `(1 − coverage) × default`, in the
//! channel's working space). Differences are in 8-bit steps of the stored encoding (1 = 1/255).

use crate::translate::channel_table::painter_default;
use materialcraft::paint::working_space::{SpaceConverter, WorkingSpace};
use materialcraft::paint::{ChannelSpec, Rect, TiledImage};
use materialcraft::tex::Tex;

/// Mean at or below this many steps counts as reproduced (rounding level).
pub const EXACT_STEPS: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Score {
    /// Mean absolute difference over texels and components.
    pub mean: f32,
    pub max: f32,
}

impl Score {
    pub fn reproduced(&self) -> bool {
        self.mean <= EXACT_STEPS
    }
}

/// Compares `ours` (straight stored values, composited over Painter's defaults) with a cache image (straight
/// values, alpha = coverage). Images of different sizes compare as far as both reach.
pub fn score(ours: &Tex, cache: &TiledImage, spec: &ChannelSpec) -> Score {
    let space = SpaceConverter::new(WorkingSpace::of(spec.color_space));
    let default = space.to_working(painter_default(spec.channel));
    let components = if spec.channel.is_color() { 3 } else { 1 };
    let (w, h) = (ours.w.min(cache.width() as usize), ours.h.min(cache.height() as usize));
    let mut row = vec![[0.0f32; 4]; w];
    let (mut sum, mut max, mut n) = (0.0f64, 0.0f32, 0usize);
    for y in 0..h {
        let r = Rect {
            x: 0,
            y: y as u32,
            w: w as u32,
            h: 1,
        };
        cache.read_region(r, &mut row);
        for (x, c) in row.iter().enumerate() {
            let a = c[3];
            let straight = space.to_working([c[0], c[1], c[2], 1.0]);
            let mut fin = [0.0f32; 4];
            for i in 0..3 {
                fin[i] = straight[i] * a + (1.0 - a) * default[i];
            }
            let mut stored = [fin];
            space.to_stored_all(&mut stored);
            let o = ours.px[y * ours.w + x];
            for i in 0..components {
                let d = (o[i] - stored[0][i]).abs() * 255.0;
                let d = if d.is_finite() { d } else { 255.0 };
                sum += d as f64;
                max = max.max(d);
                n += 1;
            }
        }
    }
    Score {
        mean: if n == 0 { 0.0 } else { (sum / n as f64) as f32 },
        max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::channel_table::channel_spec;
    use materialcraft::paint::{Channel, PixelFormat, PixelLayout, SampleType};

    #[test]
    fn uncovered_texels_compare_against_painter_defaults() {
        let spec = channel_spec(Channel::Roughness, Some(1));
        let mut cache = TiledImage::new(2, 1, PixelFormat::new(PixelLayout::GrayAlpha, SampleType::U8)).unwrap();
        cache.set(0, 0, [0.8, 0.8, 0.8, 1.0]);
        let ours = Tex {
            w: 2,
            h: 1,
            px: vec![[0.8, 0.8, 0.8, 1.0], [0.3, 0.3, 0.3, 1.0]],
        };
        let s = score(&ours, &cache, &spec);
        assert!(s.mean < 0.5 && s.reproduced(), "{s:?}");
        let off = Tex {
            w: 2,
            h: 1,
            px: vec![[0.0; 4], [0.3, 0.3, 0.3, 1.0]],
        };
        assert!(score(&off, &cache, &spec).max > 200.0);
    }
}
