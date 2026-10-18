//! [`EnvironmentLoader`]: keeps the renderer's environment in step with the view's choice (EN-01). A map is named
//! by its library path; it is looked up under the library roots, read and prefiltered on a worker thread (well under
//! a second for a 2K map), then handed to the GPU. The last few prepared maps are kept, so switching back is
//! instant. A map that can't be used leaves the built-in studio lighting the scene and says why; the view keeps
//! naming the map (the document still refers to it).

use materialcraft::diag::{Area, Sink, SinkExt};
use materialcraft::envmap::EnvMap;
use materialcraft::gpu3d::Shared;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

/// Prepared maps kept for switching back.
const KEEP: usize = 3;

/// What lights the view.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum EnvironmentStatus {
    /// The built-in studio.
    #[default]
    Studio,
    /// The map (library path) is being read and prefiltered; the previous environment lights meanwhile.
    Preparing(String),
    /// The map lights the view.
    Ready(String),
    /// The map can't be used; the studio lights the view.
    Failed { path: String, error: String },
}

#[derive(Default)]
pub struct EnvironmentLoader {
    /// The environment the GPU was last asked for (`None`: the studio); `Some(None)` once asked for the studio.
    asked: Option<Option<String>>,
    job: Option<(String, Receiver<Result<EnvMap, String>>)>,
    kept: Vec<(String, Arc<EnvMap>)>,
    status: EnvironmentStatus,
}

impl EnvironmentLoader {
    pub fn status(&self) -> &EnvironmentStatus {
        &self.status
    }

    /// True while a map is being prepared (the UI should keep polling).
    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    /// Asks for `want` (a map's library path, or `None` for the studio) if that changed, and collects a finished
    /// map. `roots` are the library folders the path is looked up in.
    pub fn sync(&mut self, want: Option<&str>, roots: &[PathBuf], gpu: &Shared, diag: &dyn Sink) {
        if self.asked.as_ref().map(Option::as_deref) != Some(want) {
            self.asked = Some(want.map(str::to_string));
            self.start(want, roots, gpu, diag);
        }
        self.poll(gpu, diag);
    }

    fn start(&mut self, want: Option<&str>, roots: &[PathBuf], gpu: &Shared, diag: &dyn Sink) {
        self.job = None;
        let Some(path) = want else {
            set(gpu, None);
            self.status = EnvironmentStatus::Studio;
            return;
        };
        if let Some((_, env)) = self.kept.iter().find(|(p, _)| p == path) {
            set(gpu, Some(env.clone()));
            self.status = EnvironmentStatus::Ready(path.to_string());
            return;
        }
        let Some(file) = resolve(path, roots) else {
            set(gpu, None);
            self.fail(path, "it is in none of the asset libraries".into(), diag);
            return;
        };
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new().name("environment".into()).spawn(move || {
            let result = std::fs::read(&file).map_err(|e| e.to_string()).and_then(|bytes| EnvMap::from_bytes(&bytes));
            let _ = tx.send(result);
        });
        match spawned {
            Ok(_) => {
                self.job = Some((path.to_string(), rx));
                self.status = EnvironmentStatus::Preparing(path.to_string());
            }
            Err(e) => self.fail(path, e.to_string(), diag),
        }
    }

    fn poll(&mut self, gpu: &Shared, diag: &dyn Sink) {
        let Some((path, rx)) = &self.job else { return };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("the worker stopped".into()),
        };
        let path = path.clone();
        self.job = None;
        match result {
            Ok(env) => {
                let env = Arc::new(env);
                set(gpu, Some(env.clone()));
                self.kept.retain(|(p, _)| *p != path);
                self.kept.insert(0, (path.clone(), env));
                self.kept.truncate(KEEP);
                diag.info(Area::Gpu, format!("The view is lit by {path}."));
                self.status = EnvironmentStatus::Ready(path);
            }
            Err(e) => {
                set(gpu, None);
                self.fail(&path, e, diag);
            }
        }
    }

    fn fail(&mut self, path: &str, error: String, diag: &dyn Sink) {
        diag.warn(Area::Gpu, format!("The environment {path} can't be used: {error}. The built-in studio lights the view."));
        self.status = EnvironmentStatus::Failed { path: path.to_string(), error };
    }
}

fn set(gpu: &Shared, env: Option<Arc<EnvMap>>) {
    if let Ok(mut g) = gpu.lock() {
        g.set_environment(env);
    }
}

/// The file of library path `path` under the first root that has it.
fn resolve(path: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    let rel = Path::new(path);
    if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return None;
    }
    roots.iter().map(|r| r.join(rel)).find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::diag::NullSink;
    use materialcraft::tex::Tex;

    fn wait(l: &mut EnvironmentLoader, want: Option<&str>, roots: &[PathBuf], gpu: &Shared) {
        // generous: prefiltering is slow in unoptimised test builds on a busy machine
        for _ in 0..12000 {
            l.sync(want, roots, gpu, &NullSink);
            if !l.is_busy() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn maps_load_from_the_libraries_and_failures_fall_back_to_the_studio() {
        let dir = std::env::temp_dir().join(format!("mc-env-loader-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("environments")).unwrap();
        let sky = Tex::from_fn(32, 16, |_, v| if v < 0.5 { [1.0, 1.2, 1.5, 1.0] } else { [0.2, 0.2, 0.2, 1.0] });
        std::fs::write(dir.join("environments/sky.hdr"), materialcraft::exr::encode_hdr(&sky)).unwrap();
        std::fs::write(dir.join("environments/broken.hdr"), b"#?RADIANCE\n\n-Y 9 +X 9\n").unwrap();
        let roots = vec![PathBuf::from("/nonexistent"), dir.clone()];
        let gpu = materialcraft::gpu3d::new_shared();
        let mut l = EnvironmentLoader::default();
        wait(&mut l, Some("environments/sky.hdr"), &roots, &gpu);
        assert_eq!(l.status(), &EnvironmentStatus::Ready("environments/sky.hdr".into()));
        wait(&mut l, Some("environments/broken.hdr"), &roots, &gpu);
        assert!(matches!(l.status(), EnvironmentStatus::Failed { .. }));
        wait(&mut l, Some("environments/missing.hdr"), &roots, &gpu);
        assert!(matches!(l.status(), EnvironmentStatus::Failed { .. }));
        wait(&mut l, Some("../escape.hdr"), &roots, &gpu);
        assert!(matches!(l.status(), EnvironmentStatus::Failed { .. }));
        // back to a kept map: at once, no worker
        l.sync(Some("environments/sky.hdr"), &roots, &gpu, &NullSink);
        assert!(!l.is_busy());
        assert_eq!(l.status(), &EnvironmentStatus::Ready("environments/sky.hdr".into()));
        wait(&mut l, None, &roots, &gpu);
        assert_eq!(l.status(), &EnvironmentStatus::Studio);
        std::fs::remove_dir_all(&dir).ok();
    }
}
