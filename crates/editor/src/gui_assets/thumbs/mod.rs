//! [`ThumbCache`]: the tray's thumbnails, made off the UI thread ([`worker`]), kept on disk between runs
//! ([`disk`]) and as textures while they are in use. Only drawn cells ask for a thumbnail; a few finished ones
//! are uploaded per frame, and the least recently drawn are let go once too many are held.

mod disk;
pub mod render;
mod worker;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use materialcraft::tex::Tex;

use super::results::{Item, ThumbSource};
pub use disk::DiskCache;
use worker::Workers;

/// Textures held at most.
const MAX_TEXTURES: usize = 768;
/// Finished thumbnails uploaded per frame at most (the rest wait a frame).
const UPLOADS_PER_FRAME: usize = 24;

/// Which thumbnail: its source as of a modification time and size.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThumbKey {
    pub source: ThumbSource,
    pub stamp: (Option<SystemTime>, u64),
}

enum Slot {
    Pending,
    Ready(TextureHandle),
    Failed,
}

/// Thumbnail textures by asset.
pub struct ThumbCache {
    slots: HashMap<ThumbKey, (Slot, u64)>,
    workers: Workers,
    frame: u64,
    uploads: usize,
}

impl ThumbCache {
    /// A cache keeping its files in `disk_dir` (none: in memory only).
    pub fn new(disk_dir: Option<PathBuf>) -> ThumbCache {
        ThumbCache { slots: HashMap::new(), workers: Workers::new(DiskCache::new(disk_dir)), frame: 0, uploads: 0 }
    }

    /// Takes finished thumbnails and lets go of old ones. Call once per frame before drawing.
    pub fn begin(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        self.uploads = 0;
        while self.uploads < UPLOADS_PER_FRAME {
            let Ok((key, made)) = self.workers.rx.try_recv() else { break };
            let Some((slot, _)) = self.slots.get_mut(&key) else { continue };
            *slot = match made {
                Ok(tex) if tex.w > 0 && tex.px.len() == tex.w * tex.h => {
                    self.uploads += 1;
                    Slot::Ready(ctx.load_texture("asset-thumb", to_image(&tex), TextureOptions::LINEAR))
                }
                _ => Slot::Failed,
            };
        }
        if self.uploads == UPLOADS_PER_FRAME {
            ctx.request_repaint();
        }
        if self.slots.len() > MAX_TEXTURES {
            self.evict();
        }
    }

    /// The thumbnail of `item`, if it is ready; asks for it otherwise.
    pub fn get(&mut self, ctx: &egui::Context, item: &Item) -> Option<&TextureHandle> {
        if item.thumb == ThumbSource::None {
            return None;
        }
        let key = ThumbKey { source: item.thumb.clone(), stamp: item.stamp };
        let frame = self.frame;
        if !self.slots.contains_key(&key) {
            self.slots.insert(key.clone(), (Slot::Pending, frame));
            for dropped in self.workers.request(ctx, key.clone()) {
                if matches!(self.slots.get(&dropped), Some((Slot::Pending, _))) {
                    self.slots.remove(&dropped);
                }
            }
        }
        let (slot, used) = self.slots.get_mut(&key)?;
        *used = frame;
        match slot {
            Slot::Ready(t) => Some(t),
            Slot::Pending | Slot::Failed => None,
        }
    }

    /// Lets go of the least recently drawn finished thumbnails, down to three quarters of the limit.
    fn evict(&mut self) {
        let mut done: Vec<(u64, ThumbKey)> = self.slots.iter().filter(|(_, (s, _))| !matches!(s, Slot::Pending)).map(|(k, (_, f))| (*f, k.clone())).collect();
        done.sort_by_key(|(f, _)| *f);
        let excess = self.slots.len().saturating_sub(MAX_TEXTURES * 3 / 4);
        for (_, k) in done.into_iter().take(excess) {
            self.slots.remove(&k);
        }
    }
}

/// Display values with alpha, as 8-bit RGBA.
fn to_image(t: &Tex) -> ColorImage {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let rgba: Vec<u8> = t.px.iter().flat_map(|p| [byte(p[0]), byte(p[1]), byte(p[2]), byte(p[3])]).collect();
    ColorImage::from_rgba_unmultiplied([t.w, t.h], &rgba)
}
