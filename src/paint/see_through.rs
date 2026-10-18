//! See-through paint content (ADR 0008 mc2-q-pt-03c; PT-25, PT-29): a paint layer whose strokes (smudge, clone,
//! blur, paint, erase) work on the composite of the layers below it instead of on its own pixels alone. The layers
//! below stay intact; the layer's images are what its recorded strokes made of that composite, and they are
//! painted again from the record whenever the composite below changes.
//!
//! The pieces the core provides (the replay itself needs the brush engine, which lives beside the core):
//! - [`BelowKey`]: what the composite below a layer depends on, compared cheaply (images that share their tiles
//!   compare by pointer), to tell when the layer must be painted again.
//! - [`base_images`]: the composite below, per channel of the layer, in the layer's image formats: the starting
//!   point of every replay, and what a live stroke reads where the layer has no pixels yet.
//! - [`fill_from`] and [`trim_to`]: a stroke on see-through content starts from the base where the layer has no
//!   tile, and afterwards frees every tile it did not touch (it still shares the base's tile), so the layer keeps
//!   pixels only where strokes went and the composite below shows through everywhere else.

use super::channel::{Channel, ChannelSpec};
use super::image::TiledImage;
use super::layer::{Layer, LayerKind};
use super::pixel::PixelFormat;
use super::stack::Compositor;
use super::texture_set::TextureSet;

/// What the composite below a layer depends on: the set's size and channels, the layers composited before it (with
/// their pixels, which share tiles with the document's) and the formats of the layer's images.
#[derive(Debug, Clone, PartialEq)]
pub struct BelowKey {
    resolution: (u32, u32),
    channels: Vec<ChannelSpec>,
    below: Vec<Layer>,
    formats: Vec<(Channel, PixelFormat)>,
}

impl BelowKey {
    /// The key of layer `id` of `set`; `None` if it is not a paint layer of the set.
    pub fn of(set: &TextureSet, id: &str) -> Option<BelowKey> {
        let layer = set.layer(id)?;
        let LayerKind::Paint(p) = &layer.kind else { return None };
        let mut below = Vec::new();
        collect_below(&set.layers, layer, &mut below);
        Some(BelowKey { resolution: set.resolution, channels: set.channels.clone(), below, formats: p.images.iter().map(|(c, i)| (*c, i.format())).collect() })
    }
}

/// Clones of the layers composited before `stop` (folders that contain it are entered, as the compositor does);
/// true once `stop` was reached.
fn collect_below(layers: &[Layer], stop: &Layer, out: &mut Vec<Layer>) -> bool {
    for l in layers {
        if std::ptr::eq(l, stop) {
            return true;
        }
        if let LayerKind::Group(children) = &l.kind
            && contains(children, stop)
        {
            return collect_below(children, stop, out);
        }
        out.push(l.clone());
    }
    false
}

fn contains(layers: &[Layer], stop: &Layer) -> bool {
    layers.iter().any(|l| std::ptr::eq(l, stop) || matches!(&l.kind, LayerKind::Group(c) if contains(c, stop)))
}

/// The ids of the set's see-through paint layers, bottom to top (a see-through layer above another sees it).
pub fn see_through_layers(set: &TextureSet) -> Vec<String> {
    let mut out = Vec::new();
    collect_see_through(&set.layers, &mut out);
    out
}

fn collect_see_through(layers: &[Layer], out: &mut Vec<String>) {
    for l in layers {
        match &l.kind {
            LayerKind::Paint(p) if p.see_through => out.push(l.id.clone()),
            LayerKind::Group(children) => collect_see_through(children, out),
            _ => {}
        }
    }
}

/// The composite below layer `id`, one image per image of the layer (same channel order and formats, every tile
/// allocated). `None` if it is not a paint layer of the compositor's set.
pub fn base_images(compositor: &Compositor, set: &TextureSet, id: &str) -> Option<Vec<(Channel, TiledImage)>> {
    let LayerKind::Paint(p) = &set.layer(id)?.kind else { return None };
    p.images.iter().map(|(c, img)| Some((*c, compositor.below_image(id, *c, img.format())?))).collect()
}

/// Gives `image` the tiles of `base` (same size and format) where it has none.
pub fn fill_from(image: &mut TiledImage, base: &TiledImage) {
    if image.size() != base.size() || image.format() != base.format() {
        return;
    }
    let missing: Vec<(u32, u32)> = image.all_tiles().into_iter().filter(|&(tx, ty)| image.tile(tx, ty).is_none()).collect();
    for (tx, ty) in missing {
        if let Some(t) = base.tile(tx, ty) {
            // same size and format, so the tile fits
            let _ = image.replace_tile(tx, ty, Some(t.clone()));
        }
    }
}

/// Frees the tiles of `image` that are still `base`'s own (no stroke wrote them).
pub fn trim_to(image: &mut TiledImage, base: &TiledImage) {
    if image.size() != base.size() || image.format() != base.format() {
        return;
    }
    let shared: Vec<(u32, u32)> =
        image.all_tiles().into_iter().filter(|&(tx, ty)| matches!((image.tile(tx, ty), base.tile(tx, ty)), (Some(a), Some(b)) if a.same(b))).collect();
    for (tx, ty) in shared {
        let _ = image.replace_tile(tx, ty, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::edit::ops::{NewLayer, create};
    use crate::paint::edit::{LayerProperty, LayerRef, Transaction};
    use crate::paint::{Document, Fill};

    fn doc() -> (Document, String, String) {
        let mut doc = Document::default();
        doc.texture_sets.push(TextureSet::new("s", "S", (256, 256)).with_channel(Channel::Roughness));
        let mut tx = Transaction::new(&mut doc);
        let fill = create(&mut tx, "s", NewLayer::Fill, None).unwrap();
        let top = create(&mut tx, "s", NewLayer::SeeThrough, Some(&fill)).unwrap();
        tx.commit();
        (doc, fill, top)
    }

    #[test]
    fn the_key_follows_the_layers_below_only() {
        let (mut doc, fill, top) = doc();
        let set = &doc.texture_sets[0];
        assert_eq!(see_through_layers(set), vec![top.clone()]);
        let key = BelowKey::of(set, &top).unwrap();
        assert!(BelowKey::of(set, &fill).is_none(), "fill layers have no key");
        // a change above leaves it, a change below changes it
        let mut tx = Transaction::new(&mut doc);
        tx.apply(crate::paint::edit::Edit::SetLayer { layer: LayerRef::new("s", &top), property: LayerProperty::Opacity(0.5) }).unwrap();
        tx.commit();
        assert_eq!(BelowKey::of(&doc.texture_sets[0], &top).unwrap(), key);
        let mut tx = Transaction::new(&mut doc);
        let value = Fill::uniform(vec![(Channel::Roughness, [0.2; 4])]);
        tx.apply(crate::paint::edit::Edit::SetLayer { layer: LayerRef::new("s", &fill), property: LayerProperty::Fill(value) }).unwrap();
        tx.commit();
        assert_ne!(BelowKey::of(&doc.texture_sets[0], &top).unwrap(), key);
    }

    #[test]
    fn base_images_hold_the_composite_below_and_fill_and_trim_share_tiles() {
        let (doc, _, top) = doc();
        let set = &doc.texture_sets[0];
        let base = base_images(&Compositor::new(set), set, &top).unwrap();
        let (channel, img) = &base[0];
        assert_eq!(*channel, Channel::Roughness);
        let v = img.get(10, 10);
        assert!((v[0] - Channel::Roughness.default_value()[0]).abs() < 1e-2 && v[3] > 0.99, "{v:?}");
        let mut layer_img = TiledImage::new(256, 256, img.format()).unwrap();
        layer_img.set(200, 200, [1.0; 4]);
        fill_from(&mut layer_img, img);
        assert_eq!(layer_img.allocated_tiles(), img.allocated_tiles());
        assert_eq!(layer_img.get(10, 10), v);
        trim_to(&mut layer_img, img);
        assert_eq!(layer_img.allocated_tiles(), 1, "only the tile a stroke wrote stays");
        assert_eq!(layer_img.get(200, 200), [1.0; 4]);
    }
}
