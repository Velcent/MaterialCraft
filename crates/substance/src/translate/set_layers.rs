//! [`SetLayers`]: the final layer stack of one texture set: the imported Painter layers (when the document
//! model has the set), the flattened cache layer and Painter's defaults, arranged by
//! [`StackStrategy`]. Without a model the set keeps the flattened import of M3.

use super::channel_table::painter_default;
use super::layers::cache_agreement::CacheAgreement;
use super::layers::report::{report_cache_disagreement, report_hidden_masks, report_stack};
use super::layers::strokes::{StrokeCount, report_strokes};
use super::layers::{LayerImport, PAINTER_RESULT_NAME, SetChannels, StackParts, StackStrategy};
use super::report::Reporter;
use super::texture_sets::SetSource;
use crate::model::{PainterDocument, PainterTextureSet};
use materialcraft::paint::{BlendMode, Fidelity, Fill, Layer, LayerKind, TextureSet};

pub const PAINTER_RESULT_ID: &str = "painter-result";
pub const DEFAULTS_LAYER_ID: &str = "painter-defaults";
pub const DEFAULTS_LAYER_NAME: &str = "Painter defaults";

/// The flattened cache layer of a set and how much it covers.
pub struct Flattened {
    pub layer: Option<Layer>,
    /// No texel of any cached channel has coverage below 1 (and there is at least one channel).
    pub fully_covered: bool,
    /// Fully covered and every declared channel has a cache image.
    pub complete: bool,
}

pub struct SetLayers<'a> {
    pub import: &'a LayerImport<'a>,
    /// Painter's model of `document.bin`, if it could be built.
    pub document: Option<&'a PainterDocument>,
}

impl SetLayers<'_> {
    /// Fills `set.layers` (bottom → top) and reports the layer stack.
    pub fn build(&self, set: &mut TextureSet, source: &SetSource, flattened: Flattened, report: &mut Reporter) {
        let name = source.record.name.clone();
        match self.painter_set(source) {
            Some(painter) => {
                let channels = SetChannels::new(&set.channels);
                let stack = self.import.import(painter, &channels, set.resolution);
                let defaults = defaults_layer(set);
                let agreement = match (&flattened.layer, stack.verdict.all_exact()) {
                    (Some(result), true) => CacheAgreement::measure(set, &defaults, &stack.layers, result),
                    _ => CacheAgreement::default(),
                };
                let strategy = StackStrategy::choose(&stack.verdict, flattened.layer.is_some(), &agreement);
                report_stack(report, &name, &stack, strategy, &channels);
                report_strokes(report, &name, &StrokeCount::of_set(painter), &stack.layers);
                report_cache_disagreement(report, &name, &agreement, &channels);
                if !strategy.shows_painter_result() {
                    report_hidden_masks(report, &name, &stack.verdict.mask_failures);
                }
                let parts = StackParts {
                    hide_unless_covered: stack.verdict.mask_failures.iter().map(|f| f.id.clone()).collect(),
                    imported: stack.layers,
                    painter_result: flattened.layer,
                    defaults,
                    result_complete: flattened.complete,
                };
                set.layers = strategy.assemble(parts, &channels);
            }
            None => {
                if !flattened.fully_covered {
                    set.layers.push(defaults_layer(set));
                }
                set.layers.extend(flattened.layer);
                report.layers(&name, source.record.layer_count, source.record.action_count);
            }
        }
        if set.layers.iter().any(|l| l.id == DEFAULTS_LAYER_ID) {
            report.add(
                format!("texture set \"{name}\" / {DEFAULTS_LAYER_NAME}"),
                Fidelity::Approximated,
                "uncovered texels show Painter's channel defaults (uniform fill layer)",
            );
        }
    }

    /// The model's set for a document-backed source, by material uid.
    fn painter_set(&self, source: &SetSource) -> Option<&PainterTextureSet> {
        let uid = source.record.scene_uid.filter(|_| source.in_document)?;
        self.document?.texture_set(uid)
    }
}

/// The flattened cache layer's name and id.
pub fn painter_result_layer(kind: LayerKind) -> Layer {
    Layer::new(PAINTER_RESULT_ID, PAINTER_RESULT_NAME, kind)
}

/// A uniform fill with Painter's default value for every declared channel.
pub fn defaults_layer(set: &TextureSet) -> Layer {
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
