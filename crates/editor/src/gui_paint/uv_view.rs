//! [`UvView`]: the 2D (UV) view widget (V2-01…V2-09). The current texture set's parts are drawn flat in UV space
//! by `gpu3d::paint_uv` (the same maps as the 3D view); egui draws the overlays on top: the tint outside the 0–1
//! working area, its frame, the island outlines, the U/V marker, the zoom readout and the view mode dropdown with
//! the set's name. Navigation uses the same bindings as the 3D view (pan, zoom by drag or wheel at the pointer);
//! fit-to-view is a button in the corner. The stroke gesture itself is recorded by the viewport, which knows which
//! view it started in.

use super::channel_view::ChannelView;
use super::panels::{PanelEvent, channel_bar};
use super::shell::actions;
use super::uv_outlines::Outline;
use super::view2d::{UvMap, View2d};
use crate::gui_workspace::shortcuts::ShortcutTable;
use eframe::egui::{self, Align2, Color32, FontId, PointerButton, Rect, Sense, Stroke, UiBuilder};
use materialcraft::diag::Sink;
use materialcraft::gpu3d::{self, Antialiasing, Display, Shared};
use materialcraft::render3d::View;
use std::sync::Arc;

const BACKGROUND: Color32 = crate::gui_theme::HEADER_BG;
/// Laid over everything outside the 0–1 square (V2-05: islands there show greyed).
const OUTSIDE_TINT: Color32 = Color32::from_rgba_premultiplied(18, 18, 22, 150);
const FRAME: Color32 = Color32::from_gray(200);
const OUTLINE: Color32 = Color32::from_rgb(255, 170, 60);

/// What the 2D view shows this frame besides the GPU picture, set by the screen.
#[derive(Default, Clone)]
pub struct UvScene {
    /// The texture set's name and its size in texels.
    pub set: Option<(String, (u32, u32))>,
    pub outline: Option<Arc<Outline>>,
}

#[derive(Default)]
pub struct UvView {
    view: View2d,
    /// The view was fitted to its first layout.
    fitted: bool,
    rect: Option<Rect>,
    pub scene: UvScene,
}

impl UvView {
    /// Where the view was laid out last frame, in UI points.
    pub fn rect(&self) -> Option<Rect> {
        self.rect
    }

    /// The view's transforms for the tools, once laid out.
    pub fn map(&self) -> Option<UvMap> {
        let r = self.rect?;
        Some(UvMap { view: self.view, size: [r.width(), r.height()] })
    }

    /// Draws the view into `rect` and handles its navigation; returns its response (the viewport records strokes
    /// from it).
    #[allow(clippy::too_many_arguments)] // the view's inputs this frame: GPU, light, display, bindings and the mode
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        gpu: &Shared,
        diag: &Arc<dyn Sink>,
        view: &View,
        display: Display,
        mode: ChannelView,
        keys: &ShortcutTable,
        events: &mut Vec<PanelEvent>,
    ) -> egui::Response {
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        self.rect = (rect.width() > 0.0 && rect.height() > 0.0).then_some(rect);
        let size = [rect.width(), rect.height()];
        if !self.fitted && self.rect.is_some() {
            self.view = View2d::fit(size);
            self.fitted = true;
        }
        self.navigate(ui, &resp, rect, keys);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, BACKGROUND);
        let error = gpu.lock().ok().and_then(|g| g.error.clone());
        if let Some(e) = error {
            let msg = format!("The GPU viewer is unavailable: {e}");
            painter.text(rect.center(), Align2::CENTER_CENTER, msg, FontId::proportional(13.0), Color32::from_rgb(255, 170, 120));
            return resp;
        }
        if self.scene.set.is_some() {
            // crisp texels: no accumulation jitter in the 2D view
            let display = Display { antialiasing: Antialiasing::Off, ..display };
            let (gpu, view, sink, camera) = (gpu.clone(), view.clone(), diag.clone(), self.view.camera(size));
            let cb = eframe::egui_glow::CallbackFn::new(move |info, p| gpu3d::paint_uv(&gpu, &info, p, &view, &display, &camera, &*sink));
            painter.add(egui::PaintCallback { rect, callback: Arc::new(cb) });
        }
        self.overlays(&painter, rect);
        self.corner(ui, rect, mode, events);
        resp
    }

    fn navigate(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: Rect, keys: &ShortcutTable) {
        let size = [rect.width(), rect.height()];
        let mods = ui.input(|i| i.modifiers);
        let gesture = [PointerButton::Primary, PointerButton::Secondary, PointerButton::Middle]
            .into_iter()
            .find(|b| resp.dragged_by(*b))
            .and_then(|b| keys.drag(mods, b));
        let d = resp.drag_delta();
        match gesture {
            // orbiting has no meaning in the 2D view: it pans like the pan gesture
            Some(actions::PAN | actions::ORBIT | actions::ORBIT_SNAP) => self.view.pan([d.x, d.y]),
            Some(actions::ZOOM) => self.view.zoom_at((-d.y * 0.01).exp(), [size[0] * 0.5, size[1] * 0.5], size),
            _ => {}
        }
        if resp.hovered() && keys.wheel(mods) == Some(actions::ZOOM) {
            let scroll = ui.input(|i| crate::gui_input::raw_scroll_delta(i).y);
            if let (true, Some(p)) = (scroll != 0.0, resp.hover_pos()) {
                self.view.zoom_at((scroll * 0.0025).exp(), [p.x - rect.min.x, p.y - rect.min.y], size);
            }
        }
    }

    /// Tint outside the working area, its frame, the island outlines, the U/V marker and the zoom readout.
    fn overlays(&self, painter: &egui::Painter, rect: Rect) {
        let size = [rect.width(), rect.height()];
        let at = |uv: [f32; 2]| {
            let s = self.view.uv_to_screen(uv, size);
            egui::pos2(rect.min.x + s[0], rect.min.y + s[1])
        };
        let square = Rect::from_min_max(at([0.0, 0.0]), at([1.0, 1.0]));
        for outside in outside_of(rect, square) {
            painter.rect_filled(outside, 0.0, OUTSIDE_TINT);
        }
        painter.rect_stroke(square, 0.0, Stroke::new(1.0, FRAME), egui::StrokeKind::Middle);
        if let Some(outline) = &self.scene.outline {
            let stroke = Stroke::new(1.0, OUTLINE);
            let view = rect.expand(2.0);
            for [a, b] in &outline.edges {
                let (p, q) = (at(*a), at(*b));
                if view.intersects(Rect::from_two_pos(p, q)) {
                    painter.line_segment([p, q], stroke);
                }
            }
        }
        // the U/V marker (V2-03): u to the right, v down
        let o = rect.left_bottom() + egui::vec2(14.0, -44.0);
        let font = FontId::proportional(10.0);
        painter.arrow(o, egui::vec2(22.0, 0.0), Stroke::new(1.5, Color32::from_rgb(230, 90, 80)));
        painter.arrow(o, egui::vec2(0.0, 22.0), Stroke::new(1.5, Color32::from_rgb(110, 200, 110)));
        painter.text(o + egui::vec2(26.0, 0.0), Align2::LEFT_CENTER, "U", font.clone(), Color32::from_gray(200));
        painter.text(o + egui::vec2(0.0, 26.0), Align2::CENTER_TOP, "V", font.clone(), Color32::from_gray(200));
        if let Some((_, res)) = &self.scene.set {
            let zoom = self.view.points_per_texel(res.0.max(res.1)) * 100.0;
            let text = format!("{zoom:.0} %  ({}×{})", res.0, res.1);
            painter.text(rect.right_bottom() + egui::vec2(-8.0, -6.0), Align2::RIGHT_BOTTOM, text, font, Color32::from_gray(170));
        }
    }

    /// The view mode dropdown, the set's name and the fit button in the top-left corner (V2-02, DS-05).
    fn corner(&mut self, ui: &mut egui::Ui, rect: Rect, mode: ChannelView, events: &mut Vec<PanelEvent>) {
        let bar = Rect::from_min_size(rect.min + egui::vec2(6.0, 4.0), egui::vec2(rect.width() - 12.0, 24.0));
        let mut fit = false;
        ui.scope_builder(UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
            channel_bar::show_with_id(ui, "paint_uv_view_mode", mode, events);
            if let Some((name, _)) = &self.scene.set {
                ui.label(egui::RichText::new(name).strong().color(Color32::from_gray(220)));
            }
            fit = ui.small_button("Fit").on_hover_text("Show the whole 0–1 square").clicked();
        });
        if fit {
            self.view = View2d::fit([rect.width(), rect.height()]);
        }
    }
}

/// The parts of `rect` outside `inner` (up to four rectangles).
fn outside_of(rect: Rect, inner: Rect) -> Vec<Rect> {
    let i = inner.intersect(rect);
    if !i.is_positive() {
        return vec![rect];
    }
    [
        Rect::from_min_max(rect.min, egui::pos2(rect.max.x, i.min.y)),
        Rect::from_min_max(egui::pos2(rect.min.x, i.max.y), rect.max),
        Rect::from_min_max(egui::pos2(rect.min.x, i.min.y), egui::pos2(i.min.x, i.max.y)),
        Rect::from_min_max(egui::pos2(i.max.x, i.min.y), egui::pos2(rect.max.x, i.max.y)),
    ]
    .into_iter()
    .filter(|r| r.is_positive())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tint_covers_exactly_what_lies_outside_the_square() {
        let rect = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 80.0));
        let inner = Rect::from_min_max(egui::pos2(20.0, 10.0), egui::pos2(70.0, 60.0));
        let parts = outside_of(rect, inner);
        let area: f32 = parts.iter().map(|r| r.area()).sum();
        assert!((area - (100.0 * 80.0 - 50.0 * 50.0)).abs() < 1e-3);
        assert!(parts.iter().all(|r| !r.intersects(inner.shrink(0.01))));
        // zoomed in past the square: no tint; scrolled away from it: all tint
        assert!(outside_of(rect, rect.expand(10.0)).is_empty());
        assert_eq!(outside_of(rect, Rect::from_min_max(egui::pos2(200.0, 0.0), egui::pos2(300.0, 50.0))), vec![rect]);
    }
}
