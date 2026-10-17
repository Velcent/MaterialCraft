//! The docking chrome in MaterialCraft's theme: dark panels with clear borders (WS-14).

use craft_ui::docking::DockStyle;
use eframe::egui::{Stroke, Ui};

use crate::gui_theme;

pub fn dock_style(ui: &Ui) -> DockStyle {
    let mut s = DockStyle::from_ui(ui);
    s.tab_height = 24.0;
    s.gap = 3.0;
    s.min_pane = 60.0;
    s.background = gui_theme::PANEL_FILL;
    s.tab_background = gui_theme::HEADER_BG;
    s.active_background = gui_theme::PANEL_FILL;
    s.text = gui_theme::HEADER_TEXT;
    s.inactive_text = gui_theme::LABEL_TEXT.gamma_multiply(0.8);
    s.border = Stroke::new(1.0, gui_theme::BORDER);
    s.accent = gui_theme::ACCENT;
    s
}
