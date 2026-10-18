//! [`Viewport`]: the 3D view of the Paint screen. Navigation follows the shortcut table (by
//! default Alt+left orbit, Alt+Shift+left orbit snapping to axis views, Alt+middle pan,
//! Alt+right or the wheel zoom, Shift+right turns the light; ADR 0008 mc2-vq12); drawing goes
//! through the shared GPU renderer in an egui paint callback with the [`ViewState`]'s display settings, and keeps
//! painting while a still view accumulates. The axis gizmo sits in the lower-right corner.

use super::axis_gizmo;
use super::channel_view::ChannelView;
use super::input::stylus::StylusFeed;
use super::input::{PointerLog, StrokeInput};
use super::navigation;
use super::shell::actions;
use super::tools::screen_image::gizmo::GizmoAction;
use super::view_state::ViewState;
use crate::gui_settings::PenSettings;
use crate::gui_workspace::shortcuts::ShortcutTable;
use eframe::egui::{self, Align2, Color32, FontId, PointerButton, Sense};
use materialcraft::camera::{Camera, Projection};
use materialcraft::diag::Sink;
use materialcraft::gpu3d::{self, Shared};
use materialcraft::render3d::View;
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
        Viewport { gpu, state, snap: None, diag, rect: None, pointer: PointerLog::default(), hover: None, layer: None, image_key: false, image_drag: None }
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

    /// The renderer's options (height self-shadowing, anisotropic filtering).
    pub fn opts(&self) -> gpu3d::GpuOpts {
        self.gpu.lock().map(|g| g.opts.clone()).unwrap_or_default()
    }

    /// True while a still view is accumulating more frames (screenshots wait for it).
    pub fn refining(&self) -> bool {
        self.gpu.lock().is_ok_and(|g| g.refining())
    }

    /// Frames drawn so far.
    pub fn frames(&self) -> u64 {
        self.gpu.lock().map_or(0, |g| g.frames)
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

    /// True while the stencil key is held (the gizmo is shown).
    pub fn image_key(&self) -> bool {
        self.image_key
    }

    /// The pointer over the view this frame.
    pub fn hover(&self) -> Option<Hover> {
        self.hover
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

    /// Switches between perspective and orthographic projection (the framing of the pivot stays).
    pub fn toggle_projection(&mut self) {
        let p = &mut self.state.display.camera.projection;
        *p = match *p {
            Projection::Perspective => Projection::Orthographic,
            Projection::Orthographic => Projection::Perspective,
        };
    }

    pub fn reset_camera(&mut self) {
        let d = View::default();
        self.state.view.set_orbit(&d.orbit());
        (self.state.view.light_yaw, self.state.view.light_pitch) = (d.light_yaw, d.light_pitch);
    }

    /// Fills the remaining space of `ui` with the view; `keys` decides the navigation gestures.
    pub fn ui(&mut self, ui: &mut egui::Ui, display: ChannelView, keys: &ShortcutTable) {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.rect = (rect.width() > 0.0 && rect.height() > 0.0).then_some(rect);
        self.layer = Some(ui.layer_id());
        ui.painter().rect_filled(rect, 0.0, BACKGROUND);
        let image_key = ui.input(|i| keys.held_shifted(i, actions::STENCIL));
        self.image_key = image_key.is_some();
        self.image_drag = image_key.and_then(|snap| image_drag(ui, &resp, rect, snap));
        if !self.image_key {
            self.camera_input(ui, &resp, keys);
        }
        self.stroke_input(ui, &resp, rect, keys);
        let error = self.gpu.lock().ok().and_then(|g| g.error.clone());
        match error {
            None => {
                let (gpu, view, sink) = (self.gpu.clone(), self.state.view.clone(), self.diag.clone());
                let shown = self.state.display_for(display);
                let cb = eframe::egui_glow::CallbackFn::new(move |info, painter| gpu3d::paint_with(&gpu, &info, painter, &view, &shown, &*sink));
                ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(cb) });
                if self.refining() {
                    ui.ctx().request_repaint();
                }
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
        ui.painter().text(corner, Align2::LEFT_BOTTOM, hint(keys), FontId::proportional(10.0), Color32::from_gray(110));
    }

    /// Records the stroke gesture (by default a left drag, or a click for a single stamp; with Shift a straight
    /// line).
    fn stroke_input(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect, keys: &ShortcutTable) {
        let mods = ui.input(|i| i.modifiers);
        let gesture = keys.drag(mods, PointerButton::Primary);
        let line = match gesture {
            Some(actions::STROKE_LINE) => Some(false),
            Some(actions::STROKE_LINE_SNAP) => Some(true),
            _ => None,
        };
        let hover = resp.hover_pos().filter(|p| rect.contains(*p));
        self.hover = hover.map(|p| Hover { pos: [p.x - rect.min.x, p.y - rect.min.y], line });
        // with the stencil key held, drags edit the stencil or projection image
        let is_stroke = (gesture == Some(actions::STROKE) || line.is_some()) && !self.image_key;
        let held = is_stroke && resp.dragged_by(PointerButton::Primary);
        let clicked = is_stroke && resp.clicked_by(PointerButton::Primary);
        let started = (is_stroke && resp.drag_started_by(PointerButton::Primary)) || clicked;
        let stopped = resp.drag_stopped_by(PointerButton::Primary) || clicked;
        ui.input(|i| self.pointer.record(i, rect, held, started, stopped, line));
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
        let v = &mut self.state.view;
        let d = resp.drag_delta();
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
