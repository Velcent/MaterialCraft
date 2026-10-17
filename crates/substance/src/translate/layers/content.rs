//! [`ContentEvaluator`]: what a layer's content (its effect stack, bottom → top) gives each declared channel:
//! a constant from uniform fills, an image from a bitmap fill, or nothing computable (procedurals,
//! strokes, filters, references, …) with the reason. Anchors only mark a place and change nothing.

use super::blend_table::{BlendTable, Holder};
use super::blending::ResolvedBlend;
use super::channels::SetChannels;
use super::computability::Computability;
use super::images::{ImageSource, channel_pixels};
use super::projection_map::map_projection;
use crate::model::{EffectHeader, FillEffect, PainterEffect, PainterSource};
use materialcraft::paint::{BlendMode, Channel, TiledImage};
use materialcraft::projection::Projection;

/// The value one channel gets.
pub enum ChannelValue {
    /// A constant in the channel's stored encoding, with the content's coverage (0..1).
    Uniform { value: [f32; 4], coverage: f32 },
    /// A bitmap in the channel's layer format and encoding (alpha = coverage, any size), laid on by
    /// `projection`, and an extra coverage factor.
    Image {
        image: TiledImage,
        coverage: f32,
        projection: Projection,
    },
    /// Nothing that can be shown.
    Nothing,
}

/// One channel the content writes.
pub struct ChannelContent {
    pub channel: Channel,
    pub value: ChannelValue,
    pub status: Computability,
}

/// Working state for one channel while walking the effects. Uniform values stay in Painter's space
/// (linear) until the end.
struct State {
    value: Acc,
    status: Computability,
    written: bool,
}

enum Acc {
    Empty,
    Uniform([f32; 4]),
    Image(TiledImage, f32, Projection),
    Opaque,
}

pub struct ContentEvaluator<'a> {
    pub channels: &'a SetChannels,
    pub table: &'a BlendTable,
    pub images: &'a dyn ImageSource,
    pub resolution: (u32, u32),
}

impl ContentEvaluator<'_> {
    /// Every declared channel the effects write, in declaration order.
    pub fn evaluate(&self, effects: &[PainterEffect]) -> Vec<ChannelContent> {
        let mut out = Vec::new();
        for channel in self.channels.all() {
            let mut state = State {
                value: Acc::Empty,
                status: Computability::Exact,
                written: false,
            };
            self.walk(effects, channel, &mut state);
            if state.written {
                out.push(self.finish(channel, state));
            }
        }
        out
    }

    fn finish(&self, channel: Channel, state: State) -> ChannelContent {
        let value = match state.value {
            Acc::Uniform(v) => ChannelValue::Uniform {
                value: self.channels.stored_value(channel, [v[0], v[1], v[2]]),
                coverage: v[3],
            },
            Acc::Image(image, coverage, projection) => ChannelValue::Image {
                image,
                coverage,
                projection,
            },
            Acc::Empty | Acc::Opaque => ChannelValue::Nothing,
        };
        ChannelContent {
            channel,
            value,
            status: state.status,
        }
    }

    fn walk(&self, effects: &[PainterEffect], channel: Channel, state: &mut State) {
        let painter = self.channels.painter(channel);
        for effect in effects.iter().filter(|e| e.is_enabled()) {
            match effect {
                PainterEffect::Fill(fill) => self.fill(fill, channel, state),
                PainterEffect::Group(group) => {
                    let blend = ResolvedBlend::of(&group.header.blending, painter, Holder::EffectGroup, self.table);
                    if !blend.contributes() {
                        continue;
                    }
                    if blend.mode() == BlendMode::Passthrough {
                        self.walk(&group.effects, channel, state);
                        continue;
                    }
                    let mut inner = State {
                        value: Acc::Empty,
                        status: Computability::Exact,
                        written: false,
                    };
                    self.walk(&group.effects, channel, &mut inner);
                    if inner.written {
                        opaque(state, "effect group with its own blending");
                    }
                }
                PainterEffect::Paint(p) if p.stroke_count() > 0 && self.writes(&p.header, channel) => {
                    let colours = p.colors.is_empty() || p.colors.iter().any(|c| c.channels().contains(painter));
                    if colours {
                        opaque(state, format!("{} strokes", p.tool.name()));
                    }
                }
                PainterEffect::Filter(f) if self.writes(&f.header, channel) => {
                    let name = f.procedural.as_ref().map_or("?", |p| p.name());
                    opaque(state, format!("filter \"{name}\""));
                }
                PainterEffect::Levels(l) if self.writes(&l.header, channel) => opaque(state, "levels"),
                PainterEffect::ColorSelection(c) if self.writes(&c.header, channel) => {
                    opaque(state, "colour selection")
                }
                PainterEffect::PolygonFill(p) if self.writes(&p.header, channel) => opaque(state, "polygon fill"),
                PainterEffect::Other(raw) => opaque(state, format!("unknown effect {}", raw.class)),
                _ => {}
            }
        }
    }

    /// True if the effect's `channelTypes` includes the channel and its blending does not switch it off.
    /// An empty mask outside a mask stack is read as "all channels" (conservative).
    fn writes(&self, header: &EffectHeader, channel: Channel) -> bool {
        let painter = self.channels.painter(channel);
        let listed = header.channels.is_empty() || self.channels.mask_has(header.channels, channel);
        listed && ResolvedBlend::of(&header.blending, painter, Holder::Layer, self.table).contributes()
    }

    fn fill(&self, fill: &FillEffect, channel: Channel, state: &mut State) {
        let painter = self.channels.painter(channel);
        if !fill.header.channels.is_empty() && !self.channels.mask_has(fill.header.channels, channel) {
            return;
        }
        let Some(source) = fill.sources.iter().find(|s| {
            let c = s.channels();
            c.contains(painter) || (c.is_all() && matches!(s, PainterSource::Procedural(_)))
        }) else {
            return;
        };
        let blend = ResolvedBlend::of(&fill.header.blending, painter, Holder::Layer, self.table);
        if !blend.contributes() {
            return;
        }
        match source {
            PainterSource::Uniform(u) => {
                let top = [u.color[0], u.color[1], u.color[2], 1.0];
                state.written = true;
                match state.value {
                    Acc::Empty => state.value = Acc::Uniform([top[0], top[1], top[2], blend.opacity]),
                    Acc::Uniform(below) => {
                        if !harmless(&blend, top) {
                            state.status.and(&blend.status());
                        }
                        if below[3] < 1.0 {
                            state
                                .status
                                .and(&Computability::approximated("uniform fills over partial coverage"));
                        }
                        state.value = Acc::Uniform(blend.mode().mix(below, top, blend.opacity));
                    }
                    _ => opaque(state, "uniform fill over a bitmap or procedural in one layer"),
                }
            }
            PainterSource::Bitmap(b) => match state.value {
                Acc::Empty => self.bitmap(fill, &b.url, channel, blend.opacity, state),
                _ => opaque(state, "bitmap fill over other content in one layer"),
            },
            PainterSource::Procedural(p) => {
                let name = p.procedural.as_ref().map_or("?", |p| p.name());
                opaque(state, format!("procedural fill \"{name}\""));
            }
            PainterSource::Reference(_) => opaque(state, "fill from an anchor"),
            PainterSource::Other(raw) => opaque(state, format!("unknown source {}", raw.class)),
        }
    }

    fn bitmap(&self, fill: &FillEffect, url: &str, channel: Channel, coverage: f32, state: &mut State) {
        state.written = true;
        let Some(spec) = self.channels.spec(channel) else {
            return opaque(state, "undeclared channel");
        };
        let loaded = match self.images.load(url) {
            Ok(l) => l,
            Err(e) => return opaque(state, e),
        };
        let mapped = map_projection(fill);
        let image = channel_pixels(&loaded, spec);
        state.status.and(&mapped.status_for(image.size(), self.resolution));
        state.value = Acc::Image(image, coverage, mapped.projection);
    }
}

/// A guessed normal-map mode with a flat normal on top cannot change anything (every such mode keeps the base).
pub(super) fn harmless(blend: &ResolvedBlend, top: [f32; 4]) -> bool {
    let flat = (top[0] - 0.5).abs() < 1e-3 && (top[1] - 0.5).abs() < 1e-3 && (top[2] - 1.0).abs() < 1e-3;
    blend.mode().is_normal_map() && flat
}

fn opaque(state: &mut State, reason: impl Into<String>) {
    state.written = true;
    state.value = Acc::Opaque;
    state.status.and(&Computability::not_computable(reason));
}
