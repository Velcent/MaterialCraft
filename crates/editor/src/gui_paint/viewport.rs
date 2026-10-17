//! [`Viewport`]: the 3D view of the Paint screen. Navigation follows the shortcut table (by
//! default Alt+left orbit, Alt+Shift+left orbit snapping to axis views, Alt+middle pan,
//! Alt+right or the wheel zoom, Shift+right turns the light; ADR 0008 mc2-vq12); drawing goes
//! through the shared GPU renderer in an egui paint callback.

use super::channel_view::ChannelView;
use super::navigation;
use super::shell::actions;
use crate::gui_workspace::shortcuts::ShortcutTable;
use eframe::egui::{self, Align2, Color32, FontId, PointerButton, Sense};
use materialcraft::diag::Sink;
use materialcraft::gpu3d::{self, Shared};
use materialcraft::render3d::{RELIEF_NONE, View};
use std::sync::Arc;

const BACKGROUND: Color32 = crate::gui_theme::HEADER_BG;

pub struct Viewport {
    gpu: Shared,
    /// Camera, light and relief settings.
    view: View,
    /// The unsnapped yaw and pitch during a snapping orbit.
    snap: Option<(f32, f32)>,
    /// Where the renderer reports shader and GL errors.
    diag: Arc<dyn Sink>,
}

impl Viewport {
    pub fn new(diag: Arc<dyn Sink>) -> Viewport {
        // Painter shows height through the normal map; displacement stays opt-in.
        Viewport { gpu: gpu3d::new_shared(), view: View { relief: RELIEF_NONE, ..View::default() }, snap: None, diag }
    }

    pub fn gpu(&self) -> &Shared {
        &self.gpu
    }

    /// The view settings together with the renderer (for its options), for the settings panel.
    pub fn settings_mut(&mut self) -> (&mut View, &Shared) {
        (&mut self.view, &self.gpu)
    }

    /// Frames drawn so far.
    pub fn frames(&self) -> u64 {
        self.gpu.lock().map_or(0, |g| g.frames)
    }

    pub fn reset_camera(&mut self) {
        let d = View::default();
        (self.view.yaw, self.view.pitch, self.view.dist) = (d.yaw, d.pitch, d.dist);
        (self.view.pan_x, self.view.pan_y) = (d.pan_x, d.pan_y);
        (self.view.light_yaw, self.view.light_pitch) = (d.light_yaw, d.light_pitch);
    }

    /// Fills the remaining space of `ui` with the view; `keys` decides the navigation gestures.
    pub fn ui(&mut self, ui: &mut egui::Ui, display: ChannelView, keys: &ShortcutTable) {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, BACKGROUND);
        self.camera_input(ui, &resp, keys);
        let error = self.gpu.lock().ok().and_then(|g| g.error.clone());
        match error {
            None => {
                let (gpu, view, sink) = (self.gpu.clone(), display.view_for(&self.view), self.diag.clone());
                let cb = eframe::egui_glow::CallbackFn::new(move |info, painter| gpu3d::paint(&gpu, &info, painter, &view, &*sink));
                ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(cb) });
            }
            Some(e) => {
                let msg = format!("The GPU viewer is unavailable: {e}");
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, msg, FontId::proportional(13.0), Color32::from_rgb(255, 170, 120));
            }
        }
        let corner = rect.left_bottom() + egui::vec2(6.0, -5.0);
        ui.painter().text(corner, Align2::LEFT_BOTTOM, hint(keys), FontId::proportional(10.0), Color32::from_gray(110));
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
        let v = &mut self.view;
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
                navigation::zoom(&mut self.view, scroll);
            }
        }
    }
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
