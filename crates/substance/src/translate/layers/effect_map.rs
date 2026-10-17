//! [`ContentMapper`]: a Painter layer's content (its effect stack, bottom → top) as our base content plus
//! content effects, effect for effect.
//!
//! - The base: a fill layer's first fill when it is switched on and writes every channel at full strength
//!   (its values, or a paint image for a bitmap, as [`core_content`] builds them); a paint layer's first effect
//!   group (its brush work; strokes are not replayed yet, so the base has no pixels). Otherwise the base is an
//!   empty fill or paint content and the first effect stays an effect.
//! - Every other Painter effect becomes one of ours: uniform and bitmap fills become fill and paint effects,
//!   the channels a fill takes from a procedural, an anchor or an unknown source become a generator, reference
//!   or unknown effect next to it; an effect group becomes a paint effect followed by its own entries; filters,
//!   levels, colour selection, polygon fill and anchors keep their kind.
//!
//! Each effect gets a blend entry for every declared channel it writes, with the mode and opacity Painter
//! stores for it. Whether a channel is computable is decided by [`super::content::ContentEvaluator`].

use super::blend_table::Holder;
use super::blending::ResolvedBlend;
use super::content::{ChannelContent, ChannelValue, ContentEvaluator};
use super::effect_data::{self as data, AnchorIds};
use super::layer_content::core_content;
use super::strokes;
use crate::model::{EffectHeader, FillEffect, LayerKind as PainterKind, PainterEffect, PainterLayer, PainterSource};
use materialcraft::json::Value;
use materialcraft::paint::effect::{PolygonFill, Selection};
use materialcraft::paint::{Channel, ChannelBlend, ContentEffect, EffectKind, Fill, LayerKind, PaintContent};

/// A layer's base content and its effects, bottom → top.
pub struct MappedContent {
    pub base: LayerKind,
    pub effects: Vec<ContentEffect>,
}

pub struct ContentMapper<'e, 'a> {
    pub evaluator: &'e ContentEvaluator<'a>,
}

impl ContentMapper<'_, '_> {
    /// The base and effects of a layer that is not a folder.
    pub fn map(&self, l: &PainterLayer, anchors: &mut AnchorIds) -> MappedContent {
        let mut effects = Vec::new();
        let base = match (&l.kind, l.content.split_first()) {
            (PainterKind::Fill, Some((PainterEffect::Fill(first), rest))) if first.header.enabled => {
                match self.fill_base(first) {
                    Some((base, leftovers)) => {
                        effects.extend(leftovers);
                        self.effects(rest, anchors, &mut effects);
                        return MappedContent { base, effects };
                    }
                    None => LayerKind::Fill(Fill::uniform(Vec::new())),
                }
            }
            (PainterKind::Fill, _) => LayerKind::Fill(Fill::uniform(Vec::new())),
            (PainterKind::Paint, Some((PainterEffect::Group(group), rest))) if group.header.enabled => {
                self.brush_work(&group.effects, anchors, &mut effects);
                self.effects(rest, anchors, &mut effects);
                return MappedContent {
                    base: LayerKind::Paint(painted(strokes::group_record(&group.effects))),
                    effects,
                };
            }
            _ => LayerKind::Paint(PaintContent::default()),
        };
        self.effects(&l.content, anchors, &mut effects);
        MappedContent { base, effects }
    }

    /// Effects of a folder (Painter allows content on folders; rare).
    pub fn folder_effects(&self, l: &PainterLayer, anchors: &mut AnchorIds) -> Vec<ContentEffect> {
        let mut effects = Vec::new();
        self.effects(&l.content, anchors, &mut effects);
        effects
    }

    /// The fill as a base when every channel it computes is at full strength, with effects for the channels
    /// it takes from elsewhere (procedurals, anchors …).
    fn fill_base(&self, fill: &FillEffect) -> Option<(LayerKind, Vec<ContentEffect>)> {
        let values = self.values(fill);
        let computed: Vec<ChannelContent> = values
            .into_iter()
            .filter(|c| !matches!(c.value, ChannelValue::Nothing))
            .collect();
        let full = computed.iter().all(|c| match c.value {
            ChannelValue::Uniform { coverage, .. } | ChannelValue::Image { coverage, .. } => coverage >= 1.0,
            ChannelValue::Nothing => true,
        });
        if !full {
            return None;
        }
        let ev = self.evaluator;
        let base = core_content(&computed, &PainterKind::Fill, ev.channels, ev.resolution);
        let written: Vec<Channel> = computed.iter().map(|c| c.channel).collect();
        Some((base, self.fill_sources(fill, &written, true)))
    }

    fn effects(&self, effects: &[PainterEffect], anchors: &mut AnchorIds, out: &mut Vec<ContentEffect>) {
        for e in effects {
            self.effect(e, anchors, out);
        }
    }

    /// The entries of a paint group other than its brush work, which is the paint content itself.
    fn brush_work(&self, effects: &[PainterEffect], anchors: &mut AnchorIds, out: &mut Vec<ContentEffect>) {
        for e in effects.iter().filter(|e| !matches!(e, PainterEffect::Paint(_))) {
            self.effect(e, anchors, out);
        }
    }

    fn effect(&self, effect: &PainterEffect, anchors: &mut AnchorIds, out: &mut Vec<ContentEffect>) {
        let Some(header) = effect.header() else {
            if let PainterEffect::Other(raw) = effect {
                let mut e = ContentEffect::new(Vec::new(), EffectKind::Unknown(data::unknown(raw)));
                e.enabled = false;
                out.push(e);
            }
            return;
        };
        let kind = match effect {
            PainterEffect::Fill(fill) => return self.fill(fill, out),
            PainterEffect::Group(g) => {
                let content = painted(strokes::group_record(&g.effects));
                out.push(self.with_header(header, Holder::EffectGroup, EffectKind::Paint(content)));
                return self.brush_work(&g.effects, anchors, out);
            }
            PainterEffect::Paint(p) => EffectKind::Paint(painted(strokes::record([p]))),
            PainterEffect::Filter(f) => EffectKind::Filter(data::procedural(f.procedural.as_ref())),
            PainterEffect::Levels(l) => EffectKind::Levels(data::levels(l.levels.as_ref())),
            PainterEffect::ColorSelection(c) => EffectKind::ColorSelection(data::color_selection(c)),
            PainterEffect::PolygonFill(_) => EffectKind::PolygonFill(PolygonFill {
                selection: Selection::default(),
                values: Fill::uniform(Vec::new()),
            }),
            PainterEffect::Anchor(h) => EffectKind::Anchor(anchors.anchor(h)),
            PainterEffect::Other(_) => return,
        };
        out.push(self.with_header(header, Holder::Layer, kind));
    }

    /// A fill effect: its uniform and bitmap channels as one fill or paint effect, the other channels as
    /// generator, reference or unknown effects.
    fn fill(&self, fill: &FillEffect, out: &mut Vec<ContentEffect>) {
        let computed: Vec<ChannelContent> = self
            .values(fill)
            .into_iter()
            .filter(|c| !matches!(c.value, ChannelValue::Nothing))
            .collect();
        let ev = self.evaluator;
        let blend: Vec<ChannelBlend> = computed
            .iter()
            .map(|c| self.entry(&fill.header, c.channel, Holder::Layer))
            .collect();
        let kind = match core_content(&computed, &PainterKind::Fill, ev.channels, ev.resolution) {
            LayerKind::Paint(p) => EffectKind::Paint(p),
            LayerKind::Fill(f) => EffectKind::Fill(f),
            LayerKind::Group(_) => EffectKind::Fill(Fill::uniform(Vec::new())),
        };
        if !computed.is_empty() || fill.sources.is_empty() {
            out.push(ContentEffect {
                enabled: fill.header.enabled,
                label: fill.header.label.clone(),
                blend,
                kind,
            });
        }
        let written: Vec<Channel> = computed.iter().map(|c| c.channel).collect();
        out.extend(self.fill_sources(fill, &written, fill.header.enabled));
    }

    /// One effect per source the fill takes the channels not in `computed` from.
    fn fill_sources(&self, fill: &FillEffect, computed: &[Channel], enabled: bool) -> Vec<ContentEffect> {
        let ev = self.evaluator;
        let mut groups: Vec<(usize, Vec<Channel>)> = Vec::new();
        for channel in ev.channels.all().filter(|c| !computed.contains(c)) {
            if !fill.header.channels.is_empty() && !ev.channels.mask_has(fill.header.channels, channel) {
                continue;
            }
            let painter = ev.channels.painter(channel);
            let found = fill.sources.iter().position(|s| {
                let c = s.channels();
                c.contains(painter) || (c.is_all() && matches!(s, PainterSource::Procedural(_)))
            });
            let Some(index) = found else { continue };
            if !ResolvedBlend::of(&fill.header.blending, painter, Holder::Layer, ev.table).contributes() {
                continue;
            }
            match groups.iter_mut().find(|(i, _)| *i == index) {
                Some((_, channels)) => channels.push(channel),
                None => groups.push((index, vec![channel])),
            }
        }
        let mut out = Vec::new();
        for (index, channels) in groups {
            let Some(source) = fill.sources.get(index) else {
                continue;
            };
            let kind = match source {
                PainterSource::Procedural(p) => EffectKind::Generator(data::procedural(p.procedural.as_ref())),
                PainterSource::Reference(r) => EffectKind::Reference(data::reference(r)),
                PainterSource::Other(raw) => EffectKind::Unknown(data::unknown(raw)),
                PainterSource::Bitmap(b) => {
                    let url = Value::object().with("url", b.url.as_str());
                    EffectKind::Unknown(data::unknown_named("substance:bitmap_fill", url))
                }
                PainterSource::Uniform(_) => {
                    EffectKind::Unknown(data::unknown_named("substance:uniform_fill", Value::object()))
                }
            };
            let blend = channels
                .iter()
                .map(|c| self.entry(&fill.header, *c, Holder::Layer))
                .collect();
            out.push(ContentEffect {
                enabled,
                label: fill.header.label.clone(),
                blend,
                kind,
            });
        }
        out
    }

    /// What the fill gives each channel, evaluated as if it were switched on.
    fn values(&self, fill: &FillEffect) -> Vec<ChannelContent> {
        let mut on = fill.clone();
        on.header.enabled = true;
        self.evaluator.evaluate(&[PainterEffect::Fill(on)])
    }

    /// An effect with an entry for every declared channel its header lists.
    fn with_header(
        &self,
        header: &EffectHeader,
        holder: Holder,
        kind: EffectKind<materialcraft::paint::effect::Content>,
    ) -> ContentEffect {
        let ev = self.evaluator;
        let listed = ev
            .channels
            .all()
            .filter(|c| header.channels.is_empty() || ev.channels.mask_has(header.channels, *c));
        let blend = listed.map(|c| self.entry(header, c, holder)).collect();
        ContentEffect {
            enabled: header.enabled,
            label: header.label.clone(),
            blend,
            kind,
        }
    }

    fn entry(&self, header: &EffectHeader, channel: Channel, holder: Holder) -> ChannelBlend {
        let ev = self.evaluator;
        let b = ResolvedBlend::of(&header.blending, ev.channels.painter(channel), holder, ev.table);
        ChannelBlend {
            channel,
            mode: b.mode(),
            opacity: b.opacity,
        }
    }
}

/// Paint content without pixels (strokes are not replayed yet), with the stroke record of its brush work.
fn painted(strokes: Option<materialcraft::paint::stroke::StrokeData>) -> PaintContent {
    PaintContent {
        images: Vec::new(),
        strokes,
    }
}
