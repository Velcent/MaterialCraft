//! [`Workers`]: a few background threads that make thumbnails. Requests wait in a bounded queue and the newest
//! is served first, so while the user scrolls the thumbnails in view come before the ones scrolled past; when the
//! queue is full the oldest request is dropped (and asked for again if it comes back into view).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

use eframe::egui;
use materialcraft::tex::Tex;

use super::super::results::ThumbSource;
use super::ThumbKey;
use super::disk::DiskCache;
use super::render;

/// Threads making thumbnails.
const THREADS: usize = 2;
/// Requests kept waiting at most.
pub const QUEUE: usize = 256;

pub type Made = (ThumbKey, Result<Tex, String>);

struct Shared {
    queue: Mutex<VecDeque<ThumbKey>>,
    wake: Condvar,
    stop: AtomicBool,
    repaint: Mutex<Option<egui::Context>>,
    disk: DiskCache,
}

/// The pool; dropping it stops the threads after their current thumbnail.
pub struct Workers {
    shared: Arc<Shared>,
    tx: Sender<Made>,
    pub rx: Receiver<Made>,
    started: bool,
}

impl Workers {
    pub fn new(disk: DiskCache) -> Workers {
        let shared =
            Arc::new(Shared { queue: Mutex::new(VecDeque::new()), wake: Condvar::new(), stop: AtomicBool::new(false), repaint: Mutex::new(None), disk });
        let (tx, rx) = channel();
        Workers { shared, tx, rx, started: false }
    }

    /// Queues `key`; returns the requests dropped to make room.
    pub fn request(&mut self, ctx: &egui::Context, key: ThumbKey) -> Vec<ThumbKey> {
        if !self.started {
            self.start(ctx);
        }
        let Ok(mut q) = self.shared.queue.lock() else { return vec![key] };
        q.push_back(key);
        let mut dropped = Vec::new();
        while q.len() > QUEUE {
            if let Some(k) = q.pop_front() {
                dropped.push(k);
            }
        }
        self.shared.wake.notify_one();
        dropped
    }

    fn start(&mut self, ctx: &egui::Context) {
        self.started = true;
        if let Ok(mut r) = self.shared.repaint.lock() {
            *r = Some(ctx.clone());
        }
        for i in 0..THREADS {
            let (shared, tx) = (self.shared.clone(), self.tx.clone());
            let _ = std::thread::Builder::new().name(format!("asset-thumbs-{i}")).spawn(move || run(&shared, &tx));
        }
    }
}

impl Drop for Workers {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.wake.notify_all();
    }
}

fn run(shared: &Shared, tx: &Sender<Made>) {
    loop {
        let key = {
            let Ok(mut q) = shared.queue.lock() else { return };
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(k) = q.pop_back() {
                    break k;
                }
                q = match shared.wake.wait(q) {
                    Ok(q) => q,
                    Err(_) => return,
                };
            }
        };
        let made = make(&shared.disk, &key);
        if tx.send((key, made)).is_err() {
            return;
        }
        if let Some(ctx) = shared.repaint.lock().ok().and_then(|c| c.clone()) {
            ctx.request_repaint();
        }
    }
}

/// One thumbnail: from the disk cache, else made and kept there.
fn make(disk: &DiskCache, key: &ThumbKey) -> Result<Tex, String> {
    if let Some(t) = disk.load(&key.source, key.stamp) {
        return Ok(t);
    }
    let made = match &key.source {
        ThumbSource::None => return Err("no picture".into()),
        ThumbSource::Image(p) => render::image(p)?,
        ThumbSource::Material(p) => render::material(p)?,
        ThumbSource::Fragment(p) => render::fragment(p)?,
    };
    disk.store(&key.source, key.stamp, &made);
    Ok(made)
}
