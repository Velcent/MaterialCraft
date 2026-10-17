//! Effects: the entries of a layer's content stack and of its mask stack.
//!
//! A layer's content is its base (the layer kind: painted pixels or a fill) followed by an ordered stack of
//! [`ContentEffect`]s, bottom to top; a mask is an initial value followed by a stack of [`MaskEffect`]s
//! (see [`super::mask`]). Both stacks use the same effect kinds ([`EffectKind`]); they differ in what an
//! effect writes ([`EffectDomain`]): content effects write material channels, each with its own blend
//! entry, mask effects write one grey value with one blend.
//!
//! The model only holds data. Kinds the compositor evaluates itself are paint, fill, anchor and the native kinds
//! (levels, colour selection, polygon fill, reference; [`native`] has their per-value maths), the latter as far as
//! the compositor has what they read (a surface map, a baked map, the anchor); generators and filters are
//! evaluated by implementations registered behind [`registry::GeneratorSource`] and [`registry::FilterKernel`]
//! (the paint model names no concrete procedural). Everything else (unknown kinds, unregistered procedurals) is
//! kept but *not computable*: the compositor leaves it out and reports it ([`super::CompositeWarning`]).

pub mod kinds;
pub mod native;
pub mod params;
pub mod registry;

pub use kinds::{Anchor, ColorSelection, Levels, Pick, PickMode, PolygonFill, Procedural, Reference, Selection, UnknownEffect};
pub use native::PickLookup;
pub use params::Params;
pub use registry::{EffectRegistry, EvalRequest, EvalTarget, FilterInput, FilterKernel, GeneratorSource, StackContext};

use super::blend::BlendMode;
use super::channel::Channel;
use super::image::TiledImage;
use super::layer::{ChannelBlend, Fill, PaintContent};
use super::stroke::StrokeData;
use std::fmt::Debug;

/// What the effects of one kind of stack write: the blend an effect carries, the pixels of a paint effect
/// and the values of a fill. Implemented by [`Content`] and [`Grey`].
pub trait EffectDomain: Debug + Clone + PartialEq + 'static {
    type Blend: Debug + Clone + PartialEq;
    type Pixels: Debug + Clone + PartialEq;
    type Values: Debug + Clone + PartialEq;
}

/// Layer content: material channels. An effect affects only the channels it has a blend entry for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Content;

impl EffectDomain for Content {
    type Blend = Vec<ChannelBlend>;
    type Pixels = PaintContent;
    type Values = Fill;
}

/// A mask: one grey value per texel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grey;

impl EffectDomain for Grey {
    type Blend = GreyBlend;
    type Pixels = MaskPixels;
    type Values = f32;
}

/// How a mask effect combines with the mask value below it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GreyBlend {
    pub mode: BlendMode,
    /// 0..1.
    pub opacity: f32,
}

impl Default for GreyBlend {
    fn default() -> GreyBlend {
        GreyBlend { mode: BlendMode::Normal, opacity: 1.0 }
    }
}

/// Painted mask pixels: the grey value is red, alpha is coverage (straight). The image has the texture set's
/// resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPixels {
    pub image: TiledImage,
    /// The strokes the pixels were painted with, if recorded (see [`super::stroke`]).
    pub strokes: Option<StrokeData>,
}

impl MaskPixels {
    /// Pixels without a stroke record.
    pub fn new(image: TiledImage) -> MaskPixels {
        MaskPixels { image, strokes: None }
    }
}

/// One entry of an effect stack.
#[derive(Debug, Clone, PartialEq)]
pub struct Effect<D: EffectDomain> {
    /// Switched-off effects are kept but do nothing.
    pub enabled: bool,
    /// Display name; usually empty (anchors are named by it).
    pub label: String,
    pub blend: D::Blend,
    pub kind: EffectKind<D>,
}

/// An effect of a layer's content stack.
pub type ContentEffect = Effect<Content>;
/// An effect of a mask stack.
pub type MaskEffect = Effect<Grey>;

/// What an effect does.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectKind<D: EffectDomain> {
    /// Painted pixels.
    Paint(D::Pixels),
    /// Uniform values (content: per channel; mask: one grey value) or, in content, a material.
    Fill(D::Values),
    /// A procedural that produces values (dirt, edge wear, a grunge …), evaluated by a registered
    /// [`GeneratorSource`].
    Generator(Procedural),
    /// A procedural that transforms what is below it in its stack, evaluated by a registered [`FilterKernel`].
    Filter(Procedural),
    Levels(Levels),
    /// A mask from the colours of a baked map (usually the ID map).
    ColorSelection(ColorSelection),
    /// Values written into picked mesh triangles, polygons, parts or UV islands.
    PolygonFill(PolygonFill<D>),
    /// A named point of the stack that references can read.
    Anchor(Anchor),
    /// Reads what an anchor holds.
    Reference(Reference),
    /// A kind this version does not know (from a newer file), kept unchanged.
    Unknown(UnknownEffect),
}

impl<D: EffectDomain> Effect<D> {
    /// An enabled, unnamed effect.
    pub fn new(blend: D::Blend, kind: EffectKind<D>) -> Effect<D> {
        Effect { enabled: true, label: String::new(), blend, kind }
    }
}

impl<D: EffectDomain> EffectKind<D> {
    /// Stable id of the kind as used in files (`paint`, `fill`, `generator`, …); an unknown kind's own id.
    pub fn id(&self) -> &str {
        match self {
            EffectKind::Paint(_) => "paint",
            EffectKind::Fill(_) => "fill",
            EffectKind::Generator(_) => "generator",
            EffectKind::Filter(_) => "filter",
            EffectKind::Levels(_) => "levels",
            EffectKind::ColorSelection(_) => "color_selection",
            EffectKind::PolygonFill(_) => "polygon_fill",
            EffectKind::Anchor(_) => "anchor",
            EffectKind::Reference(_) => "reference",
            EffectKind::Unknown(u) => &u.kind,
        }
    }

    /// True for kinds the compositor evaluates itself (given what they read); generators and filters depend on
    /// the registry, unknown kinds are not computable.
    pub fn is_builtin(&self) -> bool {
        !matches!(self, EffectKind::Generator(_) | EffectKind::Filter(_) | EffectKind::Unknown(_))
    }
}

impl ContentEffect {
    /// The blend entry for `channel`; `None` means the effect does not affect it.
    pub fn blend_for(&self, channel: Channel) -> Option<&ChannelBlend> {
        self.blend.iter().find(|b| b.channel == channel)
    }

    /// Builder: adds or replaces the blend entry for `channel`.
    pub fn with_blend(mut self, channel: Channel, mode: BlendMode, opacity: f32) -> ContentEffect {
        let entry = ChannelBlend { channel, mode, opacity };
        match self.blend.iter_mut().find(|b| b.channel == channel) {
            Some(b) => *b = entry,
            None => self.blend.push(entry),
        }
        self
    }

    /// Every channel the effect names (blend entries, paint images, fill values), for validation.
    pub fn channel_lists(&self) -> Vec<Vec<Channel>> {
        let mut lists = vec![self.blend.iter().map(|b| b.channel).collect()];
        match &self.kind {
            EffectKind::Paint(p) => lists.push(p.images.iter().map(|(c, _)| *c).collect()),
            EffectKind::Fill(f) | EffectKind::PolygonFill(PolygonFill { values: f, .. }) => lists.push(f.channels()),
            _ => {}
        }
        lists
    }
}

#[cfg(test)]
mod tests;
