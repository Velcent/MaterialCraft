//! [`Thumbnails`]: content and mask thumbnails of the listed layers for the view channel (LS-08, LS-09),
//! computed on a background thread on a reduced grid ([`PreviewGrid`]). Only rows that are drawn ask for
//! thumbnails; a thumbnail is recomputed when its layer changed (compared with the copy it was made from),
//! and the old one is shown meanwhile. One job runs at a time; requests made meanwhile wait for the next.

use crate::gui_paint::job::Job;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use materialcraft::generators;
use materialcraft::paint::layer_preview::PreviewGrid;
use materialcraft::paint::{Channel, Layer, TextureSet};
use materialcraft::tex::Tex;
use std::collections::{HashMap, HashSet};

/// Longer edge of a thumbnail in texels.
pub const THUMB: u32 = 48;

/// A computed thumbnail pair.
struct Computed {
    id: String,
    snapshot: Layer,
    content: Tex,
    mask: Option<Tex>,
}

struct Entry {
    snapshot: Layer,
    content: TextureHandle,
    mask: Option<TextureHandle>,
}

/// What a row draws.
pub struct Pair<'a> {
    pub content: Option<&'a TextureHandle>,
    pub mask: Option<&'a TextureHandle>,
}

#[derive(Default)]
pub struct Thumbnails {
    /// The set and channel the cache is for; another one starts over.
    key: Option<(String, Channel)>,
    cache: HashMap<String, Entry>,
    job: Option<Job<Computed>>,
    in_flight: HashSet<String>,
    wanted: Vec<Layer>,
}

impl Thumbnails {
    /// Takes finished thumbnails; call once per frame before drawing rows.
    pub fn begin(&mut self, ctx: &egui::Context, set: &str, channel: Channel) {
        let key = (set.to_string(), channel);
        if self.key.as_ref() != Some(&key) {
            self.cache.clear();
            self.in_flight.clear();
            self.job = None; // dropping cancels it
            self.key = Some(key);
        }
        if let Some(job) = &mut self.job {
            for c in job.poll() {
                self.in_flight.remove(&c.id);
                let content = ctx.load_texture(format!("layer-thumb-{}", c.id), to_image(&c.content), TextureOptions::LINEAR);
                let mask = c.mask.map(|m| ctx.load_texture(format!("layer-mask-{}", c.id), to_image(&m), TextureOptions::LINEAR));
                self.cache.insert(c.id, Entry { snapshot: c.snapshot, content, mask });
            }
            if job.is_finished() {
                self.job = None;
                self.in_flight.clear();
            }
        }
    }

    /// The thumbnails of `layer` as far as they exist; asks for new ones if the layer changed.
    pub fn get(&mut self, layer: &Layer) -> Pair<'_> {
        let fresh = self.cache.get(&layer.id).is_some_and(|e| e.snapshot == *layer);
        if !fresh && !self.in_flight.contains(&layer.id) && !self.wanted.iter().any(|l| l.id == layer.id) {
            self.wanted.push(layer.clone());
        }
        match self.cache.get(&layer.id) {
            Some(e) => Pair { content: Some(&e.content), mask: if layer.mask.is_some() { e.mask.as_ref() } else { None } },
            None => Pair { content: None, mask: None },
        }
    }

    /// Starts a job for the thumbnails asked for this frame, unless one is running.
    pub fn end(&mut self, ctx: &egui::Context, set: &TextureSet, channel: Channel) {
        if self.wanted.is_empty() {
            return;
        }
        if self.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
            self.wanted.clear(); // asked again next frame
            return;
        }
        let layers = std::mem::take(&mut self.wanted);
        self.in_flight.extend(layers.iter().map(|l| l.id.clone()));
        let shell = shell_of(set);
        let repaint = ctx.clone();
        self.job = Some(Job::spawn(move |cancel, tx| {
            let grid = PreviewGrid::new(&shell, THUMB);
            let effects = generators::builtin();
            for layer in layers {
                if cancel.is_cancelled() {
                    return;
                }
                let reduced = grid.reduce(&layer);
                let content = grid.content(&reduced, channel, effects);
                let mask = grid.mask(&reduced, effects);
                if tx.send(Computed { id: layer.id.clone(), snapshot: layer, content, mask }).is_err() {
                    return;
                }
                repaint.request_repaint();
            }
        }));
    }
}

impl Thumbnails {
    /// Thumbnails in the cache (for tests).
    #[cfg(test)]
    pub fn cached(&self) -> usize {
        self.cache.len()
    }
}

/// The set without its layers (channels, resolution, baked maps), to send to the worker.
fn shell_of(set: &TextureSet) -> TextureSet {
    let mut shell = TextureSet::new(set.id.clone(), set.name.clone(), set.resolution);
    shell.channels = set.channels.clone();
    shell.baked = set.baked.clone();
    shell.material_slot = set.material_slot;
    shell.uv_set = set.uv_set;
    shell
}

/// Stored-encoding values with coverage as alpha, clamped to 8 bits.
pub fn to_image(t: &Tex) -> ColorImage {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let rgba: Vec<u8> = t.px.iter().flat_map(|p| [byte(p[0]), byte(p[1]), byte(p[2]), byte(p[3])]).collect();
    ColorImage::from_rgba_unmultiplied([t.w, t.h], &rgba)
}
