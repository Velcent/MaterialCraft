//! Paint mode screen (milestone M4: view an imported Substance Painter project).
//!
//! [`PaintMode`] is the only type the rest of the editor sees. It wires small components together:
//! [`loader::Loader`] reads documents (and imports `.spp` in-process), [`scene_sync::SceneSync`]
//! keeps the GPU scene in step with the document, [`viewport::Viewport`] draws it, the
//! [`shell::Shell`] holds the docking workspace, shortcuts and settings, and the [`panels`] turn
//! clicks and key presses into [`panels::PanelEvent`]s that are applied here.

mod cache;
mod channel_view;
mod demo;
mod document_state;
mod geometry;
mod job;
mod launch;
mod loader;
mod maps_worker;
mod navigation;
mod panels;
mod scene_sync;
pub mod shell;
mod shot;
mod spp_import;
mod unlit;
mod viewport;
mod window;

use crate::gui_settings::UserSettings;
use channel_view::ChannelView;
use document_state::OpenDocument;
use eframe::egui;
use launch::{Launch, StartDocument};
use loader::{Loaded, Loader};
use materialcraft::diag::{Area, RingBuffer, Sink, SinkExt};
use materialcraft::paint::diag_entries;
use panels::PanelEvent;
use panels::import_report::ReportPanel;
use panels::layout::Screen;
use panels::status_bar::Activity;
use scene_sync::SceneSync;
use shell::Shell;
use shot::ShotJob;
use std::path::PathBuf;
use std::sync::Arc;
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
    /// Where status messages and errors go (the log file and the session history).
    diag: Arc<dyn Sink>,
    /// Docking, shortcuts, tools and the user settings they live in.
    shell: Shell,
    shot: Option<ShotJob>,
    title: WindowTitle,
}

impl PaintMode {
    /// Paint mode configured from the command line (`--paint`, `--paint-demo`, `--shot`, `--channel`);
    /// its messages go to `diag`, its workspace is kept in `settings`.
    pub fn from_args(diag: Arc<dyn Sink>, settings: UserSettings) -> PaintMode {
        let launch = Launch::from_args(std::env::args().skip(1));
        let mut pm = PaintMode {
            document: None,
            loader: None,
            scene: SceneSync::default(),
            viewport: Viewport::new(diag.clone()),
            display: launch.channel.unwrap_or(ChannelView::Material),
            report: ReportPanel::default(),
            shell: Shell::new(settings, diag.clone()),
            diag,
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
        self.diag.info(Area::Project, format!("Opening {}", path.display()));
        self.loader = Some(Loader::start(path));
    }

    /// Draws the screen and handles its input; `history` is the session's diagnostics (the status
    /// bar shows the newest entry).
    pub fn ui(&mut self, ui: &mut egui::Ui, history: &RingBuffer) -> PaintAction {
        let ctx = &ui.ctx().clone();
        self.shell.frame(ctx);
        if let Some(p) = window::dropped_document(ctx) {
            self.open(p);
        }
        self.poll_loader();
        if let Some(doc) = &mut self.document {
            self.scene.poll(doc, self.viewport.gpu(), &*self.diag);
            if let Some(e) = self.scene.error.take() {
                self.diag.error(Area::Gpu, e);
            }
        }
        let mut events = self.shell.take_shortcuts(ctx);
        events.extend(self.draw(ui, history));
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

    fn draw(&mut self, ui: &mut egui::Ui, history: &RingBuffer) -> Vec<PanelEvent> {
        let screen = Screen {
            document: self.document.as_ref(),
            report: &mut self.report,
            viewport: &mut self.viewport,
            display: self.display,
            status: history.last().map(|(_, e)| e),
            activity: Activity { loading: self.loader.as_ref().map(|l| l.progress()), maps: self.scene.progress() },
            shell: &mut self.shell,
        };
        panels::layout::show(ui, screen)
    }

    fn apply(&mut self, ev: PanelEvent) -> PaintAction {
        let Some(ev) = self.shell.apply(ev, self.document.is_some()) else {
            return PaintAction::Stay;
        };
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
            PanelEvent::ShowChannel(view) => self.show_channel(view),
            PanelEvent::CycleChannel(step) => {
                let all = ChannelView::ALL;
                let at = all.iter().position(|(v, _)| *v == self.display).unwrap_or(0) as i64;
                let next = (at + i64::from(step)).rem_euclid(all.len() as i64) as usize;
                if let Some((view, _)) = all.get(next) {
                    self.show_channel(*view);
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
                self.diag.info(Area::Project, "Cancelled.");
            }
            // Workspace events were handled by the shell.
            _ => {}
        }
        PaintAction::Stay
    }

    fn show_channel(&mut self, view: ChannelView) {
        self.display = view;
        if let Some(doc) = &mut self.document {
            self.scene.remap(doc, view);
        }
    }

    /// The interface scale the user chose (applied to the whole editor at start-up).
    pub fn ui_scale(&self) -> f32 {
        self.shell.ui_scale()
    }

    /// Saves the workspace and settings (on exit).
    pub fn flush(&mut self) {
        self.shell.flush();
    }

    fn poll_loader(&mut self) {
        let Some(result) = self.loader.as_mut().and_then(|l| l.poll()) else {
            return;
        };
        let source = self.loader.take().map(|l| l.source().display().to_string()).unwrap_or_default();
        match result {
            Ok(loaded) => self.show(loaded),
            Err(e) => {
                self.diag.error(area_of_load(&source), format!("{source}: {e}"));
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
        self.diag.info(Area::Project, format!("Opened {}: {} texture sets, {} mesh parts.", doc.source().display(), d.texture_sets.len(), d.mesh.parts.len()));
        if let Some(report) = d.import_report.as_ref().filter(|_| doc.summary.is_some()) {
            // The import report panel keeps the details; the log gets its summary line (LG-12).
            self.diag.emit(diag_entries::import_summary(report, &doc.source().display().to_string(), doc.summary.as_deref()));
        }
        self.scene.rebuild(&mut doc, self.display);
        self.viewport.reset_camera();
        self.document = Some(doc);
    }

    fn close(&mut self) {
        self.scene.clear(self.viewport.gpu());
        self.document = None;
        self.diag.info(Area::Project, "Document closed.");
    }
}

/// Failed loads of Substance projects are import problems; anything else is a project problem.
fn area_of_load(source: &str) -> Area {
    if source.to_ascii_lowercase().ends_with(".spp") { Area::Import } else { Area::Project }
}
