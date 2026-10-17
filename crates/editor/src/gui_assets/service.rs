//! [`LibraryService`]: the asset library behind the tray, scanned and watched off the UI thread. A background
//! thread owns the scanning: it applies new roots and polls the watched folders about once a second (AS-19);
//! each change bumps a generation counter and asks for a repaint. The UI thread only reads, with `try_read`, so a
//! running scan never stalls a frame (the tray keeps showing its last results meanwhile).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard};
use std::time::Duration;

use eframe::egui;
use materialcraft::assets::{Library, LibraryRoot};
use materialcraft::diag::Sink;

/// How often the watched folders are polled.
const POLL: Duration = Duration::from_millis(1000);

enum Command {
    SetRoots(Vec<LibraryRoot>),
}

/// The library and the thread keeping it current.
pub struct LibraryService {
    library: Arc<RwLock<Library>>,
    generation: Arc<AtomicU64>,
    /// `None` for a service without a thread (tests): roots are applied at once.
    commands: Option<Sender<Command>>,
    repaint: Arc<Mutex<Option<egui::Context>>>,
    roots: Vec<LibraryRoot>,
    sink: Arc<dyn Sink>,
}

impl LibraryService {
    /// A service over `roots`, scanned and polled on a background thread.
    pub fn spawn(roots: Vec<LibraryRoot>, sink: Arc<dyn Sink>) -> LibraryService {
        let library = Arc::new(RwLock::new(Library::new()));
        let generation = Arc::new(AtomicU64::new(0));
        let repaint: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
        let (tx, rx) = channel::<Command>();
        let (lib, gen_, rep, s) = (library.clone(), generation.clone(), repaint.clone(), sink.clone());
        let spawned = std::thread::Builder::new().name("asset-library".into()).spawn(move || {
            let changed = || {
                gen_.fetch_add(1, Ordering::Relaxed);
                if let Some(ctx) = rep.lock().ok().and_then(|c| c.clone()) {
                    ctx.request_repaint();
                }
            };
            loop {
                match rx.recv_timeout(POLL) {
                    Ok(Command::SetRoots(roots)) => {
                        if let Ok(mut l) = lib.write() {
                            l.set_roots(roots, &*s);
                        }
                        changed();
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        let refresh = lib.write().map(|mut l| l.poll(&*s)).unwrap_or_default();
                        if !refresh.is_empty() {
                            changed();
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        });
        let mut service = LibraryService { library, generation, commands: spawned.is_ok().then_some(tx), repaint, roots: Vec::new(), sink };
        service.set_roots(roots);
        service
    }

    /// A service over an already scanned library, without a thread (tests, headless timing).
    #[cfg(test)]
    pub fn from_library(library: Library, sink: Arc<dyn Sink>) -> LibraryService {
        let roots = library.catalogue().roots().cloned().collect();
        LibraryService {
            library: Arc::new(RwLock::new(library)),
            generation: Arc::new(AtomicU64::new(1)),
            commands: None,
            repaint: Arc::new(Mutex::new(None)),
            roots,
            sink,
        }
    }

    /// Lets the thread wake the UI when the library changed. Call once per frame.
    pub fn attach(&self, ctx: &egui::Context) {
        if let Ok(mut c) = self.repaint.lock()
            && c.is_none()
        {
            *c = Some(ctx.clone());
        }
    }

    /// Makes the roots exactly `roots` (scanned in the background).
    pub fn set_roots(&mut self, roots: Vec<LibraryRoot>) {
        if roots == self.roots {
            return;
        }
        self.roots = roots.clone();
        match &self.commands {
            Some(tx) => {
                if tx.send(Command::SetRoots(roots)).is_err() {
                    self.commands = None;
                }
            }
            None => {
                if let Ok(mut l) = self.library.write() {
                    l.set_roots(roots, &*self.sink);
                }
                self.generation.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// The roots as configured (they may still be scanning).
    pub fn roots(&self) -> &[LibraryRoot] {
        &self.roots
    }

    /// Bumped on every change of the catalogue.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// The library, unless the background thread is changing it right now.
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, Library>> {
        self.library.try_read().ok()
    }

    /// The library, waiting for a running scan (tests).
    #[cfg(test)]
    pub fn read(&self) -> Option<RwLockReadGuard<'_, Library>> {
        self.library.read().ok()
    }
}
