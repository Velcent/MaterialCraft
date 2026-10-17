//! Where stroke records sit in a document: next to the pixels of a paint layer, a content paint effect or a
//! mask paint effect ([`StrokeOwner`]), and the records that could not be read ([`StrokeProblem`]).

use super::record::StrokeData;
use crate::paint::effect::EffectKind;
use crate::paint::{Document, Layer, LayerKind};

/// Which paint pixels of a layer a stroke record belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrokeOwner {
    /// The paint layer's own pixels.
    Paint,
    /// Content paint effect `effect`.
    Effect { effect: usize },
    /// Mask paint effect `effect`.
    Mask { effect: usize },
}

impl Layer {
    /// Every stroke record of the layer: its paint pixels, then content effects, then mask effects.
    pub fn stroke_records(&self) -> Vec<(StrokeOwner, &StrokeData)> {
        let mut out = Vec::new();
        if let LayerKind::Paint(p) = &self.kind
            && let Some(s) = &p.strokes
        {
            out.push((StrokeOwner::Paint, s));
        }
        for (effect, e) in self.effects.iter().enumerate() {
            if let EffectKind::Paint(p) = &e.kind
                && let Some(s) = &p.strokes
            {
                out.push((StrokeOwner::Effect { effect }, s));
            }
        }
        if let Some(m) = &self.mask {
            for (effect, e) in m.effects.iter().enumerate() {
                if let EffectKind::Paint(p) = &e.kind
                    && let Some(s) = &p.strokes
                {
                    out.push((StrokeOwner::Mask { effect }, s));
                }
            }
        }
        out
    }

    /// The stroke record slot of `owner`; `None` if the layer has no paint pixels there.
    pub fn strokes_mut(&mut self, owner: StrokeOwner) -> Option<&mut Option<StrokeData>> {
        match owner {
            StrokeOwner::Paint => match &mut self.kind {
                LayerKind::Paint(p) => Some(&mut p.strokes),
                _ => None,
            },
            StrokeOwner::Effect { effect } => match &mut self.effects.get_mut(effect)?.kind {
                EffectKind::Paint(p) => Some(&mut p.strokes),
                _ => None,
            },
            StrokeOwner::Mask { effect } => match &mut self.mask.as_mut()?.effects.get_mut(effect)?.kind {
                EffectKind::Paint(p) => Some(&mut p.strokes),
                _ => None,
            },
        }
    }
}

/// A stroke record that could not be read when the document was loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrokeProblem {
    pub set: String,
    pub layer: String,
    pub owner: StrokeOwner,
    pub reason: String,
}

impl std::fmt::Display for StrokeProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let place = match self.owner {
            StrokeOwner::Paint => String::new(),
            StrokeOwner::Effect { effect } => format!(" effect {effect}"),
            StrokeOwner::Mask { effect } => format!(" mask effect {effect}"),
        };
        write!(f, "texture set \"{}\" / layer \"{}\"{place}: stroke record not readable ({})", self.set, self.layer, self.reason)
    }
}

/// Every stroke record of `doc` that could not be read (kept as bytes or lost), for a load report.
pub fn problems(doc: &Document) -> Vec<StrokeProblem> {
    let mut out = Vec::new();
    for set in &doc.texture_sets {
        for (_, layer) in set.walk_layers() {
            for (owner, data) in layer.stroke_records() {
                if let StrokeData::Unreadable(u) = data {
                    out.push(StrokeProblem { set: set.id.clone(), layer: layer.id.clone(), owner, reason: u.reason.clone() });
                }
            }
        }
    }
    out
}

/// Strokes and samples over every readable record of `doc`.
pub fn totals(doc: &Document) -> (usize, usize) {
    let records = doc.texture_sets.iter().flat_map(|s| s.walk_layers()).flat_map(|(_, l)| l.stroke_records()).filter_map(|(_, d)| d.record());
    records.fold((0, 0), |(strokes, samples), r| (strokes + r.strokes.len(), samples + r.sample_count()))
}
