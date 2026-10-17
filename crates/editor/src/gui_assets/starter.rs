//! Where the starter library comes from (AS-01, roadmap step 25). An installed copy next to the editor binary
//! (`<install folder>/starter`, built by `mcstarter` when packaging) is used when it is current. Otherwise the editor
//! builds its own into `starter/` next to the settings file, once, on a background thread, and again whenever the
//! starter crate's library version changes. The build is deterministic and takes a few seconds, so every machine gets
//! the same library without an install step; the tray adds the root when it is ready. The library is read-only to
//! the tray either way (root kind `Starter`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use materialcraft::diag::{Area, Level, Sink, SinkExt, Target};
use materialcraft_starter::{Options, build, is_current};

/// Folder of the starter library, in the install folder and under the settings folder.
pub const STARTER_FOLDER: &str = "starter";

/// The starter library: its folder once it is there, or the build making it.
pub struct StarterLibrary {
    folder: Option<PathBuf>,
    ready: bool,
    job: Option<Receiver<Result<(), String>>>,
    sink: Option<Arc<dyn Sink>>,
}

impl StarterLibrary {
    /// No starter library (settings in memory, tests).
    pub fn none() -> StarterLibrary {
        StarterLibrary { folder: None, ready: false, job: None, sink: None }
    }

    /// The installed starter library if it is current, else the one under `settings_dir`, built now in the
    /// background if it is missing or out of date.
    pub fn locate(settings_dir: Option<&Path>, sink: Arc<dyn Sink>) -> StarterLibrary {
        let installed = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|d| d.join(STARTER_FOLDER)));
        if let Some(dir) = installed.filter(|d| is_current(d)) {
            return StarterLibrary { folder: Some(dir), ready: true, job: None, sink: Some(sink) };
        }
        let Some(dir) = settings_dir.map(|d| d.join(STARTER_FOLDER)) else { return StarterLibrary::none() };
        if is_current(&dir) {
            return StarterLibrary { folder: Some(dir), ready: true, job: None, sink: Some(sink) };
        }
        let (tx, rx) = channel();
        let target = dir.clone();
        let spawned = std::thread::Builder::new().name("starter-library".into()).spawn(move || {
            let result = build(&target, &Options { jobs: 2, ..Options::default() }).map(|_| ()).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        if let Err(e) = spawned {
            sink.warn(Area::Assets, format!("The starter library could not be built: {e}."));
            return StarterLibrary::none();
        }
        StarterLibrary { folder: Some(dir), ready: false, job: Some(rx), sink: Some(sink) }
    }

    /// The library's folder, once it is ready.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref().filter(|_| self.ready)
    }

    /// Checks on the build; true when the library became ready in this call.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.job else { return false };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err("the build stopped".into()),
        };
        self.job = None;
        let place = self.folder.as_deref().map(|d| d.display().to_string()).unwrap_or_default();
        match result {
            Ok(()) => {
                self.ready = true;
                if let Some(s) = &self.sink {
                    s.info(Area::Assets, format!("Built the starter library in {place}."));
                }
            }
            Err(e) => {
                if let Some(s) = &self.sink {
                    s.about(Level::Warning, Area::Assets, Target::File(place.clone()), format!("The starter library could not be built in {place}: {e}."));
                }
            }
        }
        self.ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::diag::NullSink;

    #[test]
    fn a_current_library_under_the_settings_is_used_at_once_and_a_missing_one_without_settings_is_none() {
        let dir = std::env::temp_dir().join(format!("mc-tray-starter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let folder = dir.join(STARTER_FOLDER);
        build(&folder, &Options { texture_size: 16, jobs: 4 }).unwrap();
        let mut s = StarterLibrary::locate(Some(&dir), Arc::new(NullSink));
        assert_eq!(s.folder(), Some(folder.as_path()));
        assert!(!s.poll());
        assert_eq!(StarterLibrary::locate(None, Arc::new(NullSink)).folder(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
