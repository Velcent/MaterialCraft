//! Session diagnostics on the editor side: the `log` facade's backend, the rotating log file and
//! the bridge into the core's [`diag`] model (design: MaterialCraft-docs/research/diag-editor-bridge.md).
//!
//! ```text
//! log::warn!(…) in any crate ──> AppLogger ──┬─> stderr, log file
//!                                            └─> Collector ──drain, once a frame──> history ──> status bar / log panel
//! editor code ──> &dyn Sink (TeeSink) ───────┬─> log file
//!                                            └─> Collector
//! ```
//!
//! The log file is `<settings dir>/logs/materialcraft.log`; each start moves the previous one to
//! `materialcraft.1.log` (and that one to `.2`), so the log of a run that crashed survives the next
//! launch (LG-11). Levels: `info` for our crates, `warn` for the rest; `RUST_LOG` overrides that.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod file;
mod filter;
mod logger;
mod tee;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use materialcraft::diag::{Collector, RingBuffer, Sink};

use file::{LogFile, MAX_FILE_BYTES};
use logger::SharedFile;
use tee::TeeSink;

/// The editor's diagnostics: one per process, created before the window opens so start-up
/// messages are kept. Owns the collector every thread writes to and the history the UI shows.
pub struct Diagnostics {
    file: SharedFile,
    collector: Arc<Collector>,
    sink: Arc<TeeSink>,
    history: RingBuffer,
}

impl Diagnostics {
    /// Installs the `log` backend (if no other one is installed) and starts collecting. Nothing
    /// is written to disk until [`Diagnostics::attach_dir`].
    pub fn start() -> Diagnostics {
        let file: SharedFile = Arc::new(Mutex::new(LogFile::new(MAX_FILE_BYTES)));
        let collector = Arc::new(Collector::default());
        logger::install(file.clone(), collector.clone());
        let sink = Arc::new(TeeSink::new(file.clone(), collector.clone(), true));
        Diagnostics { file, collector, sink, history: RingBuffer::default() }
    }

    /// Rotates the logs in `dir` and sends lines to the fresh file, first those kept so far;
    /// returns its path.
    pub fn attach_dir(&self, dir: &Path) -> Result<PathBuf, String> {
        let (path, file) = file::rotate(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        self.file.lock().unwrap_or_else(PoisonError::into_inner).attach(file);
        Ok(path)
    }

    /// The sink for the editor's own messages (status, errors), for any thread.
    pub fn sink(&self) -> Arc<dyn Sink> {
        self.sink.clone()
    }

    /// Moves what arrived since the last call into the history. Call once per frame.
    pub fn drain(&mut self) {
        self.collector.drain_into(&mut self.history);
    }

    /// Everything this session reported, oldest first.
    pub fn history(&self) -> &RingBuffer {
        &self.history
    }
}
