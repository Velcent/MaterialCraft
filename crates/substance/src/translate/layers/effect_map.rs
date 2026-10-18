//! [`ContentMapper`]: a Painter layer's content (its effect stack, bottom → top) as our base content plus
//! content effects, effect for effect.
//!
//! - The base: a fill layer's first fill when it is switched on and writes every channel at full strength
//!   (its values, or a paint image for a bitmap, as [`core_content`] builds them); a paint layer's first effect
//!   group (its brush work: the replayed strokes' pixels, see [`super::replay`], or none where they cannot be
//!   replayed). Otherwise the base is an
//!   empty fill or paint content and the first effect stays an effect.
//! - Every other Painter effect becomes one of ours: uniform and bitmap fills become fill and paint effects,
//!   the channels a fill takes from a procedural, an anchor or an unknown source become a generator, reference
//!   or unknown effect next to it (a Substance material the user replaced with one of ours becomes a material
//!   fill, a texture the plan replaces a texture fill, either the base of a fill layer that is nothing else); an
//!   effect group becomes a paint effect (replayed
//!   like the base) followed by its own entries; filters,
//!   levels, colour selection, polygon fill and anchors keep their kind.
//!
//! Each effect gets a blend entry for every declared channel it writes, with the mode and opacity Painter
//! stores for it. Whether a channel is computable is decided by [`super::content::ContentEvaluator`].

use super::blend_table::Holder;
use super::blending::ResolvedBlend;
use super::content::brush_work;
use super::content::{ChannelContent, ChannelValue, ContentEvaluator, filter_replacement};
use super::decal::{self, geometry::SetGeometry};
use super::effect_data::{self as data, AnchorIds};
use super::layer_content::core_content;
use super::strokes;
use crate::model::{
    EffectHeader, FillEffect, LayerKind as PainterKind, PainterEffect, PainterLayer, PainterSource, PolygonFillEffect,
    ProceduralSource,
};
use crate::translate::replacements::{channel_textures, materials};
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
    /// The set's mesh, for polygon-fill picks.
    pub geometry: Option<&'a SetGeometry<'a>>,
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
                    base: LayerKind::Paint(self.painted(&group.effects)),
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
    /// A fill that is only a picked material at full strength becomes a material fill layer.
    fn fill_base(&self, fill: &FillEffect) -> Option<(LayerKind, Vec<ContentEffect>)> {
        let computed = self.computed(fill);
        let full = computed.iter().all(|c| match c.value {
            ChannelValue::Uniform { coverage, .. } | ChannelValue::Image { coverage, .. } => coverage >= 1.0,
            ChannelValue::Composited | ChannelValue::Nothing => true,
        });
        if !full {
            return None;
        }
        let ev = self.evaluator;
        let written: Vec<Channel> = computed.iter().map(|c| c.channel).collect();
        let mut leftovers = self.fill_sources(fill, &written, true);
        if let ([only], true) = (leftovers.as_slice(), computed.is_empty())
            && let EffectKind::Fill(material) = &only.kind
            && only.blend.iter().all(|b| b.opacity >= 1.0)
        {
            let base = LayerKind::Fill(material.clone());
            leftovers.clear();
            return Some((base, leftovers));
        }
        let base = core_content(&computed, &PainterKind::Fill, ev.channels, ev.resolution);
        Some((base, leftovers))
    }

    /// What the fill computes itself (constants, bitmaps, …), leaving out the channels a picked material
    /// writes: those become a material fill ([`Self::fill_sources`]).
    fn computed(&self, fill: &FillEffect) -> Vec<ChannelContent> {
        self.values(fill)
            .into_iter()
            .filter(|c| !matches!(c.value, ChannelValue::Nothing))
            .filter(|c| !matches!(self.source_of(fill, c.channel), Some(PainterSource::Procedural(p)) if self.replaced_source(p)))
            .collect()
    }

    /// True for a procedural source that becomes a fill of its own: a picked material or a replaced texture.
    fn replaced_source(&self, p: &ProceduralSource) -> bool {
        let plan = self.evaluator.plan;
        materials::chosen(p, plan).is_some() || channel_textures::chosen(p, plan).is_some()
    }

    /// The source the fill takes `channel` from.
    fn source_of<'f>(&self, fill: &'f FillEffect, channel: Channel) -> Option<&'f PainterSource> {
        self.source_index(fill, channel).and_then(|i| fill.sources.get(i))
    }

    fn source_index(&self, fill: &FillEffect, channel: Channel) -> Option<usize> {
        let painter = self.evaluator.channels.painter(channel);
        fill.sources.iter().position(|s| {
            let c = s.channels();
            c.contains(painter) || (c.is_all() && matches!(s, PainterSource::Procedural(_)))
        })
    }

    /// The paint pixels of brush work (an effect group's paint effects, or one bare paint effect): replayed when
    /// the replay can, empty otherwise; with the stroke record either way.
    fn painted(&self, effects: &[PainterEffect]) -> PaintContent {
        let strokes = strokes::group_record(effects);
        let paints = brush_work(effects);
        let images = match self
            .evaluator
            .replay
            .filter(|_| paints.iter().any(|p| p.stroke_count() > 0))
        {
            Some(r) => r
                .content(&paints, self.evaluator.channels)
                .map(|r| r.pixels)
                .unwrap_or_default(),
            None => Vec::new(),
        };
        PaintContent {
            images,
            strokes,
            see_through: false,
        }
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
        let holder = match effect {
            PainterEffect::Levels(_) | PainterEffect::Filter(_) => Holder::Filter,
            _ => Holder::Layer,
        };
        let kind = match effect {
            PainterEffect::Fill(fill) => return self.fill(fill, out),
            PainterEffect::Group(g) => {
                let content = self.painted(&g.effects);
                out.push(self.with_header(header, Holder::EffectGroup, EffectKind::Paint(content)));
                return self.brush_work(&g.effects, anchors, out);
            }
            PainterEffect::Paint(_) => EffectKind::Paint(self.painted(std::slice::from_ref(effect))),
            PainterEffect::Filter(f) => match filter_replacement(f, self.evaluator.plan) {
                Some((_, ours)) => EffectKind::Filter(ours),
                None => EffectKind::Filter(data::procedural(f.procedural.as_ref())),
            },
            PainterEffect::Levels(l) => EffectKind::Levels(data::levels(l.levels.as_ref())),
            PainterEffect::ColorSelection(c) => EffectKind::ColorSelection(data::color_selection(c)),
            PainterEffect::PolygonFill(p) => return self.polygon_fill(header, p, out),
            PainterEffect::Anchor(h) => EffectKind::Anchor(anchors.anchor(h)),
            PainterEffect::Other(_) => return,
        };
        out.push(self.with_header(header, holder, kind));
    }

    /// A polygon fill in layer content (rare): one effect per run of picks, writing the picks' colour into base
    /// colour (what Painter writes there is not known; the content verdict calls it not computable).
    fn polygon_fill(&self, header: &EffectHeader, p: &PolygonFillEffect, out: &mut Vec<ContentEffect>) {
        let decal = decal::runs(self.geometry, p);
        let runs = match decal.runs.is_empty() {
            true => vec![decal::DecalRun {
                color: [1.0; 3],
                selection: Selection::default(),
            }],
            false => decal.runs,
        };
        for run in runs {
            let [r, g, b] = run.color;
            let values = Fill::uniform(vec![(Channel::BaseColor, [r, g, b, 1.0])]);
            let kind = EffectKind::PolygonFill(PolygonFill {
                selection: run.selection,
                values,
            });
            out.push(self.with_header(header, Holder::Layer, kind));
        }
    }

    /// A fill effect: its uniform and bitmap channels as one fill or paint effect, the other channels as
    /// generator, reference or unknown effects.
    fn fill(&self, fill: &FillEffect, out: &mut Vec<ContentEffect>) {
        let computed = self.computed(fill);
        let ev = self.evaluator;
        let blend: Vec<ChannelBlend> = computed
            .iter()
            .map(|c| self.entry(&fill.header, c.channel, Holder::Layer))
            .collect();
        let kind = match core_content(&computed, &PainterKind::Fill, ev.channels, ev.resolution) {
            LayerKind::Paint(p) => EffectKind::Paint(p),
            LayerKind::Fill(f) => EffectKind::Fill(f),
            LayerKind::Group(_) | LayerKind::Instance(_) => EffectKind::Fill(Fill::uniform(Vec::new())),
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
            let Some(index) = self.source_index(fill, channel) else {
                continue;
            };
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
                PainterSource::Procedural(p) => match materials::chosen(p, ev.plan) {
                    Some(reference) => EffectKind::Fill(materials::material_fill(
                        p,
                        reference,
                        ev.projections.map(fill).projection,
                    )),
                    None => match channel_textures::chosen(p, ev.plan) {
                        Some(t) => EffectKind::Fill(t.fill(&channels, ev.projections.map(fill).projection)),
                        None => EffectKind::Generator(data::procedural(p.procedural.as_ref())),
                    },
                },
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
