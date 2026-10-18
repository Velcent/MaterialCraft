//! The replays a texture set's stack import will ask for ([`ReplayJobs`]), found up front so they can be painted in
//! parallel: every paint effect with strokes in a mask (one job each), and the brush work of layer content (an
//! effect group's paint effects together, or one bare paint effect), in the groupings the import uses.

use super::super::content::brush_work;
use crate::model::{PaintEffect, PainterEffect, PainterTextureSet};

/// The jobs of one set.
#[derive(Default)]
pub struct ReplayJobs<'s> {
    pub masks: Vec<&'s PaintEffect>,
    pub contents: Vec<Vec<&'s PaintEffect>>,
}

impl<'s> ReplayJobs<'s> {
    pub fn of(set: &'s PainterTextureSet) -> ReplayJobs<'s> {
        let mut jobs = ReplayJobs::default();
        for layer in set.all_layers() {
            jobs.content(&layer.content);
            if let Some(mask) = &layer.mask {
                jobs.mask(&mask.effects);
            }
        }
        jobs
    }

    fn mask(&mut self, effects: &'s [PainterEffect]) {
        for e in effects {
            match e {
                PainterEffect::Paint(p) if p.stroke_count() > 0 => self.masks.push(p),
                PainterEffect::Group(g) => self.mask(&g.effects),
                _ => {}
            }
        }
    }

    fn content(&mut self, effects: &'s [PainterEffect]) {
        for e in effects {
            match e {
                PainterEffect::Paint(p) if p.stroke_count() > 0 => self.contents.push(vec![p]),
                PainterEffect::Group(g) => {
                    let paints = brush_work(&g.effects);
                    if paints.iter().any(|p| p.stroke_count() > 0) {
                        self.contents.push(paints);
                    }
                    let nested: Vec<&PainterEffect> = g
                        .effects
                        .iter()
                        .filter(|e| matches!(e, PainterEffect::Group(_)))
                        .collect();
                    for n in nested {
                        self.content(std::slice::from_ref(n));
                    }
                }
                _ => {}
            }
        }
    }
}
