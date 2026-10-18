//! [`Collector`]: walks a project's layer stacks (layer content, masks, nested groups, procedural inputs) and its
//! viewer settings for what can live in the user's library: the brushes of paint effects with strokes (one preset per
//! distinct setting), their bitmap alphas and stencils, bitmaps used as fills, paint colour or procedural inputs, and
//! the environment map. Baked mesh maps are the mesh's, not assets: never offered. Bitmaps not stored in the project
//! (shelf resources Painter did not embed) cannot be copied and are only counted.

use super::brush::BrushOffer;
use super::candidate::{CandidateKind, CandidateSource, LibraryCandidate, LibraryCandidates};
use super::origin::Origin;
use crate::model::{PaintEffect, PainterDocument, PainterEffect, PainterLayer, PainterSource, Procedural};
use std::collections::{BTreeMap, HashSet};

/// What the walk needs to know about the project besides its layer document.
pub struct ProjectFacts<'a> {
    /// The project file, as the provenance records it.
    pub project: String,
    /// URLs of the baked mesh maps (never offered).
    pub baked: HashSet<String>,
    /// The viewer's environment map URL.
    pub environment: Option<String>,
    /// True when the image a URL names is stored in the project (so it can be copied).
    pub stored: &'a dyn Fn(&str) -> bool,
    /// True for a resource name Painter gives baked maps (`Ambient Occlusion Map from Mesh …`).
    pub baked_name: &'a dyn Fn(&str) -> bool,
}

/// One image seen, by kind and URL.
struct ImageSeen {
    origin: Origin,
    uses: usize,
}

/// A brush offer and how many effects use it.
type Used = (BrushOffer, usize);

/// One brush seen (by its character), with the exact settings it was used with.
struct BrushSeen {
    /// Exact key → the offer and its uses, in first-use order.
    variants: Vec<(String, BrushOffer, usize)>,
    uses: usize,
    order: usize,
}

impl BrushSeen {
    fn add(&mut self, offer: BrushOffer) {
        self.uses += 1;
        let key = offer.key();
        match self.variants.iter_mut().find(|(k, _, _)| *k == key) {
            Some((_, _, n)) => *n += 1,
            None => self.variants.push((key, offer, 1)),
        }
    }

    /// The settings used most (the first of them on a tie): the preset's size, flow and opacity.
    fn offer(self) -> Option<BrushOffer> {
        let mut best: Option<(BrushOffer, usize)> = None;
        for (_, offer, n) in self.variants {
            if best.as_ref().is_none_or(|(_, m)| n > *m) {
                best = Some((offer, n));
            }
        }
        best.map(|(offer, _)| offer)
    }
}

/// Collects the candidates of one project.
pub struct Collector<'a> {
    facts: &'a ProjectFacts<'a>,
    images: BTreeMap<(CandidateKind, String), ImageSeen>,
    brushes: BTreeMap<String, BrushSeen>,
    left_out: BTreeMap<String, usize>,
}

impl<'a> Collector<'a> {
    pub fn new(facts: &'a ProjectFacts<'a>) -> Collector<'a> {
        Collector {
            facts,
            images: BTreeMap::new(),
            brushes: BTreeMap::new(),
            left_out: BTreeMap::new(),
        }
    }

    /// Every candidate of `document` and the viewer settings.
    pub fn collect(mut self, document: &PainterDocument) -> LibraryCandidates {
        for set in &document.texture_sets {
            self.layers(&set.layers);
        }
        if let Some(url) = self.facts.environment.clone() {
            self.image(CandidateKind::Environment, &url);
        }
        self.finish()
    }

    fn layers(&mut self, layers: &[PainterLayer]) {
        for l in layers {
            self.effects(&l.content);
            if let Some(mask) = &l.mask {
                self.effects(&mask.effects);
            }
            self.layers(l.children());
        }
    }

    fn effects(&mut self, effects: &[PainterEffect]) {
        for e in effects {
            match e {
                PainterEffect::Paint(p) => self.paint(p),
                PainterEffect::Fill(f) => f.sources.iter().for_each(|s| self.source(s, CandidateKind::Texture)),
                PainterEffect::Filter(f) => {
                    if let Some(p) = &f.procedural {
                        self.procedural(p);
                    }
                }
                PainterEffect::Group(g) => self.effects(&g.effects),
                _ => {}
            }
        }
    }

    fn paint(&mut self, p: &PaintEffect) {
        if p.stroke_count() > 0 {
            match BrushOffer::of(p) {
                Ok(offer) => {
                    let order = self.brushes.len();
                    self.brushes
                        .entry(offer.character())
                        .or_insert(BrushSeen {
                            variants: Vec::new(),
                            uses: 0,
                            order,
                        })
                        .add(offer);
                }
                Err(why) => *self.left_out.entry(format!("brush with a {why}")).or_default() += 1,
            }
        }
        if let Some(PainterSource::Bitmap(b)) = p.alpha.first() {
            self.image(CandidateKind::Alpha, &b.url);
        }
        if let Some(stencil) = &p.stencil {
            self.source(stencil, CandidateKind::Stencil);
        }
        p.colors.iter().for_each(|s| self.source(s, CandidateKind::Texture));
    }

    fn source(&mut self, s: &PainterSource, kind: CandidateKind) {
        match s {
            PainterSource::Bitmap(b) => self.image(kind, &b.url),
            PainterSource::Procedural(ps) => {
                if let Some(p) = &ps.procedural {
                    self.procedural(p);
                }
            }
            _ => {}
        }
    }

    /// The bitmaps plugged into a procedural's image inputs (most are baked maps, which are skipped).
    fn procedural(&mut self, p: &Procedural) {
        for input in &p.inputs {
            if let Some(s) = input.source.as_deref() {
                self.source(s, CandidateKind::Texture);
            }
        }
    }

    fn image(&mut self, kind: CandidateKind, url: &str) {
        let Some(origin) = Origin::of_url(url) else { return };
        if self.facts.baked.contains(url) || (self.facts.baked_name)(image_name(url)) {
            return;
        }
        if !(self.facts.stored)(url) {
            *self
                .left_out
                .entry(format!("{} not stored in the project", kind.label()))
                .or_default() += 1;
            return;
        }
        self.images
            .entry((kind, url.to_string()))
            .or_insert(ImageSeen { origin, uses: 0 })
            .uses += 1;
    }

    fn finish(mut self) -> LibraryCandidates {
        let stem = project_stem(&self.facts.project);
        let mut items = Vec::new();
        let mut seen: Vec<BrushSeen> = self.brushes.into_values().collect();
        seen.sort_by(|a, b| b.uses.cmp(&a.uses).then(a.order.cmp(&b.order)));
        let brushes: Vec<Used> = seen
            .into_iter()
            .filter_map(|b| Some((b.uses, b.offer()?)))
            .map(|(n, o)| (o, n))
            .collect();
        // a brush whose alpha cannot be copied is not offered
        let images = &self.images;
        let (brushes, orphans): (Vec<Used>, Vec<Used>) = brushes.into_iter().partition(|(b, _)| {
            b.alpha_url
                .as_ref()
                .is_none_or(|u| images.contains_key(&(CandidateKind::Alpha, u.clone())))
        });
        if !orphans.is_empty() {
            *self
                .left_out
                .entry("brush whose alpha is not stored in the project".to_string())
                .or_default() += orphans.len();
        }
        for (i, (offer, uses)) in brushes.into_iter().enumerate() {
            // A brush's alpha is offered too (as an alpha of its own); the brush brings it when ticked.
            let alpha = offer.alpha_url.as_ref().map(|u| image_key(CandidateKind::Alpha, u));
            items.push(LibraryCandidate {
                key: format!("brush:{}", i + 1),
                kind: CandidateKind::Brush,
                name: format!("{stem} brush {} - {}", i + 1, offer.tip_description()),
                origin: offer.origin.clone(),
                uses,
                source: CandidateSource::Brush {
                    preset: offer.preset,
                    alpha,
                },
            });
        }
        let mut images: Vec<((CandidateKind, String), ImageSeen)> = self.images.into_iter().collect();
        images.sort_by(|((ka, ua), a), ((kb, ub), b)| ka.cmp(kb).then(b.uses.cmp(&a.uses)).then(ua.cmp(ub)));
        for ((kind, url), seen) in images {
            items.push(LibraryCandidate {
                key: image_key(kind, &url),
                kind,
                name: image_name(&url).to_string(),
                origin: seen.origin,
                uses: seen.uses,
                source: CandidateSource::Image { url },
            });
        }
        let left_out = self
            .left_out
            .into_iter()
            .map(|(why, n)| format!("{n} × {why}"))
            .collect();
        LibraryCandidates {
            project: self.facts.project.clone(),
            items,
            left_out,
        }
    }
}

fn image_key(kind: CandidateKind, url: &str) -> String {
    format!("{}:{url}", kind.label())
}

/// The resource's own name: `/<name>?version=…` → `<name>`, `resource://<shelf>/<dir>/<name>?version=…` → `<name>`.
pub fn image_name(url: &str) -> &str {
    let path = url.split('?').next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.is_empty() { path } else { name }
}

/// The project's file name without folder and extension.
fn project_stem(project: &str) -> &str {
    let name = project.rsplit(['/', '\\']).next().unwrap_or(project);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}
