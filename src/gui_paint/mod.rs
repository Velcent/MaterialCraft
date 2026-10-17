//! Paint mode screen (milestone M4: view an imported Substance Painter project).
//!
//! [`PaintMode`] is the only type the rest of the editor sees. It wires small components together:
//! [`loader::Loader`] reads documents (and imports `.spp` through `mcsub`), [`scene_sync::SceneSync`]
//! keeps the GPU scene in step with the document, [`viewport::Viewport`] draws it and the
//! [`panels`] turn clicks into [`panels::PanelEvent`]s that are applied here.

mod cache;
mod channel_view;
mod demo;
mod document_state;
mod geometry;
mod job;
mod launch;
mod loader;
mod maps_worker;
mod mcsub;
mod panels;
mod scene_sync;
mod shot;
mod unlit;
mod viewport;
mod window;

use channel_view::ChannelView;
use document_state::OpenDocument;
use eframe::egui;
use launch::{Launch, StartDocument};
use loader::{Loaded, Loader};
use panels::PanelEvent;
use panels::import_report::ReportPanel;
use panels::layout::Screen;
use panels::status_bar::{Activity, Status};
use scene_sync::SceneSync;
use shot::ShotJob;
use std::path::PathBuf;
use viewport::Viewport;
use window::WindowTitle;

/// What the editor should do after a Paint-screen frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintAction {
    Stay,
    /// Go back to the home screen.
    Home,
}

/// Which files a file dialog offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    /// `.mcpaint` documents and `.spp` projects.
    Any,
    /// Substance Painter projects only.
    Substance,
}

/// All state of the Paint screen.
pub struct PaintMode {
    document: Option<OpenDocument>,
    loader: Option<Loader>,
    scene: SceneSync,
    viewport: Viewport,
    display: ChannelView,
    report: ReportPanel,
    status: Status,
    shot: Option<ShotJob>,
    title: WindowTitle,
}

impl PaintMode {
    /// Paint mode configured from the command line (`--paint`, `--paint-demo`, `--shot`, `--channel`).
    pub fn from_args() -> PaintMode {
        let launch = Launch::from_args(std::env::args().skip(1));
        let mut pm = PaintMode {
            document: None,
            loader: None,
            scene: SceneSync::default(),
            viewport: Viewport::new(),
            display: launch.channel.unwrap_or(ChannelView::Material),
            report: ReportPanel::default(),
            status: Status::info("Open a .mcpaint document or import a Substance Painter project (.spp)."),
            shot: launch.shot.map(ShotJob::new),
            title: WindowTitle::default(),
        };
        match launch.open {
            Some(StartDocument::File(p)) => pm.open(p),
            Some(StartDocument::Demo) => pm.show(Loaded { document: demo::document(), source: "demo".into(), summary: None }),
            None => {}
        }
        pm
    }

    /// True if a document was given on the command line (the editor then starts on this screen).
    pub fn wants_screen(&self) -> bool {
        self.document.is_some() || self.loader.is_some()
    }

    /// Shows a file dialog and opens the chosen file. Returns false if the user cancelled.
    pub fn pick(&mut self, pick: Pick) -> bool {
        let dialog = match pick {
            Pick::Any => rfd::FileDialog::new()
                .add_filter("Paint document or Substance Painter project", &["mcpaint", "spp"])
                .add_filter("MaterialCraft paint document", &["mcpaint"]),
            Pick::Substance => rfd::FileDialog::new().add_filter("Substance 3D Painter project", &["spp"]),
        };
        match dialog.pick_file() {
            Some(p) => {
                self.open(p);
                true
            }
            None => false,
        }
    }

    /// Starts loading `path` in the background (the current document stays until it is done).
    pub fn open(&mut self, path: PathBuf) {
        self.status = Status::info(format!("Opening {}", path.display()));
        self.loader = Some(Loader::start(path));
    }

    /// Draws the screen and handles its input.
    pub fn ui(&mut self, ctx: &egui::Context) -> PaintAction {
        if let Some(p) = window::dropped_document(ctx) {
            self.open(p);
        }
        self.poll_loader();
        if let Some(doc) = &mut self.document {
            self.scene.poll(doc, self.viewport.gpu());
            if let Some(e) = self.scene.error.take() {
                self.status = Status::error(e);
            }
        }
        let events = self.draw(ctx);
        let mut action = PaintAction::Stay;
        for ev in events {
            if self.apply(ev) == PaintAction::Home {
                action = PaintAction::Home;
            }
        }
        let name = self.document.as_ref().map(|d| d.name());
        self.title.update(ctx, name.as_deref());
        if self.loader.is_some() || self.scene.is_busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if let Some(shot) = &mut self.shot {
            let settled = self.loader.is_none() && self.document.is_some() && !self.scene.is_busy();
            shot.update(ctx, settled, self.viewport.frames());
        }
        action
    }

    fn draw(&mut self, ctx: &egui::Context) -> Vec<PanelEvent> {
        let screen = Screen {
            document: self.document.as_ref(),
            report: &mut self.report,
            viewport: &mut self.viewport,
            display: self.display,
            status: &self.status,
            activity: Activity { loading: self.loader.as_ref().map(|l| l.progress()), maps: self.scene.progress() },
        };
        panels::layout::show(ctx, screen)
    }

    fn apply(&mut self, ev: PanelEvent) -> PaintAction {
        match ev {
            PanelEvent::Open => {
                self.pick(Pick::Any);
            }
            PanelEvent::Import => {
                self.pick(Pick::Substance);
            }
            PanelEvent::Close => self.close(),
            PanelEvent::Home => {
                self.title.forget(); // the editor sets its own title meanwhile
                return PaintAction::Home;
            }
            PanelEvent::ResetCamera => self.viewport.reset_camera(),
            PanelEvent::ShowChannel(view) => {
                self.display = view;
                if let Some(doc) = &mut self.document {
                    self.scene.remap(doc, view);
                }
            }
            PanelEvent::SetVisible(set, visible) => {
                if let Some(doc) = &mut self.document {
                    doc.set_visible(set, visible);
                    self.scene.rebuild(doc, self.display);
                }
            }
            PanelEvent::CancelLoad => {
                self.loader = None;
                self.status = Status::info("Cancelled.");
            }
        }
        PaintAction::Stay
    }

    fn poll_loader(&mut self) {
        let Some(result) = self.loader.as_mut().and_then(|l| l.poll()) else {
            return;
        };
        let source = self.loader.take().map(|l| l.source().display().to_string()).unwrap_or_default();
        match result {
            Ok(loaded) => self.show(loaded),
            Err(e) => {
                self.status = Status::error(e.clone());
                if let Some(shot) = &self.shot {
                    shot.abort(&format!("{source}: {e}"));
                }
            }
        }
    }

    /// Replaces the current document with `loaded`.
    fn show(&mut self, loaded: Loaded) {
        let mut doc = OpenDocument::new(loaded.document, loaded.source, loaded.summary);
        let d = doc.doc();
        self.status = Status::info(format!("Opened {}: {} texture sets, {} mesh parts.", doc.source().display(), d.texture_sets.len(), d.mesh.parts.len()));
        self.scene.rebuild(&mut doc, self.display);
        self.viewport.reset_camera();
        self.document = Some(doc);
    }

    fn close(&mut self) {
        self.scene.clear(self.viewport.gpu());
        self.document = None;
        self.status = Status::info("Document closed.");
    }
}
