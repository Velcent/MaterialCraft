//! The asset tray (roadmap step 15, AS-08…AS-20, LS-18/19, WS-15): browse the asset libraries, filter by usage
//! type and folder, search with the query syntax, keep searches, and drag assets into the layer stack.
//!
//! The tray changes nothing itself: it reports what the user asked for as [`TrayEvent`]s (an asset dropped at a
//! screen position, changed library settings), and the screen hosting it applies them: a drop through the layer
//! panel's drop target and the document history, settings through the settings store. It works without an open
//! document (AS-18).
//!
//! - [`service`]: the library, scanned and polled off the UI thread; [`roots`]: which libraries, first run;
//!   [`starter`]: the starter library, installed or built on first run;
//! - [`view`]: the filter (types, folder, query) and saved searches; [`results`]: what the filter matches;
//! - [`header`], [`folders`], [`grid`], [`settings_page`]: drawing; [`icons`]: usage icons;
//! - [`thumbs`]: thumbnails made off the UI thread and cached; [`drag`], [`payload`]: dragging into the stack;
//!   [`brush_item`]: what a brush preset or alpha does on the brush tools;
//! - [`manage`]: an asset's context menu (rename, edit, move to another library, delete), carried out by the asset
//!   store on the library thread.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod brush_item;
mod drag;
mod folders;
mod grid;
mod header;
pub mod icons;
pub mod manage;
pub mod payload;
pub mod results;
pub mod roots;
pub mod service;
mod settings_page;
pub mod starter;
pub mod thumbs;
pub mod view;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Pos2, Ui};
use materialcraft::diag::Sink;
use materialcraft::paint::edit::ops::DropPayload;

use crate::gui_settings::{LibrarySettings, SavedSearch, TrayLayout, TraySettings};
use brush_item::BrushDrop;
use drag::Drag;
use manage::{Manage, MoveTarget};
use results::{Item, Results};
use service::LibraryService;
use starter::StarterLibrary;
use thumbs::ThumbCache;
use view::Filter;

/// Narrower than this, an `Auto` tray uses the narrow layout.
const WIDE_FROM: f32 = 330.0;

/// What the tray asks of its host.
#[derive(Clone, Debug, PartialEq)]
pub enum TrayEvent {
    /// An asset was let go at screen position `pos` while dragged; `replace` is Ctrl (ADR 0008 q-effect-drop).
    Dropped { item: Box<Item>, payload: DropPayload, pos: Pos2, replace: bool },
    /// A picture (alpha or texture) was let go at screen position `pos`: it has no place on the layer stack, but can
    /// become the stencil or the projection image when let go over the 3D view (PT-15, PJ-26).
    DroppedImage { item: Box<Item>, pos: Pos2 },
    /// An environment map was let go at screen position `pos`: over the 3D view it lights the view (EN-01).
    DroppedEnvironment { item: Box<Item>, pos: Pos2 },
    /// A brush preset or an alpha was let go at screen position `pos`: over the tools strip or the brush panel it loads
    /// the preset or sets the tip (AS-06, PT-11); an alpha that is a picture (`image`) can still go onto the view.
    DroppedBrush { item: Box<Item>, brush: BrushDrop, pos: Pos2, image: bool },
    /// The further library folders changed (AS-18).
    SetRoots(Vec<PathBuf>),
    /// The saved searches changed (AS-16, AS-17).
    SetSavedSearches(Vec<SavedSearch>),
    /// The view choices changed (AS-09, AS-13).
    SetView(TraySettings),
}

/// The tray's state.
pub struct AssetTray {
    service: LibraryService,
    results: Results,
    thumbs: ThumbCache,
    filter: Filter,
    /// The user's library folder (none when settings live in memory).
    user: Option<PathBuf>,
    extra: Vec<PathBuf>,
    /// The read-only starter library (added once it is ready).
    starter: StarterLibrary,
    saved: Vec<SavedSearch>,
    view: TraySettings,
    /// The saved search whose tab is active.
    tab: Option<usize>,
    ui: header::HeaderState,
    folders: folders::FolderState,
    settings: settings_page::SettingsState,
    drag: Option<Drag>,
    /// The first asset in view last frame and where it was drawn.
    first_cell: Option<(usize, egui::Rect)>,
    /// The asset dialog open (rename, edit, delete).
    manage: Manage,
}

impl AssetTray {
    /// The tray over the libraries in `settings`; the user library lives in `settings_dir` (created with its
    /// standard layout on first run), the starter library is found or built there ([`starter`]). Also returns what to
    /// tell the user about the first run.
    pub fn new(settings: &LibrarySettings, settings_dir: Option<&Path>, sink: Arc<dyn Sink>) -> (AssetTray, Option<String>) {
        let user = roots::user_folder(settings_dir);
        let note = user.as_deref().and_then(roots::first_run);
        let starter = StarterLibrary::locate(settings_dir, sink.clone());
        let service = LibraryService::spawn(roots::roots(user.as_deref(), &settings.roots, starter.folder()), sink);
        let thumbs = ThumbCache::new(settings_dir.map(|d| d.join("cache").join("thumbnails")));
        let mut tray = AssetTray::with_service(service, thumbs, user, settings);
        tray.starter = starter;
        (tray, note)
    }

    /// The tray over an existing library service (tests).
    pub fn with_service(service: LibraryService, thumbs: ThumbCache, user: Option<PathBuf>, settings: &LibrarySettings) -> AssetTray {
        AssetTray {
            service,
            results: Results::default(),
            thumbs,
            filter: Filter::default(),
            user,
            extra: settings.roots.clone(),
            starter: StarterLibrary::none(),
            saved: settings.saved_searches.clone(),
            view: settings.tray.clone(),
            tab: None,
            ui: header::HeaderState::default(),
            folders: folders::FolderState::default(),
            settings: settings_page::SettingsState::default(),
            drag: None,
            first_cell: None,
            manage: Manage::default(),
        }
    }

    /// The user's own library folder, where saved smart materials and smart masks go (none when settings live in
    /// memory).
    pub fn user_library(&self) -> Option<&Path> {
        self.user.as_deref()
    }

    /// The folders material references are looked up in: every library root, in tray order.
    pub fn material_folders(&self) -> Vec<PathBuf> {
        self.service.roots().iter().filter_map(|r| r.path.clone()).collect()
    }

    /// The project template files (`.mctemplate`) of every library, in tray order, with their display names. Empty
    /// while the library is busy.
    pub fn template_files(&self) -> Vec<(String, PathBuf)> {
        let Some(library) = self.service.try_read() else { return Vec::new() };
        let catalogue = library.catalogue();
        let mut out = Vec::new();
        for root in catalogue.roots() {
            let Some(dir) = &root.path else { continue };
            let templates = catalogue.assets(&root.id).filter(|a| a.format == materialcraft::assets::Format::Template);
            out.extend(templates.map(|a| (a.name.clone(), dir.join(&a.path))));
        }
        out
    }

    /// The payload of the asset being dragged, if it can be dropped on the layer stack (for its landing preview).
    pub fn dragged_payload(&self) -> Option<&DropPayload> {
        self.drag.as_ref().and_then(|d| d.payload.as_ref().ok())
    }

    /// Draws the tray.
    pub fn show(&mut self, ui: &mut Ui, events: &mut Vec<TrayEvent>) {
        self.service.attach(ui.ctx());
        self.thumbs.begin(ui.ctx());
        let wide = match self.view.layout {
            TrayLayout::Wide => true,
            TrayLayout::Narrow => false,
            TrayLayout::Auto => ui.available_width() >= WIDE_FROM,
        };
        let mut h = header::Header { filter: &mut self.filter, saved: &mut self.saved, tab: &mut self.tab, view: &mut self.view, state: &mut self.ui, events };
        h.show(ui, wide);
        if self.ui.settings_open {
            let page = settings_page::Page { user: self.user.as_deref(), extra: &mut self.extra, view: &mut self.view, state: &mut self.settings, events };
            page.show(ui);
            self.apply_roots();
            return;
        }
        self.refresh();
        folders::breadcrumbs(ui, &self.results, &mut self.filter);
        ui.separator();
        let targets = self.move_targets();
        let mut picked = None;
        let mut body = |ui: &mut Ui, results: &Results, thumbs: &mut ThumbCache, drag: &mut Option<Drag>, first: &mut Option<(usize, egui::Rect)>, size| {
            let shown = grid::show(ui, results, thumbs, size, &targets);
            if let Some(d) = shown.started {
                *drag = Some(d);
            }
            *first = shown.first;
            if shown.menu.is_some() {
                picked = shown.menu;
            }
        };
        if wide {
            let area = ui.available_rect_before_wrap();
            let tree_width = (area.width() * 0.3).clamp(110.0, 200.0);
            let tree_rect = egui::Rect::from_min_size(area.min, egui::vec2(tree_width, area.height()));
            let grid_rect = egui::Rect::from_min_max(egui::pos2(tree_rect.max.x + 7.0, area.min.y), area.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(tree_rect), |ui| {
                folders::tree(ui, &self.results, &mut self.filter, &mut self.folders, self.view.hide_empty);
            });
            let x = tree_rect.max.x + 3.0;
            ui.painter().vline(x, area.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
            ui.scope_builder(egui::UiBuilder::new().max_rect(grid_rect), |ui| {
                body(ui, &self.results, &mut self.thumbs, &mut self.drag, &mut self.first_cell, self.view.thumbnails)
            });
            ui.allocate_rect(area, egui::Sense::hover());
        } else {
            egui::CollapsingHeader::new("Folders").id_salt("asset_tray_folders").default_open(false).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("asset_tray_tree_narrow").max_height(160.0).show(ui, |ui| {
                    folders::tree(ui, &self.results, &mut self.filter, &mut self.folders, self.view.hide_empty);
                });
            });
            body(ui, &self.results, &mut self.thumbs, &mut self.drag, &mut self.first_cell, self.view.thumbnails);
        }
        if let Some((item, choice)) = picked
            && let Some(request) = self.manage.choose(item, choice)
        {
            self.service.store(request);
        }
        if let Some(request) = self.manage.show(ui.ctx()) {
            self.service.store(request);
        }
    }

    /// The libraries assets can be moved into: every writable library folder.
    fn move_targets(&self) -> Vec<MoveTarget> {
        self.service.roots().iter().filter(|r| !r.is_read_only() && r.path.is_some()).map(|r| MoveTarget { root: r.id.clone(), name: r.name.clone() }).collect()
    }

    /// Ends a drag when the pointer is let go (anywhere on screen) and draws the dragged asset under the pointer.
    /// Call once per frame after every panel was drawn, whether the tray is visible or not.
    pub fn end_frame(&mut self, ctx: &egui::Context) -> Option<TrayEvent> {
        if self.starter.poll() {
            self.apply_roots();
        }
        let drag = self.drag.as_ref()?;
        let (released, down, pos, ctrl, escape) =
            ctx.input(|i| (i.pointer.any_released(), i.pointer.primary_down(), i.pointer.latest_pos(), i.modifiers.command, i.key_pressed(egui::Key::Escape)));
        if escape {
            self.drag = None;
            return None;
        }
        if released || !down {
            let drag = self.drag.take()?;
            let pos = pos?;
            let (image, environment) = (drag.is_view_image(), drag.is_environment());
            return match (drag.payload, drag.brush) {
                (Ok(payload), _) => Some(TrayEvent::Dropped { item: Box::new(drag.item), payload, pos, replace: ctrl }),
                (Err(_), Ok(brush)) => Some(TrayEvent::DroppedBrush { item: Box::new(drag.item), brush, pos, image }),
                (Err(_), Err(_)) if image => Some(TrayEvent::DroppedImage { item: Box::new(drag.item), pos }),
                (Err(_), Err(_)) if environment => Some(TrayEvent::DroppedEnvironment { item: Box::new(drag.item), pos }),
                (Err(_), Err(_)) => None,
            };
        }
        drag.paint_ghost(ctx, pos);
        ctx.request_repaint();
        None
    }

    /// Brings the results up to date with the filter and the library, in this frame if the library isn't being
    /// rescanned right now (else the last results stay until it is done).
    fn refresh(&mut self) {
        let search = self.filter.search();
        let generation = self.service.generation();
        if self.results.is_current(generation, &search) {
            return;
        }
        if let Some(lib) = self.service.try_read() {
            self.results.refresh(&lib, generation, &search);
        }
    }

    /// Hands changed library folders to the service.
    fn apply_roots(&mut self) {
        let wanted = roots::roots(self.user.as_deref(), &self.extra, self.starter.folder());
        self.service.set_roots(wanted);
    }
}
