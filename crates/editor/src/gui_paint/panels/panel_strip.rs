//! The vertical panel strip (UI-03): one button per panel. A click shows the panel as a popover
//! next to its button, a second click hides it; a closed panel keeps its button.

use eframe::egui::{Rect, Ui};

use super::super::shell::Shell;
use super::super::shell::actions;
use super::super::shell::panels::PaintPanel;
use super::{PanelEvent, Strip};
use crate::gui_settings::Side;
use crate::gui_widgets::strip::{self, StripItem};
use crate::gui_workspace::PanelKind;

/// Size of a popover when it opens.
const POPOVER: [f32; 2] = [320.0, 420.0];

pub fn show(ui: &mut Ui, shell: &Shell, events: &mut Vec<PanelEvent>) {
    let panels: Vec<PaintPanel> = PaintPanel::ALL.iter().copied().filter(|p| !p.is_canvas()).collect();
    let items: Vec<StripItem> = panels
        .iter()
        .map(|p| StripItem {
            icon: p.icon(),
            label: p.title(),
            shortcut: Some(shell.shortcuts.label(actions::toggle_action(*p))),
            selected: shell.workspace.is_open(*p),
        })
        .collect();
    let out = strip::vertical(ui, &items, None);
    if let Some((panel, button)) = out.clicked.and_then(|i| Some((*panels.get(i)?, *out.rects.get(i)?))) {
        let screen = ui.ctx().content_rect();
        events.push(PanelEvent::Popover(panel, popover_rect(button, shell.strip_side(Strip::Panels), screen)));
    }
    super::tools_strip::side_menu(out.background, Strip::Panels, shell, events);
}

/// Where a popover opens: beside its button, towards the window's inside, kept on screen.
fn popover_rect(button: Rect, side: Side, screen: Rect) -> [f32; 4] {
    let [w, h] = POPOVER;
    let x = match side {
        Side::Right => button.left() - 6.0 - w,
        Side::Left => button.right() + 6.0,
    };
    let x = x.clamp(screen.left(), (screen.right() - w).max(screen.left()));
    let y = button.top().clamp(screen.top(), (screen.bottom() - h).max(screen.top()));
    [x, y, w, h.min(screen.height())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    #[test]
    fn popovers_open_inward_and_stay_on_screen() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 800.0));
        let right = Rect::from_min_size(pos2(1160.0, 700.0), vec2(30.0, 30.0));
        let [x, y, w, h] = popover_rect(right, Side::Right, screen);
        assert!(x + w <= right.left() && y + h <= screen.bottom());
        let left = Rect::from_min_size(pos2(4.0, 40.0), vec2(30.0, 30.0));
        assert!(popover_rect(left, Side::Left, screen)[0] >= left.right());
    }
}
