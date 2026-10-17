//! [`QuickMaskTint`]: the quick mask shown as a red tint over the view (PT-42), laid into each texture set's GPU
//! maps by the maps worker. A snapshot of the masks (their tiles are shared, so it costs a pointer per tile).

use materialcraft::gpu3d::GpuMaps;
use materialcraft::paint::{Document, TiledImage};
use materialcraft_brush::QuickMask;
use std::collections::BTreeMap;

/// The tint colour (sRGB) and how strongly a fully protected texel takes it.
const TINT: [f32; 3] = [235.0, 45.0, 45.0];
const STRENGTH: f32 = 0.55;

/// The quick masks of a document's texture sets as the view shows them.
#[derive(Clone, Default)]
pub struct QuickMaskTint {
    masks: BTreeMap<String, TiledImage>,
    inverted: bool,
}

impl QuickMaskTint {
    /// The tint to show for `mask` over `doc`'s sets, or `None` while the quick mask is off.
    pub fn of(mask: &QuickMask, doc: &Document) -> Option<QuickMaskTint> {
        if !mask.is_shown() {
            return None;
        }
        let masks = doc.texture_sets.iter().filter_map(|s| Some((s.id.clone(), mask.image(&s.id)?.clone()))).collect();
        Some(QuickMaskTint { masks, inverted: mask.is_inverted() })
    }

    /// Tints texture set `set`'s maps where its mask protects (the base colour, and the emissive map the unlit
    /// views draw with).
    pub fn apply(&self, set: &str, maps: &mut GpuMaps) {
        let mask = self.masks.get(set);
        if mask.is_none() && !self.inverted {
            return; // an empty mask protects nothing
        }
        let size = maps.size;
        let protection = |i: usize| -> f32 {
            let v = match mask {
                Some(m) => {
                    let (x, y) = (i % size.max(1), i / size.max(1));
                    let mx = (x as u64 * u64::from(m.width()) / size.max(1) as u64) as u32;
                    let my = (y as u64 * u64::from(m.height()) / size.max(1) as u64) as u32;
                    m.get(mx, my)[0].clamp(0.0, 1.0)
                }
                None => 0.0,
            };
            if self.inverted { 1.0 - v } else { v }
        };
        tint(&mut maps.base, &protection);
        if let Some(e) = &mut maps.emissive {
            tint(e, &protection);
        }
    }
}

/// Mixes each RGBA8 pixel towards the tint by its protection.
fn tint(rgba: &mut [u8], protection: &dyn Fn(usize) -> f32) {
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        let k = protection(i) * STRENGTH;
        if k <= 0.0 {
            continue;
        }
        for (c, t) in px.iter_mut().zip(TINT) {
            *c = (f32::from(*c) + (t - f32::from(*c)) * k).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::PixelFormat;

    #[test]
    fn protected_texels_turn_red() {
        let mut m = TiledImage::new(2, 2, PixelFormat::from_id("gray8").unwrap()).unwrap();
        m.set(1, 0, [1.0; 4]);
        let t = QuickMaskTint { masks: [("s".to_string(), m)].into_iter().collect(), inverted: false };
        let mut maps = GpuMaps { size: 2, base: vec![100; 16], normal: Vec::new(), rma: Vec::new(), emissive: None, height: None };
        t.apply("s", &mut maps);
        assert_eq!(&maps.base[0..4], &[100; 4], "free");
        assert!(maps.base[4] > 150 && maps.base[5] < 100, "protected: {:?}", &maps.base[4..8]);
        let mut other = GpuMaps { size: 2, base: vec![100; 16], normal: Vec::new(), rma: Vec::new(), emissive: None, height: None };
        t.apply("other", &mut other);
        assert_eq!(other.base, vec![100; 16]);
    }
}
