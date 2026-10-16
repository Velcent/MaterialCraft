//! [`Viewport`]: the 3D view of the Paint screen. Camera input works like the node editor's 3D view
//! (drag: orbit, Alt+drag or middle: pan, Shift+drag: light, wheel: zoom); drawing goes through the
//! shared GPU renderer in an egui paint callback.

use super::channel_view::ChannelView;
use eframe::egui::{self, Align2, Color32, FontId, PointerButton, Sense};
use materialcraft::gpu3d::{self, Shared};
use materialcraft::render3d::{View, RELIEF_NONE};
use std::sync::Arc;

const BACKGROUND: Color32 = Color32::from_rgb(30, 30, 30);
const HINT: &str = "drag: orbit   Alt+drag / middle: move   Shift+drag: light   wheel: zoom";

pub struct Viewport {
    gpu: Shared,
    /// Camera, light and relief settings.
    view: View,
}

impl Viewport {
    pub fn new() -> Viewport {
        // Painter shows height through the normal map; displacement stays opt-in.
        Viewport {
            gpu: gpu3d::new_shared(),
            view: View {
                relief: RELIEF_NONE,
                ..View::default()
            },
        }
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

    /// Fills the remaining space of `ui` with the view.
    pub fn ui(&mut self, ui: &mut egui::Ui, display: ChannelView) {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, BACKGROUND);
        self.camera_input(ui, &resp);
        let error = self.gpu.lock().ok().and_then(|g| g.error.clone());
        match error {
            None => {
                let (gpu, view) = (self.gpu.clone(), display.view_for(&self.view));
                let cb = eframe::egui_glow::CallbackFn::new(move |info, painter| {
                    gpu3d::paint(&gpu, &info, painter, &view)
                });
                ui.painter().add(egui::PaintCallback {
                    rect,
                    callback: Arc::new(cb),
                });
            }
            Some(e) => {
                let msg = format!("The GPU viewer is unavailable: {e}");
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    msg,
                    FontId::proportional(13.0),
                    Color32::from_rgb(255, 170, 120),
                );
            }
        }
        let corner = rect.left_bottom() + egui::vec2(6.0, -5.0);
        ui.painter().text(
            corner,
            Align2::LEFT_BOTTOM,
            HINT,
            FontId::proportional(10.0),
            Color32::from_gray(110),
        );
    }

    fn camera_input(&mut self, ui: &egui::Ui, resp: &egui::Response) {
        let v = &mut self.view;
        if resp.dragged() {
            let d = resp.drag_delta();
            let (alt, shift) = ui.input(|i| (i.modifiers.alt, i.modifiers.shift));
            if alt || resp.dragged_by(PointerButton::Middle) {
                let k = 0.004 * v.dist / 3.4;
                v.pan_x = (v.pan_x - d.x * k).clamp(-3.0, 3.0);
                v.pan_y = (v.pan_y + d.y * k).clamp(-3.0, 3.0);
            } else if shift {
                v.light_yaw -= d.x * 0.01;
                v.light_pitch = (v.light_pitch + d.y * 0.01).clamp(-1.4, 1.4);
            } else {
                v.yaw -= d.x * 0.01;
                v.pitch = (v.pitch + d.y * 0.01).clamp(-1.4, 1.4);
            }
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                v.dist = (v.dist * (1.0 - scroll * 0.001)).clamp(1.6, 9.0);
            }
        }
    }
}
