//! [`Slot`]: one part of a Painter layer stack whose pixels we cannot compute from the document, located so it
//! can be swapped for constants ([`super::substitute`]).
//!
//! Two roles: a **candidate** is an effect whose output we want to measure (a procedural fill — mask generator,
//! texture, material — or a filter, in a mask or in a layer's content; or brush work in a mask, which the import
//! replays); a **nuisance** is anything else we
//! cannot compute (brush strokes, levels, polygon fill, anchors …). Every candidate can also act as a nuisance
//! when another effect is measured. Nuisances in a mask are single effects (an effect group counts as one);
//! nuisances in a layer's content stand for the layer's whole content (the content of a paint layer is its
//! strokes; anything mixed with them is unknown too). Hidden layers and switched-off masks and effects
//! contribute nothing and are skipped.

use crate::model::{LayerKind, PainterEffect, PainterLayer, PainterSource, PainterTextureSet};
use crate::translate::replacements;

/// Indices from the set's top-level layers down through folders.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayerAddress(pub Vec<usize>);

/// Where an effect sits in its layer: an index into `content` or into the mask's effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    Content(usize),
    Mask(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EffectAddress {
    pub layer: LayerAddress,
    pub place: Place,
}

/// What kind of effect a candidate is (the parity subject family it belongs to).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// A mask generator (`mg_*`), ours or not yet.
    Generator,
    /// A single-output texture (grunge, noise, pattern) in a mask or a channel.
    Texture,
    /// A material feeding several channels.
    Material,
    Filter,
    /// Brush work in a mask (a paint effect, or an effect group holding only brush work), replayed by
    /// `translate::layers::replay`.
    Strokes,
}

/// The id stroke candidates carry as "ours" (the `brush.replay` parity subject).
pub const STROKES_ID: &str = "stroke_replay";

impl Family {
    pub fn id(self) -> &'static str {
        match self {
            Family::Generator => "generator",
            Family::Texture => "texture",
            Family::Material => "material",
            Family::Filter => "filter",
            Family::Strokes => "brush",
        }
    }
}

/// Why a nuisance cannot be computed; decides which roadmap step would remove it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NuisanceKind {
    Strokes,
    /// A procedural fill (generator, texture, material) that is not the measured one.
    Procedural,
    Filter,
    Levels,
    PolygonFill,
    ColorSelection,
    Anchor,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub address: EffectAddress,
    /// Painter's graph name (`mg_dirt`, `blur`).
    pub graph: String,
    /// Our generator or texture id standing in for it, if any.
    pub ours: Option<&'static str>,
    pub family: Family,
    /// `Folder / Layer` as shown in Painter.
    pub path: String,
    /// Painter's projection of a fill (0 = UV).
    pub projection: u64,
}

impl Candidate {
    pub fn in_mask(&self) -> bool {
        matches!(self.address.place, Place::Mask(_))
    }

    /// The id the census and the parity subjects use: ours where we have one, else Painter's graph name.
    pub fn id(&self) -> String {
        self.ours.map_or_else(|| self.graph.clone(), str::to_string)
    }
}

/// What can be swapped for constants.
#[derive(Debug, Clone, PartialEq)]
pub enum Slot {
    Candidate(Candidate),
    /// A mask effect we cannot compute.
    MaskEffect(EffectAddress, NuisanceKind),
    /// A layer whose content we cannot compute.
    Layer(LayerAddress, NuisanceKind),
}

/// A slot acting as a nuisance: what is swapped and how many states it has.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Nuisance {
    /// Overridden by a white or black fill at full strength: 2 states.
    MaskEffect(EffectAddress),
    /// The layer switched off, or its content a black or white fill at normal blending: 3 states.
    Layer(LayerAddress),
    /// Several unknown effects of one mask together: the whole mask black or white, 2 states.
    Mask(LayerAddress),
    /// Unknown content and an unknown mask together: the layer switched off, or black or white without a
    /// mask, 3 states.
    LayerWithMask(LayerAddress),
}

impl Nuisance {
    pub fn states(&self) -> usize {
        match self {
            Nuisance::MaskEffect(_) | Nuisance::Mask(_) => 2,
            Nuisance::Layer(_) | Nuisance::LayerWithMask(_) => 3,
        }
    }

    /// The layer (or folder) the nuisance sits in.
    pub fn layer(&self) -> &LayerAddress {
        match self {
            Nuisance::MaskEffect(a) => &a.layer,
            Nuisance::Layer(l) | Nuisance::Mask(l) | Nuisance::LayerWithMask(l) => l,
        }
    }
}

impl Slot {
    /// The slot's nuisance form and kind.
    pub fn nuisance(&self) -> (Nuisance, NuisanceKind) {
        match self {
            Slot::Candidate(c) => {
                let kind = match c.family {
                    Family::Filter => NuisanceKind::Filter,
                    Family::Strokes => NuisanceKind::Strokes,
                    _ => NuisanceKind::Procedural,
                };
                match c.address.place {
                    Place::Mask(_) => (Nuisance::MaskEffect(c.address.clone()), kind),
                    Place::Content(_) => (Nuisance::Layer(c.address.layer.clone()), kind),
                }
            }
            Slot::MaskEffect(a, k) => (Nuisance::MaskEffect(a.clone()), *k),
            Slot::Layer(l, k) => (Nuisance::Layer(l.clone()), *k),
        }
    }

    pub fn candidate(&self) -> Option<&Candidate> {
        match self {
            Slot::Candidate(c) => Some(c),
            _ => None,
        }
    }
}

/// Every slot of the set's visible layers, bottom → top.
pub fn slots(set: &PainterTextureSet) -> Vec<Slot> {
    let mut out = Vec::new();
    walk(&set.layers, &[], "", &mut out);
    out
}

fn walk(layers: &[PainterLayer], parent: &[usize], path: &str, out: &mut Vec<Slot>) {
    for (i, l) in layers.iter().enumerate().filter(|(_, l)| l.visible) {
        let mut address = parent.to_vec();
        address.push(i);
        let name = match path.is_empty() {
            true => l.name.clone(),
            false => format!("{path} / {}", l.name),
        };
        if let Some(mask) = l.mask.as_ref().filter(|m| m.enabled) {
            for (k, e) in mask.effects.iter().enumerate().filter(|(_, e)| e.is_enabled()) {
                let at = EffectAddress {
                    layer: LayerAddress(address.clone()),
                    place: Place::Mask(k),
                };
                if let Some(slot) = mask_slot(e, at, &name) {
                    out.push(slot);
                }
            }
        }
        match &l.kind {
            LayerKind::Group(children) => walk(children, &address, &name, out),
            _ => content_slots(l, LayerAddress(address), &name, out),
        }
    }
}

fn mask_slot(e: &PainterEffect, at: EffectAddress, path: &str) -> Option<Slot> {
    match e {
        PainterEffect::Fill(f) if matches!(f.sources.first(), Some(PainterSource::Procedural(_))) => {
            candidate(e, at, path)
        }
        PainterEffect::Filter(_) => candidate(e, at, path),
        PainterEffect::Paint(_) | PainterEffect::Group(_) if brush_work_only(e) => Some(Slot::Candidate(Candidate {
            address: at,
            graph: "strokes".into(),
            ours: Some(STROKES_ID),
            family: Family::Strokes,
            path: path.to_string(),
            projection: 0,
        })),
        other => unknown(other).map(|kind| Slot::MaskEffect(at, kind)),
    }
}

/// A layer's content: candidates, or one layer nuisance when anything else in it is unknown.
fn content_slots(l: &PainterLayer, layer: LayerAddress, path: &str, out: &mut Vec<Slot>) {
    let mut candidates = Vec::new();
    let mut nuisance: Option<NuisanceKind> = None;
    for (k, e) in l.content.iter().enumerate().filter(|(_, e)| e.is_enabled()) {
        let at = EffectAddress {
            layer: layer.clone(),
            place: Place::Content(k),
        };
        let kind = match e {
            PainterEffect::Fill(f) => {
                let procedural = f
                    .sources
                    .iter()
                    .filter(|s| matches!(s, PainterSource::Procedural(_)))
                    .count();
                if f.sources.iter().any(|s| matches!(s, PainterSource::Reference(_))) {
                    Some(NuisanceKind::Anchor)
                } else if f.sources.iter().any(|s| matches!(s, PainterSource::Other(_))) {
                    Some(NuisanceKind::Unknown)
                } else if procedural > 1 {
                    Some(NuisanceKind::Procedural)
                } else {
                    if procedural == 1 {
                        candidates.extend(candidate(e, at, path));
                    }
                    None
                }
            }
            PainterEffect::Filter(_) => {
                candidates.extend(candidate(e, at, path));
                None
            }
            other => unknown(other),
        };
        // the first unknown names the layer's kind
        nuisance = nuisance.or(kind);
    }
    match nuisance {
        Some(kind) => out.push(Slot::Layer(layer, kind)),
        None => out.extend(candidates),
    }
}

/// True for brush work with strokes and nothing else unknown: a paint effect, or a group of paint effects,
/// anchors, computable fills and such groups.
fn brush_work_only(e: &PainterEffect) -> bool {
    fn walk(e: &PainterEffect, strokes: &mut bool) -> bool {
        if !e.is_enabled() {
            return true;
        }
        match e {
            PainterEffect::Paint(p) => {
                *strokes |= p.stroke_count() > 0;
                true
            }
            PainterEffect::Group(g) => g.effects.iter().all(|x| walk(x, strokes)),
            PainterEffect::Anchor(_) => true,
            other => unknown(other).is_none(),
        }
    }
    let mut strokes = false;
    walk(e, &mut strokes) && strokes
}

/// What makes an effect unknown when it is not a candidate (inside effect groups too), if anything.
fn unknown(e: &PainterEffect) -> Option<NuisanceKind> {
    if !e.is_enabled() {
        return None;
    }
    Some(match e {
        PainterEffect::Paint(p) if p.stroke_count() > 0 => NuisanceKind::Strokes,
        PainterEffect::Group(g) => return g.effects.iter().find_map(unknown),
        PainterEffect::Fill(f) => match f.sources.first()? {
            PainterSource::Procedural(_) => NuisanceKind::Procedural,
            PainterSource::Reference(_) => NuisanceKind::Anchor,
            PainterSource::Other(_) => NuisanceKind::Unknown,
            PainterSource::Uniform(_) | PainterSource::Bitmap(_) => return None,
        },
        PainterEffect::Filter(_) => NuisanceKind::Filter,
        PainterEffect::Levels(_) => NuisanceKind::Levels,
        PainterEffect::ColorSelection(_) => NuisanceKind::ColorSelection,
        PainterEffect::PolygonFill(_) => NuisanceKind::PolygonFill,
        PainterEffect::Other(_) => NuisanceKind::Unknown,
        _ => return None,
    })
}

fn candidate(e: &PainterEffect, address: EffectAddress, path: &str) -> Option<Slot> {
    let (graph, family, projection) = match e {
        PainterEffect::Fill(f) => {
            let p = f.sources.iter().find_map(|s| match s {
                PainterSource::Procedural(p) => Some(p),
                _ => None,
            })?;
            let graph = p.procedural.as_ref().map_or("?", |p| p.name()).to_string();
            let family = family_of(&graph, p.mode, matches!(address.place, Place::Mask(_)));
            (graph, family, f.projection)
        }
        PainterEffect::Filter(f) => {
            let graph = f.procedural.as_ref().map_or("?", |p| p.name()).to_string();
            (graph, Family::Filter, 0)
        }
        _ => return None,
    };
    let graph = graph.trim_end_matches(".sbsar").to_string();
    let ours = replacements::find(&graph).map(|r| r.generator);
    Some(Slot::Candidate(Candidate {
        address,
        graph,
        ours,
        family,
        path: path.to_string(),
        projection,
    }))
}

/// Our replacement table decides where it knows the graph; otherwise `mg_*` names generators, material mode
/// (1) materials, and the rest counts as a texture.
fn family_of(graph: &str, mode: u64, in_mask: bool) -> Family {
    let graph = graph.trim_end_matches(".sbsar");
    if replacements::texture_for(graph).is_some() {
        return Family::Texture;
    }
    if replacements::find(graph).is_some() || graph.starts_with("mg_") {
        return Family::Generator;
    }
    match mode == 1 && !in_mask {
        true => Family::Material,
        false => Family::Texture,
    }
}
