//! [`CacheAgreement`]: whether a fully computable stack really shows what Painter saved. The imported layers
//! (over Painter's defaults) are composited per channel and scored against the flattened cache layer. A stack
//! can be exact by its parts and still disagree when the cache is stale (Painter re-renders sets lazily, so a
//! set edited last, e.g. a folder hidden, may keep the old pixels).

use super::cache_score::{score, Score};
use materialcraft::paint::{Channel, Compositor, Layer, LayerKind, TextureSet};

/// Mean difference above which a channel counts as disagreeing (8-bit steps). Above the 0.5 of float vs
/// 8-bit rounding, so 8-bit constants rounded the other way (1 step) still agree.
pub const AGREEMENT_STEPS: f32 = 1.0;

/// True when a channel's score is beyond rounding (or not a number).
pub fn disagrees(s: &Score) -> bool {
    s.mean.is_nan() || s.mean > AGREEMENT_STEPS
}

/// The channels whose composite differs from the cache, with the measured difference.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CacheAgreement {
    pub disagreeing: Vec<(Channel, Score)>,
}

impl CacheAgreement {
    /// Composites `imported` over `defaults` with `set`'s channels and resolution and scores every channel
    /// the cache layer `result` has an image for.
    pub fn measure(set: &TextureSet, defaults: &Layer, imported: &[Layer], result: &Layer) -> CacheAgreement {
        let LayerKind::Paint(cache) = &result.kind else {
            return CacheAgreement::default();
        };
        let mut stack = TextureSet::new(set.id.clone(), set.name.clone(), set.resolution);
        stack.channels = set.channels.clone();
        stack.layers.push(defaults.clone());
        stack.layers.extend(imported.iter().cloned());
        let compositor = Compositor::new(&stack);
        let mut disagreeing = Vec::new();
        for (channel, image) in &cache.images {
            let Some(spec) = set.channel_spec(*channel) else {
                continue;
            };
            let (ours, _) = compositor.composite_tex(*channel);
            let s = score(&ours, image, spec);
            if disagrees(&s) {
                disagreeing.push((*channel, s));
            }
        }
        CacheAgreement { disagreeing }
    }

    pub fn agrees(&self) -> bool {
        self.disagreeing.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::channel_table::channel_spec;
    use materialcraft::paint::{BlendMode, Fill, PaintContent, TiledImage};

    fn set_and_cache(cached: f32) -> (TextureSet, Layer) {
        let mut set = TextureSet::new("s", "S", (4, 4));
        set.channels.push(channel_spec(Channel::Roughness, Some(1)));
        let mut image = TiledImage::new(4, 4, set.channels[0].layer_format()).unwrap();
        image.fill([cached, cached, cached, 1.0]);
        let mut content = PaintContent::default();
        content.set_image(Channel::Roughness, image);
        (set, Layer::new("r", "R", LayerKind::Paint(content)))
    }

    #[test]
    fn stale_caches_disagree_and_rounding_does_not() {
        let fill = Layer::new(
            "f",
            "F",
            LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [0.3; 4])])),
        )
        .with_blend(Channel::Roughness, BlendMode::Normal, 1.0);
        let defaults = Layer::new("d", "D", LayerKind::Fill(Fill::uniform(vec![])));
        let (set, result) = set_and_cache(76.0 / 255.0);
        assert!(CacheAgreement::measure(&set, &defaults, std::slice::from_ref(&fill), &result).agrees());
        let (set, result) = set_and_cache(0.9);
        let a = CacheAgreement::measure(&set, &defaults, &[fill], &result);
        assert_eq!(a.disagreeing.len(), 1);
        assert!(a.disagreeing[0].1.mean > 100.0);
    }
}
