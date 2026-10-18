//! [`Viewport`]: the views of the Paint screen: the 3D view and the 2D (UV) view ([`UvView`]), shown alone or side
//! by side ([`ViewLayout`]). A stroke gesture belongs to the view it started in ([`StrokeView`]). In the 3D view,
//! navigation follows the shortcut table (by
//! default Alt+left orbit, Alt+Shift+left orbit snapping to axis views, Alt+middle pan,
//! Alt+right or the wheel zoom, Shift+right turns the light; ADR 0008 mc2-vq12); drawing goes
//! through the shared GPU renderer in an egui paint callback with the [`ViewState`]'s display settings, and keeps
//! painting while a still view accumulates. The axis gizmo sits in the lower-right corner. The environment the view
//! names is prepared by its [`EnvironmentLoader`].

use super::axis_gizmo;
use super::channel_view::ChannelView;
use super::environment::{EnvironmentLoader, EnvironmentStatus};
use super::input::stylus::StylusFeed;
use super::input::{PointerLog, StrokeInput};
use super::navigation;
use super::panels::PanelEvent;
use super::projection_edit::{GizmoPointer, Phase};
use super::shell::actions;
use super::tools::screen_image::gizmo::GizmoAction;
use super::uv_view::UvView;
use super::view_state::ViewLayout;
use super::view_state::ViewState;
use super::view2d::UvMap;
use crate::gui_settings::PenSettings;
use crate::gui_workspace::shortcuts::ShortcutTable;
use eframe::egui::{self, Align2, Color32, FontId, PointerButton, Sense};
use materialcraft::camera::{Camera, Projection};
use materialcraft::diag::Sink;
use materialcraft::gpu3d::{self, Shared};
use materialcraft::render3d::View;
use std::path::PathBuf;
use std::sync::Arc;

const BACKGROUND: Color32 = crate::gui_theme::HEADER_BG;

pub struct Viewport {
    gpu: Shared,
    /// Camera, light, relief and display settings.
    state: ViewState,
    /// The unsnapped yaw and pitch during a snapping orbit.
    snap: Option<(f32, f32)>,
    /// Where the renderer reports shader and GL errors.
    diag: Arc<dyn Sink>,
    /// Where the view was laid out last frame (UI points).
    rect: Option<egui::Rect>,
    /// The stroke gesture's input, for the brush tools.
    pointer: PointerLog,
    /// Where the pointer hovers over the view this frame (view coordinates) and the stroke kind its modifiers
    /// would start, for the brush overlays.
    hover: Option<Hover>,
    /// The layer the view is drawn on, so overlays drawn later in the frame land on top of it.
    layer: Option<egui::LayerId>,
    /// The stencil key is held this frame: drags edit the stencil or projection image instead of painting.
    image_key: bool,
    /// This frame's drag of the image gizmo.
    image_drag: Option<ImageDrag>,
    /// The pointer is over a part of the projection gizmo (set by the screen from last frame's hover): a primary
    /// press grabs it instead of painting.
    gizmo_hover: bool,
    /// A projection gizmo drag holds the primary button.
    gizmo_active: bool,
    /// This frame's projection gizmo pointer event.
    gizmo_pointer: Option<GizmoPointer>,
    environment: EnvironmentLoader,
    /// The 2D view.
    uv: UvView,
    /// The view the stroke gesture in progress (or the last one) started in.
    stroke_view: StrokeView,
    /// Where the pointer hovers over the 2D view this frame (view coordinates).
    uv_hover: Option<Hover>,
}

/// Which view a stroke gesture comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StrokeView {
    #[default]
    Mesh,
    Uv,
}

/// A drag of the stencil's or projection image's gizmo (the stencil key held).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageDrag {
    pub action: GizmoAction,
    /// The pointer (view coordinates); where the drag started when `started`.
    pub at: [f32; 2],
    pub started: bool,
    /// Turn in 90° steps (Shift held with the stencil key).
    pub snap: bool,
}

/// The pointer over the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover {
    /// View coordinates (from the view's top-left corner).
    pub pos: [f32; 2],
    /// `Some(snap)` while the modifiers held would start a straight line.
    pub line: Option<bool>,
}

impl Viewport {
    /// A viewport starting with `state` and the GPU options `opts`.
    pub fn new(diag: Arc<dyn Sink>, state: ViewState, opts: gpu3d::GpuOpts) -> Viewport {
        let gpu = gpu3d::new_shared();
        if let Ok(mut g) = gpu.lock() {
            g.opts = opts;
        }
        Viewport {
            gpu,
            state,
            snap: None,
            diag,
            rect: None,
            pointer: PointerLog::default(),
            hover: None,
            layer: None,
            image_key: false,
            image_drag: None,
            gizmo_hover: false,
            gizmo_active: false,
            gizmo_pointer: None,
            environment: EnvironmentLoader::default(),
            uv: UvView::default(),
            stroke_view: StrokeView::Mesh,
            uv_hover: None,
        }
    }

    /// The 2D view (the screen sets what it shows each frame).
    pub fn uv_mut(&mut self) -> &mut UvView {
        &mut self.uv
    }

    /// The 2D view's transforms this frame, if it is shown.
    pub fn uv_map(&self) -> Option<UvMap> {
        self.uv.map()
    }

    /// Where the 2D view was laid out last frame, in UI points.
    pub fn uv_rect(&self) -> Option<egui::Rect> {
        self.uv.rect()
    }

    /// The pointer over the 2D view this frame.
    pub fn uv_hover(&self) -> Option<Hover> {
        self.uv_hover
    }

    /// The view the stroke input taken last came from.
    pub fn stroke_view(&self) -> StrokeView {
        self.stroke_view
    }

    pub fn gpu(&self) -> &Shared {
        &self.gpu
    }

    /// The view and display settings together with the renderer (for its options), for the settings panel.
    pub fn settings_mut(&mut self) -> (&mut ViewState, &Shared) {
        (&mut self.state, &self.gpu)
    }

    pub fn state(&self) -> &ViewState {
        &self.state
    }

    /// Lights the view with the environment map at library path `map` (`None`: the built-in studio).
    pub fn set_environment_map(&mut self, map: Option<String>) {
        self.state.set_environment_map(map);
    }

    /// Prepares the environment the view names when it changed; `roots` are the library folders maps are looked
    /// up in. Returns true while a map is being prepared.
    pub fn sync_environment(&mut self, roots: &[PathBuf]) -> bool {
        self.environment.sync(self.state.environment_map(), roots, &self.gpu, &*self.diag);
        self.environment.is_busy()
    }

    /// What lights the view.
    pub fn environment_status(&self) -> &EnvironmentStatus {
        self.environment.status()
    }

    /// The renderer's options (height self-shadowing, anisotropic filtering).
    pub fn opts(&self) -> gpu3d::GpuOpts {
        self.gpu.lock().map(|g| g.opts.clone()).unwrap_or_default()
    }

    /// True while a still view is accumulating more frames (screenshots wait for it).
    pub fn refining(&self) -> bool {
        self.gpu.lock().is_ok_and(|g| g.refining())
    }

    /// Frames drawn so far by either view.
    pub fn frames(&self) -> u64 {
        self.gpu.lock().map_or(0, |g| g.frames + g.uv.frames)
    }

    /// The camera of the last drawn frame on a viewport in UI points (screen positions relative to
    /// [`Viewport::rect`]'s top-left corner), or `None` before the view was laid out.
    pub fn camera(&self) -> Option<Camera> {
        let r = self.rect?;
        Some(self.state.view.camera(&self.state.display.camera, [r.width(), r.height()]))
    }

    /// Where the view was laid out last frame, in UI points.
    pub fn rect(&self) -> Option<egui::Rect> {
        self.rect
    }

    /// The stroke gesture's input since the last call (positions relative to the view's top-left corner).
    pub fn take_stroke_input(&mut self) -> Vec<StrokeInput> {
        self.pointer.take()
    }

    /// This frame's gizmo drag of the stencil or projection image, if any.
    pub fn image_drag(&self) -> Option<ImageDrag> {
        self.image_drag
    }

    /// This frame's projection gizmo pointer event, if any.
    pub fn gizmo_pointer(&self) -> Option<GizmoPointer> {
        self.gizmo_pointer
    }

    /// Whether the pointer is over the projection gizmo (a press then grabs it).
    pub fn set_gizmo_hover(&mut self, over: bool) {
        self.gizmo_hover = over;
    }

    /// True while the stencil key is held (the gizmo is shown).
    pub fn image_key(&self) -> bool {
        self.image_key
    }

    /// The pointer over the view this frame.
    pub fn hover(&self) -> Option<Hover> {
        self.hover
    }

    /// A painter over the 2D view, for overlays drawn after it this frame.
    pub fn uv_overlay_painter(&self, ctx: &egui::Context) -> Option<egui::Painter> {
        Some(ctx.layer_painter(self.layer?).with_clip_rect(self.uv.rect()?))
    }

    /// A painter over the view, for overlays drawn after it this frame.
    pub fn overlay_painter(&self, ctx: &egui::Context) -> Option<egui::Painter> {
        Some(ctx.layer_painter(self.layer?).with_clip_rect(self.rect?))
    }

    /// Applies the user's pen settings to the stroke input (call each frame; cheap when unchanged).
    pub fn set_pen(&mut self, pen: &PenSettings) {
        self.pointer.configure(pen);
    }

    /// Where the platform's pen reader writes its samples.
    pub fn pen_feed(&self) -> StylusFeed {
        self.pointer.feed()
    }

    /// True while the stroke gesture is held.
    pub fn stroking(&self) -> bool {
        self.pointer.is_down()
    }

    /// Switches between perspective and orthographic projection (the framing of the pivot stays); not on a locked
    /// camera.
    pub fn toggle_projection(&mut self) {
        if self.state.active_locked() {
            return;
        }
        let p = &mut self.state.display.camera.projection;
        *p = match *p {
            Projection::Perspective => Projection::Orthographic,
            Projection::Orthographic => Projection::Perspective,
        };
    }

    /// Puts the default camera back to its start view (another active camera first hands over to it, CM-06).
    pub fn reset_camera(&mut self) {
        self.state.before_navigation();
        let d = View::default();
        self.state.view.set_orbit(&d.orbit());
        (self.state.view.light_yaw, self.state.view.light_pitch) = (d.light_yaw, d.light_pitch);
    }

    /// Fills the remaining space of `ui` with the views of the chosen layout; `keys` decides the navigation
    /// gestures. The 2D view's corner dropdown adds its events to `events`.
    pub fn ui(&mut self, ui: &mut egui::Ui, display: ChannelView, keys: &ShortcutTable, events: &mut Vec<PanelEvent>) {
        let (full, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let (mesh, uv) = split(full, self.state.layout);
        self.layer = Some(ui.layer_id());
        let mesh = mesh.map(|rect| (rect, self.mesh_ui(ui, rect, display, keys)));
        if mesh.is_none() {
            (self.rect, self.hover, self.image_drag, self.gizmo_pointer) = (None, None, None, None);
        }
        let shown = self.state.display_for(display);
        let uv = uv.map(|rect| (rect, self.uv.ui(ui, rect, &self.gpu, &self.diag, &self.state.view, shown, display, keys, events)));
        if uv.is_none() {
            self.uv_hover = None;
        }
        // the gesture belongs to the view it started in; a new one to the view under the pointer
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let over_uv = uv.as_ref().is_some_and(|(r, _)| pointer.is_some_and(|p| r.contains(p)));
        let want = if self.pointer.is_down() {
            self.stroke_view
        } else if over_uv || mesh.is_none() {
            StrokeView::Uv
        } else {
            StrokeView::Mesh
        };
        let chosen = match want {
            StrokeView::Uv => uv.as_ref().map(|v| (StrokeView::Uv, v)).or(mesh.as_ref().map(|v| (StrokeView::Mesh, v))),
            StrokeView::Mesh => mesh.as_ref().map(|v| (StrokeView::Mesh, v)).or(uv.as_ref().map(|v| (StrokeView::Uv, v))),
        };
        if let Some((view, (rect, resp))) = chosen {
            self.stroke_view = view;
            let hover = self.stroke_input(ui, resp, *rect, keys, view == StrokeView::Mesh);
            match view {
                StrokeView::Mesh => (self.hover, self.uv_hover) = (hover, None),
                StrokeView::Uv => (self.hover, self.uv_hover) = (None, hover),
            }
        }
        if self.refining() {
            ui.ctx().request_repaint();
        }
    }

    /// The 3D view in `rect`: navigation, the stencil and projection gizmos, the GPU picture and its corner aids.
    fn mesh_ui(&mut self, ui: &mut egui::Ui, rect: egui::Rect, display: ChannelView, keys: &ShortcutTable) -> egui::Response {
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        self.rect = (rect.width() > 0.0 && rect.height() > 0.0).then_some(rect);
        ui.painter().rect_filled(rect, 0.0, BACKGROUND);
        let image_key = ui.input(|i| keys.held_shifted(i, actions::STENCIL));
        self.image_key = image_key.is_some();
        self.image_drag = image_key.and_then(|snap| image_drag(ui, &resp, rect, snap));
        self.gizmo_pointer = if self.image_key { None } else { self.gizmo_input(ui, &resp, rect) };
        if !self.image_key {
            self.camera_input(ui, &resp, keys);
        }
        let error = self.gpu.lock().ok().and_then(|g| g.error.clone());
        match error {
            None => {
                let (gpu, view, sink) = (self.gpu.clone(), self.state.view.clone(), self.diag.clone());
                let shown = self.state.display_for(display);
                let cb = eframe::egui_glow::CallbackFn::new(move |info, painter| gpu3d::paint_with(&gpu, &info, painter, &view, &shown, &*sink));
                ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(cb) });
                if let Some(camera) = self.camera() {
                    axis_gizmo::draw(ui.painter(), rect, &camera);
                }
            }
            Some(e) => {
                let msg = format!("The GPU viewer is unavailable: {e}");
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, msg, FontId::proportional(13.0), Color32::from_rgb(255, 170, 120));
            }
        }
        let corner = rect.left_bottom() + egui::vec2(6.0, -5.0);
        ui.painter_at(rect).text(corner, Align2::LEFT_BOTTOM, hint(keys), FontId::proportional(10.0), Color32::from_gray(110));
        resp
    }

    /// Records the stroke gesture of the view at `rect` (by default a left drag, or a click for a single stamp;
    /// with Shift a straight line); returns where the pointer hovers over it. The stencil key and the projection
    /// gizmo only take the gesture over the 3D view (`mesh`).
    fn stroke_input(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect, keys: &ShortcutTable, mesh: bool) -> Option<Hover> {
        let mods = ui.input(|i| i.modifiers);
        let gesture = keys.drag(mods, PointerButton::Primary);
        let line = match gesture {
            Some(actions::STROKE_LINE) => Some(false),
            Some(actions::STROKE_LINE_SNAP) => Some(true),
            _ => None,
        };
        let hover = resp.hover_pos().filter(|p| rect.contains(*p)).map(|p| Hover { pos: [p.x - rect.min.x, p.y - rect.min.y], line });
        // with the stencil key held, drags edit the stencil or projection image
        // on the projection gizmo, presses grab it instead
        let on_gizmo = mesh && (self.gizmo_active || (self.gizmo_hover && !mods.alt));
        let is_stroke = (gesture == Some(actions::STROKE) || line.is_some()) && !(mesh && self.image_key) && !on_gizmo;
        let held = is_stroke && resp.dragged_by(PointerButton::Primary);
        let clicked = is_stroke && resp.clicked_by(PointerButton::Primary);
        let started = (is_stroke && resp.drag_started_by(PointerButton::Primary)) || clicked;
        let stopped = resp.drag_stopped_by(PointerButton::Primary) || clicked;
        ui.input(|i| self.pointer.record(i, rect, held, started, stopped, line));
        hover
    }

    fn camera_input(&mut self, ui: &egui::Ui, resp: &egui::Response, keys: &ShortcutTable) {
        let mods = ui.input(|i| i.modifiers);
        let gesture = if resp.dragged() {
            [PointerButton::Primary, PointerButton::Secondary, PointerButton::Middle, PointerButton::Extra1, PointerButton::Extra2]
                .into_iter()
                .find(|b| resp.dragged_by(*b))
                .and_then(|b| keys.drag(mods, b))
        } else {
            None
        };
        let d = resp.drag_delta();
        let moves_camera = matches!(gesture, Some(actions::ORBIT | actions::ORBIT_SNAP | actions::PAN | actions::ZOOM));
        let wheel = resp.hovered() && keys.wheel(mods) == Some(actions::ZOOM) && ui.input(|i| crate::gui_input::raw_scroll_delta(i).y) != 0.0;
        if (moves_camera && d != egui::Vec2::ZERO) || wheel {
            // imported cameras never move: the default camera takes over their view first (CM-06)
            self.state.before_navigation();
        }
        let v = &mut self.state.view;
        match gesture {
            Some(actions::ORBIT) => navigation::orbit(v, d.x, d.y),
            Some(actions::ORBIT_SNAP) => {
                let raw = self.snap.get_or_insert((v.yaw, v.pitch));
                navigation::orbit_snapped(v, raw, d.x, d.y);
            }
            Some(actions::PAN) => navigation::pan(v, d.x, d.y),
            Some(actions::ZOOM) => navigation::zoom(v, -d.y * 5.0),
            Some(actions::LIGHT) => navigation::light(v, d.x, d.y),
            _ => {}
        }
        if gesture != Some(actions::ORBIT_SNAP) {
            self.snap = None;
        }
        if resp.hovered() && keys.wheel(mods) == Some(actions::ZOOM) {
            let scroll = ui.input(|i| crate::gui_input::raw_scroll_delta(i).y);
            if scroll != 0.0 {
                navigation::zoom(&mut self.state.view, scroll);
            }
        }
    }
}

impl Viewport {
    /// The projection gizmo's pointer: a primary press over it (without Alt, which navigates) grabs it; the drag
    /// and its release follow. Ctrl steps, Shift snaps onto the surface or locks an axis.
    fn gizmo_input(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect) -> Option<GizmoPointer> {
        let (mods, press, now) = ui.input(|i| (i.modifiers, i.pointer.press_origin(), i.pointer.interact_pos()));
        let at = |p: egui::Pos2| [p.x - rect.min.x, p.y - rect.min.y];
        let pointer = |p: Option<egui::Pos2>, phase| GizmoPointer { at: p.map_or([f32::NAN; 2], at), phase, step: mods.command, shift: mods.shift };
        if resp.drag_started_by(PointerButton::Primary) && self.gizmo_hover && !mods.alt {
            self.gizmo_active = true;
            return Some(pointer(press.or(now), Phase::Press));
        }
        if !self.gizmo_active {
            return None;
        }
        if resp.drag_stopped_by(PointerButton::Primary) || !resp.dragged_by(PointerButton::Primary) {
            self.gizmo_active = false;
            return Some(pointer(now, Phase::Release));
        }
        Some(pointer(now, Phase::Drag))
    }
}

/// The gizmo drag under way with the stencil key held: left turns, right scales, middle moves.
fn image_drag(ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect, snap: bool) -> Option<ImageDrag> {
    let (button, action) =
        [(PointerButton::Primary, GizmoAction::Turn), (PointerButton::Secondary, GizmoAction::Scale), (PointerButton::Middle, GizmoAction::Move)]
            .into_iter()
            .find(|(b, _)| resp.dragged_by(*b))?;
    let started = resp.drag_started_by(button);
    let pos = ui.input(|i| if started { i.pointer.press_origin() } else { None }.or(i.pointer.interact_pos()))?;
    Some(ImageDrag { action, at: [pos.x - rect.min.x, pos.y - rect.min.y], started, snap })
}

/// The navigation hint in the view's corner, from the current bindings.
fn hint(keys: &ShortcutTable) -> String {
    [(actions::ORBIT, "orbit"), (actions::PAN, "pan"), (actions::ZOOM, "zoom"), (actions::LIGHT, "light")]
        .iter()
        .filter_map(|(id, what)| {
            let k = keys.label(id);
            (!k.is_empty()).then(|| format!("{k}: {what}"))
        })
        .collect::<Vec<_>>()
        .join("   ")
}

/// The 3D view's and the 2D view's rectangles in `full` for `layout` (side by side: 3D left, 2D right).
fn split(full: egui::Rect, layout: ViewLayout) -> (Option<egui::Rect>, Option<egui::Rect>) {
    match layout {
        ViewLayout::Mesh => (Some(full), None),
        ViewLayout::Uv => (None, Some(full)),
        ViewLayout::Both => {
            let mid = full.center().x;
            let left = egui::Rect::from_min_max(full.min, egui::pos2(mid - 1.0, full.max.y));
            let right = egui::Rect::from_min_max(egui::pos2(mid + 1.0, full.min.y), full.max);
            (Some(left), Some(right))
        }
    }
}
