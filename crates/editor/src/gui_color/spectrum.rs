//! The picker's spectrum (CP-02): a saturation × value field for the current hue with a hue strip beside it. Both
//! work on display-encoded HSV; dragging sets the components under the pointer.

use eframe::egui::{Color32, Rect, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use super::color::{rgb_of, srgb_color32};
use super::painters::grid_mesh;
use crate::gui_theme;

const STRIP_W: f32 = 18.0;
const GAP: f32 = 8.0;
/// Grid cells per side of the field (smooth enough, cheap enough to draw every frame).
const CELLS: usize = 24;

/// Draws the spectrum filling the width; returns true if `hsv` changed.
pub fn show(ui: &mut Ui, hsv: &mut [f32; 3]) -> bool {
    let width = (ui.available_width() - STRIP_W - GAP).max(80.0);
    let height = (width * 0.7).clamp(100.0, 260.0);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        changed |= field(ui, hsv, vec2(width, height));
        changed |= hue_strip(ui, hsv, height);
    });
    changed
}

/// The saturation (across) × value (up) field.
fn field(ui: &mut Ui, hsv: &mut [f32; 3], size: eframe::egui::Vec2) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let hue = hsv[0];
    ui.painter().add(grid_mesh(rect, CELLS, |x, y| srgb_color32(rgb_of([hue, x, 1.0 - y]))));
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, gui_theme::BORDER), StrokeKind::Outside);
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on()) {
        let (s, v) = (fraction(p.x, rect.left(), rect.width()), 1.0 - fraction(p.y, rect.top(), rect.height()));
        if s != hsv[1] || v != hsv[2] {
            (hsv[1], hsv[2]) = (s, v);
            changed = true;
        }
    }
    let marker = pos2(rect.left() + hsv[1] * rect.width(), rect.top() + (1.0 - hsv[2]) * rect.height());
    let ink = if hsv[2] > 0.6 && hsv[1] < 0.4 { Color32::BLACK } else { Color32::WHITE };
    ui.painter().with_clip_rect(rect.expand(6.0)).circle_stroke(marker, 5.0, Stroke::new(1.5, ink));
    changed
}

/// The hue strip: 0° at the top to 360° at the bottom.
fn hue_strip(ui: &mut Ui, hsv: &mut [f32; 3], height: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(STRIP_W, height), Sense::click_and_drag());
    ui.painter().add(grid_mesh(rect, CELLS, |_, y| srgb_color32(rgb_of([y, 1.0, 1.0]))));
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, gui_theme::BORDER), StrokeKind::Outside);
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on()) {
        let h = fraction(p.y, rect.top(), rect.height());
        if h != hsv[0] {
            hsv[0] = h;
            changed = true;
        }
    }
    marker_arrows(ui, rect, rect.top() + hsv[0] * rect.height());
    changed
}

/// Two small triangles pointing at `y` from both sides of `strip`.
fn marker_arrows(ui: &Ui, strip: Rect, y: f32) {
    for (x, dir) in [(strip.left() - 1.0, 1.0f32), (strip.right() + 1.0, -1.0)] {
        let tri = vec![pos2(x, y), pos2(x - 5.0 * dir, y - 4.0), pos2(x - 5.0 * dir, y + 4.0)];
        ui.painter().add(Shape::convex_polygon(tri, gui_theme::HEADER_TEXT, Stroke::NONE));
    }
}

/// Where `x` lies between `start` and `start + len`, clamped to 0…1.
fn fraction(x: f32, start: f32, len: f32) -> f32 {
    if len > 0.0 { ((x - start) / len).clamp(0.0, 1.0) } else { 0.0 }
}
