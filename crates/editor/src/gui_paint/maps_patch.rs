//! The GPU patch for the tiles a brush stroke changed: the set's final maps over the tiles they reach
//! ([`MaterialMaps::build_tiles`]) turned into the bytes the whole preparation has there ([`gpu3d::prepare_patch`]).
//! Only for the lit material view without a quick mask tint, a square set the viewer shows at full size, and a change
//! that does not reach most of the set ([`MaterialMaps::tiles_reached`]); otherwise `None` and the caller composites
//! the whole set. Kept masks, fills and composites come from the stack cache, which keeps what the change does not
//! reach.

use super::channel_view::{ChannelView, Shown};
use materialcraft::generators;
use materialcraft::gpu3d::{self, MapsPatch, TileInputs};
use materialcraft::paint::stack::StackCache;
use materialcraft::paint::{Channel, FillContext, MaterialMaps, OutputOptions, TextureSet};

/// Largest map side the viewer uploads unscaled (see `gpu3d::prepare`).
const MAX_UNSCALED: u32 = 4096;

/// Whether maps shown as `shown` can be patched at all (the tint and isolation are checked by the caller).
pub fn patchable(shown: &Shown, set: &TextureSet) -> bool {
    let (w, h) = set.resolution;
    shown.view == ChannelView::Material && w == h && w > 0 && w <= MAX_UNSCALED
}

/// The patch for the tiles `changed` (tile coordinates of the set's grid), or `None` (see the module docs).
/// The images of the layers `layers` (ids) changed there. `cache` is the stack cache with the context of `fills`.
pub fn patch(
    set: &TextureSet,
    options: &OutputOptions,
    fills: &FillContext,
    shown: &Shown,
    cache: Option<(&StackCache, u64)>,
    layers: &[String],
    changed: &[(u32, u32)],
) -> Option<MapsPatch> {
    if !patchable(shown, set) {
        return None;
    }
    let ids: Vec<&str> = layers.iter().map(String::as_str).collect();
    let reached = MaterialMaps::tiles_reached(set, options, generators::builtin(), fills, &ids, changed).ok()?;
    let session = cache.map(|(c, context)| c.session(set, context, Some(&reached.area)));
    let tiles = MaterialMaps::build_tiles(set, options, generators::builtin(), fills, session.as_ref(), &reached.tiles).ok()?;
    let emissive = set.has_channel(Channel::Emissive);
    let height = set.has_channel(Channel::Height);
    let inputs: Vec<TileInputs> = tiles
        .iter()
        .map(|t| TileInputs {
            x: t.rect.x as usize,
            y: t.rect.y as usize,
            w: t.rect.w as usize,
            h: t.rect.h as usize,
            base: Some(&t.base_color),
            normal: Some(&t.normal),
            rough: Some(&t.roughness),
            metal: Some(&t.metallic),
            ao: Some(&t.ambient_occlusion),
            emissive: emissive.then_some(t.emissive.as_slice()),
            height: height.then_some(t.height.as_slice()),
        })
        .collect();
    Some(gpu3d::prepare_patch(&inputs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_paint::channel_display::ChannelDisplay;
    use materialcraft::paint::{BlendMode, Fill, Layer, LayerKind, NO_FILL_CONTEXT, PaintContent, PixelFormat, PixelLayout, SampleType, TiledImage};

    fn set(size: u32) -> TextureSet {
        let mut s = TextureSet::new("s", "S", (size, size)).with_channel(Channel::BaseColor).with_channel(Channel::Height).with_channel(Channel::Roughness);
        s.layers.push(Layer::new("f", "F", LayerKind::Fill(Fill::uniform(vec![(Channel::BaseColor, [0.4; 4])]))).with_blend(
            Channel::BaseColor,
            BlendMode::Normal,
            1.0,
        ));
        let mut img = TiledImage::new(size, size, PixelFormat::new(PixelLayout::Rgba, SampleType::U8)).expect("image");
        for y in 100..160 {
            for x in 100..160 {
                img.set(x, y, [0.9, 0.1, 0.1, 1.0]);
            }
        }
        let mut height = img.clone();
        height.fill([0.0; 4]);
        for y in 120..140 {
            for x in 120..140 {
                height.set(x, y, [1.0, 1.0, 1.0, 1.0]);
            }
        }
        let p = PaintContent::from_images(vec![(Channel::BaseColor, img), (Channel::Height, height)]);
        s.layers.push(Layer::new("p", "P", LayerKind::Paint(p)).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0).with_blend(
            Channel::Height,
            BlendMode::Normal,
            1.0,
        ));
        s
    }

    #[test]
    fn a_patch_has_the_bytes_of_the_whole_maps() {
        let s = set(256);
        let shown = Shown { view: ChannelView::Material, channel: ChannelDisplay::default() };
        let o = OutputOptions::default();
        let whole = shown.prepare(&MaterialMaps::build_with(&s, &o, generators::builtin(), &NO_FILL_CONTEXT), &s);
        let p = patch(&s, &o, &NO_FILL_CONTEXT, &shown, None, &["p".to_string()], &[(0, 0)]).expect("patchable");
        assert_eq!(p.tiles.len(), 4, "the tile and its neighbours (the normal reads across)");
        for t in &p.tiles {
            for row in 0..t.h {
                let at = ((t.y + row) * whole.size + t.x) * 4;
                let len = t.w * 4;
                assert_eq!(t.base[row * len..(row + 1) * len], whole.base[at..at + len]);
                assert_eq!(t.normal[row * len..(row + 1) * len], whole.normal[at..at + len]);
                assert_eq!(t.rma[row * len..(row + 1) * len], whole.rma[at..at + len]);
            }
        }
        let channel = Shown { view: ChannelView::Single(Channel::BaseColor), ..shown };
        assert!(patch(&s, &o, &NO_FILL_CONTEXT, &channel, None, &["p".to_string()], &[(0, 0)]).is_none());
    }
}
