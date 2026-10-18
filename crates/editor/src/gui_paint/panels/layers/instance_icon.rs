//! The instance badge of a layer row (IN-09), drawn on the content thumbnail's top-right corner: a filled diamond on a
//! source, two linked rings on an instance, a red cross on a broken instance. Normal layers have none.

use crate::gui_theme;
use eframe::egui::{Color32, Id, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};
use materialcraft::paint::instance::InstanceRole;

const SIZE: f32 = 12.0;
const LINK: Color32 = Color32::from_rgb(90, 170, 255);
const BROKEN: Color32 = Color32::from_rgb(220, 70, 60);

/// Draws the badge for `role` in the top-right corner of `thumb`; `None` for a normal layer.
pub fn show(ui: &mut Ui, id: Id, thumb: Rect, role: InstanceRole) -> Option<Response> {
    let tip = match role {
        InstanceRole::Normal => return None,
        InstanceRole::Source => "Source: other layers are instances of it and follow its changes",
        InstanceRole::Instance => "Instance: takes everything from its source; edit the source to change it",
        InstanceRole::Broken => "Broken instance: its source is gone or it sits inside the folder it instances. It shows nothing.",
    };
    let rect = Rect::from_min_size(pos2(thumb.max.x - SIZE + 2.0, thumb.min.y - 2.0), vec2(SIZE, SIZE));
    let painter = ui.painter_at(rect.expand(1.0));
    let c = rect.center();
    let r = SIZE / 2.0 - 1.0;
    painter.circle_filled(c, r + 0.5, gui_theme::PANEL_FILL);
    match role {
        InstanceRole::Source => {
            let diamond = vec![pos2(c.x, c.y - r), pos2(c.x + r, c.y), pos2(c.x, c.y + r), pos2(c.x - r, c.y)];
            painter.add(Shape::convex_polygon(diamond, gui_theme::ACCENT, Stroke::NONE));
        }
        InstanceRole::Instance => {
            painter.circle_stroke(pos2(c.x - 2.0, c.y), 2.6, Stroke::new(1.3, LINK));
            painter.circle_stroke(pos2(c.x + 2.0, c.y), 2.6, Stroke::new(1.3, LINK));
        }
        InstanceRole::Broken => {
            let k = r * 0.7;
            painter.line_segment([pos2(c.x - k, c.y - k), pos2(c.x + k, c.y + k)], Stroke::new(1.6, BROKEN));
            painter.line_segment([pos2(c.x - k, c.y + k), pos2(c.x + k, c.y - k)], Stroke::new(1.6, BROKEN));
        }
        InstanceRole::Normal => {}
    }
    Some(ui.interact(rect, id, Sense::hover()).on_hover_text(tip))
}
