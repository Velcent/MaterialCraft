//! The native effect kinds in the compositor: levels, colour selection, polygon fill and references.
//!
//! - Levels transform what is below them in their stack, in the channel's stored encoding.
//! - Colour selection (masks only) selects texels by the colours of a baked map of the set.
//! - Polygon fill writes its values into the texels whose triangle (from the [`crate::mesh::SurfaceMap`]) is
//!   picked; without a surface map it is not computable.
//! - A reference reads what an anchor holds: the stack the anchor sits in (a layer's content or mask), evaluated
//!   up to the anchor, through the reference's levels. References that loop back are cut off after a few steps.

use super::Compositor;
use super::pass::Pass;
use crate::paint::channel::Channel;
use crate::paint::effect::{ColorSelection, EffectKind, Levels, PickLookup, PolygonFill, Reference};
use crate::paint::image::Rect;
use crate::paint::layer::{Fill, Layer, LayerKind, LayerWalk};
use crate::paint::working_space::SpaceConverter;
use std::cell::Cell;

/// How deep references may read through other references.
const MAX_REFERENCE_DEPTH: u32 = 4;

thread_local! {
    /// References being evaluated on this thread (the compositor is shared between threads, a texel is not).
    static REFERENCE_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// True unless a reference is being evaluated on this thread (what is read through references may be cut off).
pub(super) fn at_top_level() -> bool {
    REFERENCE_DEPTH.with(Cell::get) == 0
}

/// Where an anchor sits.
#[derive(Debug, Clone, Copy)]
pub(super) enum AnchorSite<'s> {
    /// Content effect `index` of `layer`.
    Content { layer: &'s Layer, index: usize },
    /// Mask effect `index` of `layer`.
    Mask { layer: &'s Layer, index: usize },
}

impl Compositor<'_> {
    /// The anchor with `id` anywhere in the set.
    pub(super) fn find_anchor(&self, id: &str) -> Option<AnchorSite<'_>> {
        let is = |k: &EffectKind<_>| matches!(k, EffectKind::Anchor(a) if a.id == id);
        for (_, layer) in LayerWalk::new(&self.set.layers) {
            if let Some(index) = layer.effects.iter().position(|e| is(&e.kind)) {
                return Some(AnchorSite::Content { layer, index });
            }
            let mask = layer.mask.as_ref().and_then(|m| m.effects.iter().position(|e| matches!(&e.kind, EffectKind::Anchor(a) if a.id == id)));
            if let Some(index) = mask {
                return Some(AnchorSite::Mask { layer, index });
            }
        }
        None
    }

    /// Levels over content `c` (working space) of the pass's channel, applied in the stored encoding.
    pub(super) fn levels_top(&self, levels: &Levels, pass: &Pass, c: &[[f32; 4]]) -> Vec<[f32; 4]> {
        let mut px = c.to_vec();
        pass.space.to_stored_all(&mut px);
        for p in &mut px {
            *p = levels.apply(*p);
        }
        pass.space.to_working_all(&mut px);
        px
    }

    /// Whether the compositor has what the polygon fill needs (a surface map).
    pub(super) fn picks_computable(&self) -> bool {
        self.fills.surface.is_some()
    }

    /// Per texel of `r`: 1 where the triangle under the texel's centre is picked, else 0 (no anti-aliasing along
    /// the selection's edges, as Painter's texture cache shows); `None` without a surface map.
    pub(super) fn picked(&self, selection: &crate::paint::effect::Selection, r: Rect) -> Option<Vec<f32>> {
        let surface = self.fills.surface?;
        let lookup = PickLookup::new(selection, |name| (0..).map_while(|p| surface.part_name(p).map(|n| (p, n))).find(|(_, n)| *n == name).map(|(p, _)| p));
        let w = surface.width();
        let mut out = vec![0.0; r.area()];
        if lookup.is_empty() {
            return Some(out);
        }
        for (i, o) in out.iter_mut().enumerate() {
            let (x, y) = (r.x as usize + i % r.w as usize, r.y as usize + i / r.w as usize);
            if x < w
                && let Some((part, tri, _)) = surface.triangle_index(y * w + x)
                && lookup.covers(part, tri)
            {
                *o = 1.0;
            }
        }
        Some(out)
    }

    /// A content polygon fill: its values where picked, coverage 0 elsewhere.
    pub(super) fn polygon_fill_top(&self, p: &PolygonFill<crate::paint::effect::Content>, pass: &Pass) -> Option<Vec<[f32; 4]>> {
        let picked = self.picked(&p.selection, pass.r)?;
        let values = self.fill_top(&p.values, pass)?.into_pixels(pass.r.area());
        Some(values.into_iter().zip(picked).map(|(v, k)| [v[0], v[1], v[2], v[3] * k]).collect())
    }

    /// The selection weights of a colour selection over `r` (grey in red, coverage 1), or `None` without the map.
    pub(super) fn color_selection_top(&self, s: &ColorSelection, r: Rect) -> Option<Vec<[f32; 4]>> {
        let map = self.set.baked(&s.map)?;
        let mut px = vec![[0.0; 4]; r.area()];
        map.read_region(r, &mut px);
        Some(px.into_iter().map(|c| s.weight([c[0], c[1], c[2]])).map(|w| [w, w, w, 1.0]).collect())
    }

    /// Whether the reference's anchor exists.
    pub(super) fn reference_computable(&self, r: &Reference) -> bool {
        self.find_anchor(&r.anchor).is_some()
    }

    /// What the reference reads for the pass's channel (working space), or `None`.
    pub(super) fn reference_content(&self, r: &Reference, pass: &Pass) -> Option<Vec<[f32; 4]>> {
        let mut px = self.reference_stored(r, Some(pass.channel), pass.r)?;
        pass.space.to_working_all(&mut px);
        Some(px)
    }

    /// What the reference reads for a mask over `r`: grey in red, alpha = coverage.
    pub(super) fn reference_grey(&self, r: &Reference, rect: Rect) -> Option<Vec<[f32; 4]>> {
        let px = self.reference_stored(r, None, rect)?;
        Some(px.into_iter().map(|p| [p[0], p[0], p[0], p[3]]).collect())
    }

    /// The anchor's values in the stored encoding of the channel read, through the reference's levels.
    /// `target` is the channel being evaluated (`None` for a mask).
    fn reference_stored(&self, r: &Reference, target: Option<Channel>, rect: Rect) -> Option<Vec<[f32; 4]>> {
        let depth = REFERENCE_DEPTH.with(Cell::get);
        if depth >= MAX_REFERENCE_DEPTH {
            return None;
        }
        REFERENCE_DEPTH.with(|d| d.set(depth + 1));
        let read = self.anchor_stored(r, target, rect);
        REFERENCE_DEPTH.with(|d| d.set(depth));
        let mut px = read?;
        if let Some(levels) = &r.levels {
            for p in &mut px {
                *p = levels.apply(*p);
            }
        }
        Some(px)
    }

    fn anchor_stored(&self, r: &Reference, target: Option<Channel>, rect: Rect) -> Option<Vec<[f32; 4]>> {
        match self.find_anchor(&r.anchor)? {
            AnchorSite::Mask { layer, index } => {
                let mask = layer.mask.as_ref()?;
                Some(self.mask_values(layer, mask, index, rect).into_iter().map(|g| [g, g, g, 1.0]).collect())
            }
            AnchorSite::Content { layer, index } => {
                let channel = r.channel.or(target).or_else(|| first_channel(layer))?;
                let pass = Pass { channel, r: rect, space: SpaceConverter::new(self.working_space(channel)) };
                let mut px = self.content(layer, index, &pass, 0);
                pass.space.to_stored_all(&mut px);
                Some(px)
            }
        }
    }
}

/// The first channel a layer writes (its own blend entries, else its fill's channels).
fn first_channel(l: &Layer) -> Option<Channel> {
    l.blend.first().map(|b| b.channel).or_else(|| match &l.kind {
        LayerKind::Fill(f) => Fill::channels(f).first().copied(),
        _ => None,
    })
}
