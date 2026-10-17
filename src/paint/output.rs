//! [`MaterialMaps`]: the final PBR maps of a texture set, for the viewer and for export.
//!
//! Each map is the composited channel (see [`crate::paint::Compositor`]) with alpha 1; channels the
//! set does not declare are filled with their default. Two maps combine several inputs:
//! - **normal** = `RNM(RNM(baked, height_normal), normal_channel)` as in Substance Painter
//!   ([`super::normal_combine`]): `baked` is the baked mesh normal map (flat if absent) and
//!   `height_normal` the normal derived from the composited height with
//!   [`super::height_normal::normal_from_height`] and the document's filter (edges clamped, flat if
//!   the set has no height channel). Flat inputs drop out.
//! - **ambient_occlusion** = baked AO × AO channel.
//!
//! Baked maps of another resolution are resampled bilinearly. Values stay in each channel's stored
//! colour space (base colour and emissive are usually sRGB-encoded).

use super::channel::Channel;
use super::doc::Document;
use super::height_normal::{self, HeightFilter};
use super::normal;
use super::normal_combine;
use super::par::for_each_mut;
use super::stack::{CompositeWarning, Compositor, FillContext, NO_FILL_CONTEXT};
use super::texture_set::{BakedKind, TextureSet};
use crate::tex::Tex;

/// Settings for [`MaterialMaps::build`]. [`OutputOptions::for_document`] gives a document's own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputOptions {
    /// Multiplier on Painter's height → normal scale (1 = Painter, see
    /// [`super::height_normal::UV_SLOPE`]); 0 ignores the height channel for the normal.
    pub height_normal_strength: f32,
    /// Gradient estimate of the height → normal conversion ([`Document::height_filter`]).
    pub height_filter: HeightFilter,
}

impl OutputOptions {
    /// The settings `doc` asks for, at strength 1.
    pub fn for_document(doc: &Document) -> OutputOptions {
        OutputOptions { height_filter: doc.height_filter, ..OutputOptions::default() }
    }
}

impl Default for OutputOptions {
    fn default() -> OutputOptions {
        OutputOptions { height_normal_strength: 1.0, height_filter: HeightFilter::default() }
    }
}

/// Final maps at the texture set resolution. Gray maps hold the value in R, G and B.
#[derive(Debug, Clone)]
pub struct MaterialMaps {
    pub base_color: Tex,
    pub roughness: Tex,
    pub metallic: Tex,
    /// Tangent-space, OpenGL +Y, encoded.
    pub normal: Tex,
    pub height: Tex,
    pub ambient_occlusion: Tex,
    pub emissive: Tex,
    pub opacity: Tex,
    /// Compositing warnings of all channels, without repeats.
    pub warnings: Vec<CompositeWarning>,
}

impl MaterialMaps {
    pub fn build(set: &TextureSet, options: &OutputOptions) -> MaterialMaps {
        MaterialMaps::build_with_fills(set, options, &NO_FILL_CONTEXT)
    }

    /// Like [`MaterialMaps::build`], evaluating fills through `fills` (material sources, the mesh's surface).
    pub fn build_with_fills(set: &TextureSet, options: &OutputOptions, fills: &FillContext) -> MaterialMaps {
        let (w, h) = (set.resolution.0 as usize, set.resolution.1 as usize);
        let compositor = Compositor::new(set).with_fills(fills);
        let mut warnings = Vec::new();
        let mut channel = |c: Channel| {
            if !set.has_channel(c) {
                return Tex::solid(w, h, c.default_value());
            }
            let (mut tex, warn) = compositor.composite_tex(c);
            for wn in warn {
                if !warnings.contains(&wn) {
                    warnings.push(wn);
                }
            }
            par_pixels(&mut tex.px, |_, p| p[3] = 1.0);
            tex
        };
        let base_color = channel(Channel::BaseColor);
        let roughness = channel(Channel::Roughness);
        let metallic = channel(Channel::Metallic);
        let height = channel(Channel::Height);
        let mut normal = channel(Channel::Normal);
        let mut ambient_occlusion = channel(Channel::AmbientOcclusion);
        let emissive = channel(Channel::Emissive);
        let opacity = channel(Channel::Opacity);

        let height_normal = (set.has_channel(Channel::Height) && options.height_normal_strength != 0.0 && w * h > 0)
            .then(|| height_normal::normal_from_height(&height, options.height_filter, options.height_normal_strength));
        let baked_normal = set.baked(&BakedKind::Normal).map(|i| resample(&i.to_tex(), w, h));
        if height_normal.is_some() || baked_normal.is_some() {
            par_pixels(&mut normal.px, |i, p| {
                let at = |t: &Option<Tex>| t.as_ref().and_then(|t| t.px.get(i)).map_or(normal::FLAT, |&c| normal::decode(c));
                let n = normal_combine::combine(at(&baked_normal), at(&height_normal), normal::decode(*p));
                *p = normal::encode(n);
            });
        } else {
            par_pixels(&mut normal.px, |_, p| *p = normal::encode(normal::decode(*p)));
        }
        if let Some(ao) = set.baked(&BakedKind::AmbientOcclusion).map(|i| resample(&i.to_tex(), w, h)) {
            par_pixels(&mut ambient_occlusion.px, |i, p| {
                let k = ao.px.get(i).map_or(1.0, |a| a[0]);
                *p = [p[0] * k, p[1] * k, p[2] * k, 1.0];
            });
        }
        MaterialMaps { base_color, roughness, metallic, normal, height, ambient_occlusion, emissive, opacity, warnings }
    }

    /// The map for a standard channel; `None` for user channels.
    pub fn map(&self, channel: Channel) -> Option<&Tex> {
        Some(match channel {
            Channel::BaseColor => &self.base_color,
            Channel::Roughness => &self.roughness,
            Channel::Metallic => &self.metallic,
            Channel::Normal => &self.normal,
            Channel::Height => &self.height,
            Channel::AmbientOcclusion => &self.ambient_occlusion,
            Channel::Emissive => &self.emissive,
            Channel::Opacity => &self.opacity,
            Channel::User(_) => return None,
        })
    }
}

/// Runs `f(pixel index, pixel)` over all pixels in parallel.
fn par_pixels<F: Fn(usize, &mut [f32; 4]) + Sync>(px: &mut [[f32; 4]], f: F) {
    const CHUNK: usize = 16384;
    let mut chunks: Vec<&mut [[f32; 4]]> = px.chunks_mut(CHUNK).collect();
    for_each_mut(&mut chunks, |ci, chunk| {
        for (i, p) in chunk.iter_mut().enumerate() {
            f(ci * CHUNK + i, p);
        }
    });
}

/// `tex` at `w × h`, bilinear (unchanged if the size already matches).
fn resample(tex: &Tex, w: usize, h: usize) -> Tex {
    if (tex.w, tex.h) == (w, h) || tex.px.is_empty() {
        return tex.clone();
    }
    let mut out = Tex::solid(w, h, [0.0; 4]);
    par_pixels(&mut out.px, |i, p| *p = tex.sample(((i % w) as f32 + 0.5) / w as f32, ((i / w) as f32 + 0.5) / h as f32));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{BakedMap, BlendMode, Fill, Layer, LayerKind, PixelFormat, TiledImage};

    fn flat_set() -> TextureSet {
        let mut s = TextureSet::new("s", "S", (8, 8)).with_channel(Channel::Normal).with_channel(Channel::Height).with_channel(Channel::AmbientOcclusion);
        let fill = Fill::uniform(vec![(Channel::Height, [0.3; 4])]);
        s.layers.push(Layer::new("h", "H", LayerKind::Fill(fill)).with_blend(Channel::Height, BlendMode::Normal, 1.0));
        s
    }

    fn baked(kind: BakedKind, size: u32, v: [f32; 4]) -> BakedMap {
        let mut image = TiledImage::new(size, size, PixelFormat::from_id("rgb16").unwrap()).unwrap();
        image.fill(v);
        BakedMap { kind, image }
    }

    #[test]
    fn flat_inputs_give_flat_normal() {
        let mut s = flat_set();
        s.baked.push(baked(BakedKind::Normal, 4, [0.5, 0.5, 1.0, 1.0]));
        let maps = MaterialMaps::build(&s, &OutputOptions::default());
        assert!(maps.normal.px.iter().all(|p| (p[0] - 0.5).abs() < 1e-4 && (p[1] - 0.5).abs() < 1e-4 && (p[2] - 1.0).abs() < 1e-4));
        assert_eq!((maps.normal.w, maps.normal.h), (8, 8));
        assert!(maps.height.px.iter().all(|p| (p[0] - 0.3).abs() < 1e-3 && p[3] == 1.0));
        assert_eq!(maps.base_color.px[0], Channel::BaseColor.default_value());
        assert!(maps.warnings.is_empty());
    }

    #[test]
    fn baked_normal_passes_through_and_ao_multiplies() {
        let mut s = flat_set();
        let tilted = normal::encode(normal::normalize([0.3, 0.0, 1.0]));
        s.baked.push(baked(BakedKind::Normal, 8, tilted));
        s.baked.push(baked(BakedKind::AmbientOcclusion, 16, [0.5, 0.5, 0.5, 1.0]));
        let fill = Fill::uniform(vec![(Channel::AmbientOcclusion, [0.8; 4])]);
        s.layers.push(Layer::new("ao", "AO", LayerKind::Fill(fill)).with_blend(Channel::AmbientOcclusion, BlendMode::Normal, 1.0));
        let maps = MaterialMaps::build(&s, &OutputOptions::default());
        assert!((maps.normal.px[9][0] - tilted[0]).abs() < 1e-3);
        assert!((maps.ambient_occlusion.px[9][0] - 0.4).abs() < 1e-3);
        assert!(maps.map(Channel::User(0)).is_none());
    }

    #[test]
    fn options_follow_the_document() {
        let doc = Document { height_filter: HeightFilter::Sobel, ..Document::default() };
        let o = OutputOptions::for_document(&doc);
        assert_eq!(o.height_filter, HeightFilter::Sobel);
        assert_eq!(o.height_normal_strength, 1.0);
    }
}
