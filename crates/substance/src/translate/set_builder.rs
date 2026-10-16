//! [`SetBuilder`]: one [`TextureSet`] from a [`SetSource`]: channels, the flattened paint layer, baked maps.

use super::baked_maps::BakedMaps;
use super::cache_channels::{CacheChannels, CacheImage};
use super::channel_table::{channel_for_bit, channel_spec, painter_channel_name, painter_default};
use super::report::Reporter;
use super::texture_sets::SetSource;
use materialcraft::paint::{BlendMode, Channel, Fidelity, Fill, Layer, LayerKind, PaintContent, TextureSet};
use std::collections::BTreeSet;

/// Resolution for sets with neither cache entries nor a recorded resolution (Painter's default).
const FALLBACK_RESOLUTION: u32 = 2048;
/// Largest `resolutionLog2` accepted (Painter's maximum is 8K = 2^13).
const MAX_LOG2: i32 = 14;
pub const IMPORTED_LAYER_ID: &str = "imported";
pub const IMPORTED_LAYER_NAME: &str = "Imported (flattened)";
pub const DEFAULTS_LAYER_ID: &str = "painter-defaults";
pub const DEFAULTS_LAYER_NAME: &str = "Painter defaults";

pub struct SetBuilder<'a> {
    pub cache: &'a CacheChannels<'a>,
    pub baked: &'a BakedMaps<'a>,
}

impl SetBuilder<'_> {
    /// Builds the set; parts that fail are reported and left out. Datasets turned into pixels are added to `translated`.
    pub fn build(&self, source: &SetSource, report: &mut Reporter, translated: &mut BTreeSet<String>) -> TextureSet {
        let name = &source.record.name;
        let images = self.cache_images(source, report, translated);
        let resolution = resolve_resolution(source, &images, report);
        let mut set = TextureSet::new(source.id.clone(), name.clone(), resolution);
        set.material_slot = source.slot;
        declare_channels(&mut set, source, &images, report);
        let fully_covered = !images.is_empty() && images.iter().all(|(_, i)| i.fully_covered);
        if !fully_covered {
            set.layers.push(defaults_layer(&set));
            report.add(
                format!("texture set \"{name}\" / {DEFAULTS_LAYER_NAME}"),
                Fidelity::Approximated,
                "uncovered texels show Painter's channel defaults (bottom fill layer)",
            );
        }
        if let Some(layer) = imported_layer(&set, images, report) {
            set.layers.push(layer);
        }
        self.add_baked(source, &mut set, report, translated);
        report.layers(name, source.record.layer_count, source.record.action_count);
        set
    }

    /// Converted cache images with their channel; unknown bits and failures are reported.
    fn cache_images(
        &self,
        source: &SetSource,
        report: &mut Reporter,
        translated: &mut BTreeSet<String>,
    ) -> Vec<(Channel, CacheImage)> {
        let name = &source.record.name;
        let Some(folder) = &source.cache_set else {
            report.set_problem(
                name,
                Fidelity::Approximated,
                "no texture cache (Painter never rendered this set); channels are empty",
            );
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in self.cache.entries_of(folder) {
            let label = painter_channel_name(entry.channel_bit);
            let Some(channel) = channel_for_bit(entry.channel_bit) else {
                report.channel_failed(name, &label, "cache entry for an unknown channel; kept raw");
                continue;
            };
            match self.cache.read(entry) {
                Ok(image) => {
                    translated.insert(entry.path.clone());
                    out.push((channel, image));
                }
                Err(e) => report.channel_failed(name, &label, format!("unreadable cache entry: {e}")),
            }
        }
        out
    }

    fn add_baked(
        &self,
        source: &SetSource,
        set: &mut TextureSet,
        report: &mut Reporter,
        translated: &mut BTreeSet<String>,
    ) {
        let name = &source.record.name;
        for baked in self.baked.sources(name, &source.record.baked) {
            match self.baked.read(&baked) {
                Ok(image) => {
                    report.baked_map(name, &baked.name, image.missing_bytes);
                    translated.insert(image.dataset);
                    set.baked.push(image.map);
                }
                Err(e) => report.baked_failed(name, &baked.name, e.to_string()),
            }
        }
    }
}

/// Declares the document's channels (in its order and formats), then any channel only the cache has.
fn declare_channels(set: &mut TextureSet, source: &SetSource, images: &[(Channel, CacheImage)], report: &mut Reporter) {
    for c in &source.record.channels {
        match channel_for_bit(c.type_bit) {
            Some(channel) if !set.has_channel(channel) => set.channels.push(channel_spec(channel, Some(c.format))),
            Some(_) => {}
            None => report.channel_failed(
                &set.name,
                &painter_channel_name(c.type_bit),
                "no matching channel; not imported",
            ),
        }
    }
    for (channel, _) in images {
        if !set.has_channel(*channel) {
            set.channels.push(channel_spec(*channel, None));
        }
    }
}

/// The paint layer holding the flattened channels in the set's layer formats; `None` without usable images.
fn imported_layer(set: &TextureSet, images: Vec<(Channel, CacheImage)>, report: &mut Reporter) -> Option<Layer> {
    let mut layer = Layer::new(
        IMPORTED_LAYER_ID,
        IMPORTED_LAYER_NAME,
        LayerKind::Paint(PaintContent::default()),
    );
    let mut content = PaintContent::default();
    for (channel, image) in images {
        let label = painter_channel_name(image.bit);
        if image.image.size() != set.resolution {
            let note = format!(
                "cache image is {:?}, set is {:?}; not imported",
                image.image.size(),
                set.resolution
            );
            report.channel_failed(&set.name, &label, note);
            continue;
        }
        let pixels = match set.channel_spec(channel).map(|s| s.layer_format()) {
            Some(f) if f != image.image.format() => image.image.converted(f),
            _ => image.image,
        };
        content.set_image(channel, pixels);
        layer.set_blend(channel, BlendMode::Normal, 1.0);
        report.channel(&set.name, &label, image.missing_bytes);
    }
    layer.kind = LayerKind::Paint(content);
    (!layer.blend.is_empty()).then_some(layer)
}

/// A uniform fill with Painter's default value for every declared channel.
fn defaults_layer(set: &TextureSet) -> Layer {
    let values = set
        .channels
        .iter()
        .map(|s| (s.channel, painter_default(s.channel)))
        .collect();
    let mut layer = Layer::new(
        DEFAULTS_LAYER_ID,
        DEFAULTS_LAYER_NAME,
        LayerKind::Fill(Fill::uniform(values)),
    );
    for spec in &set.channels {
        layer.set_blend(spec.channel, BlendMode::Normal, 1.0);
    }
    layer
}

/// The cache images' size; else the document's `resolutionLog2`; else the fallback. Both sources are reported.
fn resolve_resolution(source: &SetSource, images: &[(Channel, CacheImage)], report: &mut Reporter) -> (u32, u32) {
    let name = &source.record.name;
    let recorded = source
        .record
        .resolution_log2
        .filter(|r| r.iter().all(|v| (0..=MAX_LOG2).contains(v)))
        .map(|[w, h]| (1u32 << w, 1u32 << h));
    let cached = images.first().map(|(_, i)| i.image.size());
    let show = |r: Option<(u32, u32)>| r.map_or("none".to_string(), |(w, h)| format!("{w}x{h}"));
    let note = format!("resolution: cache {}, document {}", show(cached), show(recorded));
    match cached.or(recorded) {
        Some(r) => {
            if cached.is_some() && recorded.is_some() && cached != recorded {
                report.set_problem(name, Fidelity::Approximated, format!("{note} differ; using the cache"));
            } else {
                report.set(name, note);
            }
            r
        }
        None => {
            report.set_problem(
                name,
                Fidelity::Approximated,
                format!("{note}; using {FALLBACK_RESOLUTION}"),
            );
            (FALLBACK_RESOLUTION, FALLBACK_RESOLUTION)
        }
    }
}
