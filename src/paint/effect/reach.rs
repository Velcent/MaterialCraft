//! How far an effect reads around a texel: [`Reach`] answers "which texels of its input does this effect read
//! to produce one output texel?", per effect instance with its parameters at the texture set's resolution, and
//! [`EffectReach`] adds what an effect reads elsewhere in the stack (an anchor, the heights around its layer).
//!
//! The input of an effect is what lies below it in its own stack (for an effect layer: the composite below the
//! layer). Only filters read it beyond the texel itself; every other kind reads at most the same texel of its
//! input (through its blend) and otherwise its own data, baked maps, the surface map or its parameters, which
//! don't change with the stack. Radii are upper bounds in texels per axis (a square around the texel), never
//! too small, whatever rectangles the compositor evaluates. The concrete filters and generators answer behind
//! [`super::FilterKernel::reach`] and [`super::GeneratorSource::stack_reach`]; this module names none of them.
//!
//! A change confined to a rectangle of an effect's input changes its output only within [`Reach::grow`] of it,
//! and that is also the input rectangle an output rectangle needs. Effects one above the other add up
//! ([`Reach::then`]); effects side by side take the larger ([`Reach::union`]).

use super::registry::{EffectRegistry, EvalRequest, EvalTarget};
use super::{Effect, EffectDomain, EffectKind};
use crate::paint::channel::Channel;
use crate::paint::image::Rect;
use crate::paint::texture_set::TextureSet;

/// Which texels around an output texel an effect reads to produce it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Only the same texel.
    Texel,
    /// Texels within this many texels on each axis (never 0: that is [`Reach::Texel`]; see [`Reach::radius`]).
    Radius(u32),
    /// Any texel of the image (global statistics, or a result that depends on how the image is cut into tiles).
    Whole,
}

impl Reach {
    /// `Radius(r)`, or `Texel` for 0.
    pub fn radius(r: u32) -> Reach {
        if r == 0 { Reach::Texel } else { Reach::Radius(r) }
    }

    /// The radius in texels: 0 for `Texel`, `None` for `Whole`.
    pub fn texels(self) -> Option<u32> {
        match self {
            Reach::Texel => Some(0),
            Reach::Radius(r) => Some(r),
            Reach::Whole => None,
        }
    }

    pub fn is_texel(self) -> bool {
        self.texels() == Some(0)
    }

    /// The larger of two reaches: what reading both `self` and `other` around the same texel reads.
    pub fn union(self, other: Reach) -> Reach {
        match (self.texels(), other.texels()) {
            (Some(a), Some(b)) => Reach::radius(a.max(b)),
            _ => Reach::Whole,
        }
    }

    /// Two reaches one after the other (an effect of reach `self` over the output of one of reach `other`): the
    /// radii add up.
    pub fn then(self, other: Reach) -> Reach {
        match (self.texels(), other.texels()) {
            (Some(a), Some(b)) => a.checked_add(b).map_or(Reach::Whole, Reach::radius),
            _ => Reach::Whole,
        }
    }

    /// `rect` grown by the reach on every side, clipped to an image of `size`: the texels whose output may change
    /// when the input changes within `rect`, and the input texels the output over `rect` reads. `Whole` gives
    /// the whole image; an empty rectangle (or one outside the image) stays empty.
    pub fn grow(self, rect: Rect, size: (u32, u32)) -> Rect {
        let (x1, y1) = (rect.x.saturating_add(rect.w).min(size.0), rect.y.saturating_add(rect.h).min(size.1));
        if rect.x >= x1 || rect.y >= y1 {
            return Rect { x: rect.x.min(size.0), y: rect.y.min(size.1), w: 0, h: 0 };
        }
        let Some(r) = self.texels() else { return Rect { x: 0, y: 0, w: size.0, h: size.1 } };
        let (x0, y0) = (rect.x.saturating_sub(r), rect.y.saturating_sub(r));
        let (x1, y1) = (x1.saturating_add(r).min(size.0), y1.saturating_add(r).min(size.1));
        Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
}

/// A reference's read: the stack the anchor `anchor` sits in (a layer's content or mask), evaluated up to the
/// anchor, at the same texel. `channel` is the channel read from an anchor in content; `None`: the channel
/// being evaluated (for a mask reading content, the anchor layer's first channel). An anchor in a mask gives
/// the mask's grey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorRead {
    pub anchor: String,
    pub channel: Option<Channel>,
}

/// Everything one effect reads to produce one texel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectReach {
    /// Around the texel in its input (what is below it in its own stack).
    pub input: Reach,
    /// Around the texel in the Height channel of its own layer's content and of the composite below its layer
    /// (generators comparing heights, through [`super::StackContext`]); `None` if it reads neither.
    pub stack_height: Option<Reach>,
    /// What a reference reads elsewhere in the set, at the same texel.
    pub anchor: Option<AnchorRead>,
}

impl EffectReach {
    /// Reads only the same texel of its input and nothing else of the stack.
    pub fn local() -> EffectReach {
        EffectReach { input: Reach::Texel, stack_height: None, anchor: None }
    }

    /// True if the effect reads nothing but the same texel of its input.
    pub fn is_local(&self) -> bool {
        self.input.is_texel() && self.stack_height.is_none() && self.anchor.is_none()
    }
}

impl<D: EffectDomain> Effect<D> {
    /// What the effect reads to produce one texel of `target` on `set` (its resolution sizes the kernels):
    /// [`EffectKind::reach`]; a switched-off effect reads nothing.
    pub fn reach(&self, registry: &EffectRegistry, set: &TextureSet, target: EvalTarget) -> EffectReach {
        if self.enabled { self.kind.reach(registry, set, target) } else { EffectReach::local() }
    }
}

impl<D: EffectDomain> EffectKind<D> {
    /// What an effect of this kind reads to produce one texel of `target` on `set`:
    /// - filters: their kernel's [`super::FilterKernel::reach`] of the input;
    /// - generators: their [`super::GeneratorSource::stack_reach`] as heights around the layer (the compositor hands
    ///   the stack to mask generators only, so a content generator reads none in practice);
    /// - references: the anchor they read;
    /// - paint, fills (uniform, image, texture, material), levels, colour selection, polygon fill, anchors and
    ///   unknown kinds: nothing beyond the same texel.
    ///
    /// A procedural without a registered implementation is left out by the compositor and reads nothing.
    pub fn reach(&self, registry: &EffectRegistry, set: &TextureSet, target: EvalTarget) -> EffectReach {
        let (w, h) = set.resolution;
        let request = |params| EvalRequest { set, target, rect: Rect { x: 0, y: 0, w, h }, params, stack: None, surface: None, frame: None };
        match self {
            EffectKind::Filter(p) => {
                let input = registry.filter(&p.id).map_or(Reach::Texel, |k| k.reach(&request(&p.params)));
                EffectReach { input, ..EffectReach::local() }
            }
            EffectKind::Generator(p) => {
                let stack_height = registry.generator(&p.id).and_then(|g| g.stack_reach(&request(&p.params)));
                EffectReach { stack_height, ..EffectReach::local() }
            }
            EffectKind::Reference(r) => EffectReach { anchor: Some(AnchorRead { anchor: r.anchor.clone(), channel: r.channel }), ..EffectReach::local() },
            EffectKind::Paint(_)
            | EffectKind::Fill(_)
            | EffectKind::Levels(_)
            | EffectKind::ColorSelection(_)
            | EffectKind::PolygonFill(_)
            | EffectKind::Anchor(_)
            | EffectKind::Unknown(_) => EffectReach::local(),
        }
    }
}

#[cfg(test)]
#[path = "tests_reach.rs"]
mod tests;
