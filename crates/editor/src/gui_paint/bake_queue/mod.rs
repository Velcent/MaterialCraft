//! [`BakeQueue`]: bakes texture sets one after another on a background thread (BK2-08, BK-08), with progress
//! that names the set being baked and a cancel that stops the running bake within a few thousand texels.
//!
//! The queue knows nothing about the screen: it is given the mesh, the folder to look for high-poly files in and
//! a list of [`BakeJob`]s (built from the document's bake configuration), and streams [`BakeEvent`]s back. The
//! Paint screen stores each result in the set's baked maps as an undoable step (`super::baking`); tests drive the
//! same API headless.

mod high;
mod jobs;
mod sink;
#[cfg(test)]
mod tests;

pub use jobs::BakeJob;

use high::HighScenes;
use materialcraft::bake::{self, BakeError, BakedMaps, CancelToken};
use materialcraft::diag::{Area, Level, Sink, SinkExt, Target};
use materialcraft::mesh::Mesh;
use sink::SetSink;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

/// What the queue reports, in order.
#[derive(Debug)]
pub enum BakeEvent {
    /// Job `index` of `total` (0-based) starts.
    Started {
        name: String,
        index: usize,
        total: usize,
    },
    Baked {
        set: String,
        maps: BakedMaps,
    },
    Failed {
        set: String,
        error: String,
    },
    /// The queue has ended; `cancelled` if it was stopped before the last job finished.
    Finished {
        cancelled: bool,
    },
}

/// A running queue. Dropping it cancels the bake.
pub struct BakeQueue {
    rx: Receiver<BakeEvent>,
    cancel: CancelToken,
    /// (set name, index, total) of the bake running now.
    current: Option<(String, usize, usize)>,
    finished: bool,
}

impl BakeQueue {
    /// Starts baking `jobs` in order. `base_dir` is searched for high-poly files given by name; entries of the
    /// bake go to `log` (area Bake, targets keyed by set).
    pub fn start(mesh: Arc<Mesh>, base_dir: Option<PathBuf>, jobs: Vec<BakeJob>, log: Arc<dyn Sink>) -> BakeQueue {
        let (tx, rx) = channel();
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let total = jobs.len();
        std::thread::spawn(move || {
            let mut highs = HighScenes::new(Arc::clone(&mesh), base_dir);
            for (index, job) in jobs.into_iter().enumerate() {
                if token.is_cancelled() {
                    break;
                }
                if tx.send(BakeEvent::Started { name: job.name.clone(), index, total }).is_err() {
                    return;
                }
                let event = match run_one(&mesh, &mut highs, &job, &token, &log) {
                    Ok(maps) => BakeEvent::Baked { set: job.set, maps },
                    Err(None) => break,
                    Err(Some(error)) => BakeEvent::Failed { set: job.set, error },
                };
                if tx.send(event).is_err() {
                    return;
                }
            }
            let _ = tx.send(BakeEvent::Finished { cancelled: token.is_cancelled() });
        });
        BakeQueue { rx, cancel, current: None, finished: false }
    }

    /// Events since the last call.
    pub fn poll(&mut self) -> Vec<BakeEvent> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(ev) => {
                    match &ev {
                        BakeEvent::Started { name, index, total, .. } => self.current = Some((name.clone(), *index, *total)),
                        BakeEvent::Finished { .. } => {
                            self.current = None;
                            self.finished = true;
                        }
                        _ => {}
                    }
                    out.push(ev);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.current = None;
                    self.finished = true;
                    break;
                }
            }
        }
        out
    }

    /// Stops the running bake; no later job starts.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// (set name, jobs done, total) while a bake runs.
    pub fn progress(&self) -> Option<(&str, usize, usize)> {
        self.current.as_ref().map(|(n, i, t)| (n.as_str(), *i, *t))
    }
}

impl Drop for BakeQueue {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// One job; `Err(None)` when it was cancelled, else the reason it failed (also logged).
fn run_one(mesh: &Mesh, highs: &mut HighScenes, job: &BakeJob, cancel: &CancelToken, log: &Arc<dyn Sink>) -> Result<BakedMaps, Option<String>> {
    let sink = SetSink { inner: Arc::clone(log), set: job.set.clone(), name: job.name.clone() };
    let high = highs.get(&job.high).map_err(|e| {
        let text = format!("no high-poly mesh: {e}");
        sink.about(Level::Error, Area::Bake, Target::BakeSetting("high".into()), text.clone());
        Some(text)
    })?;
    bake::bake_set(mesh, &job.target, &high, &job.settings, cancel, &sink).map_err(|e| match e {
        BakeError::Cancelled => None,
        e => Some(e.to_string()),
    })
}
