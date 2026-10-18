//! The geometry mask icon of a layer row (GM-02, GM-03): a small hexagon, filled when the layer includes all of the
//! set's parts, half filled with the count of parts left when it includes some, hollow when it includes none.
//! Clicking it enters (or leaves) the geometry mask edit mode.

use crate::gui_theme;
use eframe::egui::{Align2, Color32, FontId, Id, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};
use materialcraft::paint::Inclusion;

/// Width the icon takes on the row's second line.
pub const WIDTH: f32 = 44.0;

const RADIUS: f32 = 5.0;
const NONE_COLOR: Color32 = Color32::from_rgb(220, 110, 90);

/// Draws the icon with its left edge at `left` (vertically centred on `centre_y`); returns its response.
pub fn show(ui: &mut Ui, id: Id, left: f32, centre_y: f32, inclusion: Inclusion, editing: bool) -> Response {
    let rect = Rect::from_min_size(pos2(left, centre_y - 7.0), vec2(WIDTH, 14.0));
    let painter = ui.painter_at(rect.expand(1.0));
    let centre = pos2(rect.min.x + RADIUS + 2.0, centre_y);
    let outline = hexagon(centre, RADIUS);
    let (colour, label) = match inclusion {
        Inclusion::All => (gui_theme::ICON, String::new()),
        Inclusion::Some { included, total } => (gui_theme::ACCENT, format!("{included}/{total}")),
        Inclusion::None => (NONE_COLOR, "0".to_owned()),
    };
    match inclusion {
        Inclusion::All => {
            painter.add(Shape::convex_polygon(outline.to_vec(), colour, Stroke::NONE));
        }
        Inclusion::Some { .. } => {
            // the left half filled
            let h = RADIUS * std::f32::consts::FRAC_PI_3.sin();
            let half = vec![pos2(centre.x, centre.y + h), outline[2], outline[3], outline[4], pos2(centre.x, centre.y - h)];
            painter.add(Shape::convex_polygon(half, colour, Stroke::NONE));
        }
        Inclusion::None => {}
    }
    painter.add(Shape::closed_line(outline.to_vec(), Stroke::new(1.2, colour)));
    if !label.is_empty() {
        painter.text(pos2(centre.x + RADIUS + 4.0, centre_y), Align2::LEFT_CENTER, label, FontId::proportional(10.5), colour);
    }
    if editing {
        painter.rect_stroke(rect, 2.0, Stroke::new(1.5, Color32::from_rgb(90, 170, 255)), StrokeKind::Inside);
    }
    let tip = match (editing, inclusion) {
        (true, _) => "Geometry mask: editing. Click to finish.".to_owned(),
        (false, Inclusion::All) => "Geometry mask: all parts. Click to choose parts.".to_owned(),
        (false, Inclusion::Some { included, total }) => format!("Geometry mask: {included} of {total} parts. Click to edit."),
        (false, Inclusion::None) => "Geometry mask: no part. Click to edit.".to_owned(),
    };
    ui.interact(rect, id, Sense::click()).on_hover_text(tip)
}

/// A hexagon with a flat top around `c`, corners from the right going clockwise on screen.
fn hexagon(c: Pos2, r: f32) -> [Pos2; 6] {
    std::array::from_fn(|k| {
        let a = std::f32::consts::FRAC_PI_3 * k as f32;
        pos2(c.x + r * a.cos(), c.y + r * a.sin())
    })
}
