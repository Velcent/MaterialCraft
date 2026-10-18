//! Headless performance benchmarks of the Paint screen's real paths (roadmap step 51): a brush stroke on a new
//! project ([`stroke`]) and single layer edits on big imported projects ([`edits`]). They drive the same components
//! the screen drives (the document and its history, the brush tools, [`SceneSync`] with its maps worker, the GPU
//! scene's upload queue) without a window, and wait for every recomposite to arrive so the numbers are repeatable.
//!
//! Ignored by default; they read projects from the corpus. Run one at a time, at low priority:
//!
//! ```text
//! flock ../Corpus/.heavy.lock nice -n 10 \
//!     cargo test --release -p materialcraft-editor perf_ -- --ignored --nocapture --test-threads 1
//! ```
//!
//! `MC_PERF_MESH` names the stroke benchmark's mesh (default: the corpus's Capricon `LP.obj`), `MC_PERF_DOCS` the edit
//! benchmark's documents (`.mcpaint`, separated by `:`).
//!
//! Step 52 made them general: a benchmark is a scenario described as data (`materialcraft_perf::Scenario`), played
//! by the one [`runner`] ([`source`] opens, [`stroke_play`] and [`edit_play`] act, [`measure`] records). The two
//! tests above are thin wrappers around the built-in scenarios `stroke-2k` and `edits-big`; `mcperf` runs any
//! scenario through the editor binary's `--perf-run` and keeps the history.

mod edit_play;
#[cfg(test)]
mod edits;
mod measure;
pub mod runner;
mod source;
#[cfg(test)]
mod stroke;
mod stroke_play;

use super::channel_view::ChannelView;
use super::document_state::OpenDocument;
use super::scene_stats::SceneStats;
use super::scene_sync::SceneSync;
use materialcraft::diag::NullSink;
use materialcraft::gpu3d::{self, Shared};
use materialcraft::paint::Document;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// An open document with its scene, as the Paint screen holds them.
pub struct Bench {
    pub doc: OpenDocument,
    pub scene: SceneSync,
    pub gpu: Shared,
    pub view: ChannelView,
}

impl Bench {
    /// Opens `doc` and waits until every set's maps are on the GPU queue; returns the bench and that time.
    pub fn open(doc: Document, source: PathBuf) -> (Bench, Duration) {
        let mut b = Bench { doc: OpenDocument::new(doc, source, None), scene: SceneSync::default(), gpu: gpu3d::new_shared(), view: ChannelView::Material };
        let started = Instant::now();
        b.scene.rebuild(&mut b.doc, b.view);
        b.settle();
        (b, started.elapsed())
    }

    /// One frame's poll of the scene (the screen polls once per frame); returns its time.
    pub fn poll(&mut self) -> Duration {
        let started = Instant::now();
        self.scene.poll(&mut self.doc, &self.gpu, &NullSink);
        started.elapsed()
    }

    /// Polls until the scene has no work left; returns the time it took.
    pub fn settle(&mut self) -> Duration {
        let started = Instant::now();
        loop {
            self.poll();
            if !self.scene.is_busy() {
                return started.elapsed();
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    }

    /// Recomposites `sets` as an edit does and waits for the result: the time to the updated maps and what the
    /// scene did meanwhile.
    pub fn refresh(&mut self, sets: Vec<usize>) -> (Duration, SceneStats) {
        let before = self.scene.stats();
        let started = Instant::now();
        self.scene.refresh(&mut self.doc, self.view, sets);
        self.settle();
        (started.elapsed(), delta(before, self.scene.stats()))
    }
}

/// What the scene did between two readings of its stats.
pub fn delta(a: SceneStats, b: SceneStats) -> SceneStats {
    SceneStats {
        sets: b.sets - a.sets,
        patches: b.patches - a.patches,
        composite: b.composite - a.composite,
        prepare: b.prepare - a.prepare,
        upload_bytes: b.upload_bytes - a.upload_bytes,
        handoff: b.handoff - a.handoff,
    }
}

/// A corpus file from an environment variable or its default place; `None` (the benchmark is skipped) if missing.
#[cfg(test)]
pub fn corpus_file(var: &str, default: &str) -> Option<PathBuf> {
    let path = std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(default));
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipped: {} not found (set {var})", path.display());
        None
    }
}
