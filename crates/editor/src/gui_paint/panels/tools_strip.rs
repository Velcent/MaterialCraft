//! The vertical tools strip (UI-01): one button per painting tool, greyed out with the reason
//! when the selection can't be painted (UI-02). Brush presets dragged from the tray can be let go on it.

use eframe::egui::Ui;

use super::super::shell::Shell;
use super::super::shell::tools::{self, Tool};
use super::drop_zones::{self, BrushZone};
use super::{PanelEvent, Strip};
use crate::gui_widgets::strip::{self, StripItem};

pub fn show(ui: &mut Ui, shell: &Shell, has_document: bool, paintable: bool, events: &mut Vec<PanelEvent>) {
    // a brush preset dragged from the tray loads here (AS-06)
    drop_zones::mark(ui.ctx(), BrushZone::ToolsStrip, ui.max_rect());
    let items: Vec<StripItem> = Tool::ALL
        .iter()
        .map(|t| StripItem { icon: t.icon(), label: t.label(), shortcut: Some(shell.shortcuts.label(t.action())), selected: shell.tools.current == *t })
        .collect();
    let out = strip::vertical(ui, &items, tools::unavailable(has_document, paintable));
    if let Some(tool) = out.clicked.and_then(|i| Tool::ALL.get(i)) {
        events.push(PanelEvent::SelectTool(*tool));
    }
    side_menu(out.background, Strip::Tools, shell, events);
}

/// The strip's context menu: move it to the other window edge.
pub fn side_menu(background: Option<eframe::egui::Response>, strip: Strip, shell: &Shell, events: &mut Vec<PanelEvent>) {
    use crate::gui_settings::Side;
    let Some(bg) = background else { return };
    bg.context_menu(|ui| {
        let (label, to) = match shell.strip_side(strip) {
            Side::Left => ("Move to the right edge", Side::Right),
            Side::Right => ("Move to the left edge", Side::Left),
        };
        if ui.button(label).clicked() {
            events.push(PanelEvent::MoveStrip(strip, to));
            ui.close();
        }
    });
}
