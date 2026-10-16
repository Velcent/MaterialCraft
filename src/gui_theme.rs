//! Look shared by the node editor and the Paint screen: panel headers and their colours.
//! Lives in its own module so the Paint screen does not reach into the editor.

use eframe::egui::{self, vec2, Align2, Color32, FontId, Id, Rect, Sense, Stroke};

/// Background of panel headers, status and view areas.
pub const HEADER_BG: Color32 = Color32::from_rgb(30, 30, 30);
/// Text of panel headers.
pub const HEADER_TEXT: Color32 = Color32::from_gray(225);
/// The orange used for the active tab, selection and the "PAINT" mark.
pub const ACCENT: Color32 = Color32::from_rgb(235, 130, 20);

/// A panel title bar; with `closable`, draws a close cross and returns true when it was clicked.
pub fn panel_header(ui: &mut egui::Ui, title: &str, closable: bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, HEADER_BG);
    ui.painter().text(rect.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(12.0), HEADER_TEXT);
    let mut clicked = false;
    if closable {
        let cr = Rect::from_center_size(rect.right_center() - vec2(12.0, 0.0), vec2(16.0, 16.0));
        let resp = ui.interact(cr, Id::new(("hdr_close", title)), Sense::click());
        let col = if resp.hovered() { Color32::WHITE } else { Color32::from_gray(150) };
        let c = cr.center();
        ui.painter().line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], Stroke::new(1.3_f32, col));
        ui.painter().line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)], Stroke::new(1.3_f32, col));
        clicked = resp.clicked();
    }
    clicked
}
