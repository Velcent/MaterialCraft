//! Fitting a fragment to the texture set it is dropped into: every paint, effect and mask image is resampled to
//! the set's resolution (bilinear), and what the set does not declare is left out (blend entries, images and fill
//! values of undeclared channels), so the result is valid in the set. Fill images keep their size: fills sample
//! their images through the projection at any size.

use crate::paint::channel::Channel;
use crate::paint::effect::{EffectKind, MaskEffect};
use crate::paint::fill_image::ImageFill;
use crate::paint::image::TiledImage;
use crate::paint::layer::{Fill, FillSource, Layer, LayerKind};
use crate::paint::mask::Mask;
use crate::paint::texture_set::TextureSet;

/// Fits `layers` (children included) to `set`: images resampled to its resolution, undeclared channels dropped.
pub fn fit_layers(layers: &mut [Layer], set: &TextureSet) {
    let declared = |c: Channel| set.has_channel(c);
    for l in layers {
        fit_layer(l, set.resolution, &declared);
    }
}

/// Fits a mask's paint images to `resolution`.
pub fn fit_mask(mask: &mut Mask, resolution: (u32, u32)) {
    fit_mask_effects(&mut mask.effects, resolution);
}

/// Fits the paint images among mask effects to `resolution`.
pub fn fit_mask_effects(effects: &mut [MaskEffect], resolution: (u32, u32)) {
    for e in effects {
        if let EffectKind::Paint(p) = &mut e.kind {
            refit(&mut p.image, resolution);
        }
    }
}

fn fit_layer(l: &mut Layer, resolution: (u32, u32), declared: &dyn Fn(Channel) -> bool) {
    l.blend.retain(|b| declared(b.channel));
    match &mut l.kind {
        LayerKind::Paint(p) => {
            p.images.retain(|(c, _)| declared(*c));
            p.images.iter_mut().for_each(|(_, img)| refit(img, resolution));
        }
        LayerKind::Fill(f) => retain_fill(f, declared),
        LayerKind::Group(children) => children.iter_mut().for_each(|c| fit_layer(c, resolution, declared)),
    }
    for e in &mut l.effects {
        e.blend.retain(|b| declared(b.channel));
        match &mut e.kind {
            EffectKind::Paint(p) => {
                p.images.retain(|(c, _)| declared(*c));
                p.images.iter_mut().for_each(|(_, img)| refit(img, resolution));
            }
            EffectKind::Fill(f) => retain_fill(f, declared),
            _ => {}
        }
    }
    if let Some(m) = &mut l.mask {
        fit_mask(m, resolution);
    }
}

fn retain_fill(f: &mut Fill, declared: &dyn Fn(Channel) -> bool) {
    match &mut f.source {
        FillSource::Uniform(values) => values.retain(|(c, _)| declared(*c)),
        FillSource::Material { .. } => {}
        FillSource::Image(fill) => {
            let mut kept = ImageFill::default();
            for (c, img) in fill.images().filter(|(c, _)| declared(*c)) {
                kept.set_image(c, img.image().clone());
            }
            for &(c, v) in fill.values().iter().filter(|(c, _)| declared(*c)) {
                kept.set_value(c, v);
            }
            *fill = kept;
        }
    }
}

fn refit(img: &mut TiledImage, size: (u32, u32)) {
    if img.size() != size {
        *img = resample(img, size);
    }
}

/// `img` at `size` (bilinear, texel centres aligned, edges clamped); an image with nothing painted stays empty.
pub fn resample(img: &TiledImage, size: (u32, u32)) -> TiledImage {
    let (w, h) = (size.0.max(1), size.1.max(1));
    let Ok(mut out) = TiledImage::new(w, h, img.format()) else { return img.clone() };
    if img.allocated_tiles() == 0 {
        return out;
    }
    let (sw, sh) = img.size();
    let (fx, fy) = (sw as f32 / w as f32, sh as f32 / h as f32);
    let tiles = out.all_tiles();
    out.update_tiles(&tiles, |r, px| {
        for y in 0..r.h {
            let v = ((r.y + y) as f32 + 0.5) * fy - 0.5;
            for x in 0..r.w {
                let u = ((r.x + x) as f32 + 0.5) * fx - 0.5;
                if let Some(p) = px.get_mut((y * r.w + x) as usize) {
                    *p = bilinear(img, u, v);
                }
            }
        }
    });
    out
}

fn bilinear(img: &TiledImage, u: f32, v: f32) -> [f32; 4] {
    let (w, h) = img.size();
    let clamp = |t: f32, n: u32| t.clamp(0.0, (n - 1) as f32);
    let (u, v) = (clamp(u, w), clamp(v, h));
    let (x0, y0) = (u.floor() as u32, v.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (u - x0 as f32, v - y0 as f32);
    let (a, b, c, d) = (img.get(x0, y0), img.get(x1, y0), img.get(x0, y1), img.get(x1, y1));
    std::array::from_fn(|i| {
        let top = a[i] + (b[i] - a[i]) * tx;
        let bottom = c[i] + (d[i] - c[i]) * tx;
        top + (bottom - top) * ty
    })
}
