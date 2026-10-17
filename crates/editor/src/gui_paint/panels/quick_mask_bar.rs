//! The quick mask's quick-access controls (PT-42, PT-43; ADR 0008 mc2-q-pt-07c): its state, on/off, invert while
//! editing, and clear (a separate action, so the toggle never loses the mask).

use super::super::tools::ToolEvent;
use super::PanelEvent;
use eframe::egui::Ui;
use materialcraft_brush::QuickMaskState;

pub fn show(ui: &mut Ui, state: QuickMaskState, events: &mut Vec<PanelEvent>) {
    let (label, tip) = match state {
        QuickMaskState::Off => ("Quick mask", "Edit the quick mask: the paint tool protects, the eraser frees"),
        QuickMaskState::Editing => ("Editing quick mask", "Switch the quick mask off (it is kept); the previous-tool key applies it"),
        QuickMaskState::Applied => ("Quick mask on", "Switch the quick mask off (it is kept)"),
    };
    if ui.selectable_label(state != QuickMaskState::Off, label).on_hover_text(tip).clicked() {
        events.push(PanelEvent::Tool(ToolEvent::QuickMaskToggle));
    }
    if ui.add_enabled(state == QuickMaskState::Editing, eframe::egui::Button::new("Invert")).on_hover_text("Swap protected and free").clicked() {
        events.push(PanelEvent::Tool(ToolEvent::QuickMaskInvert));
    }
    if ui.button("Clear").on_hover_text("Empty the quick mask of this texture set").clicked() {
        events.push(PanelEvent::Tool(ToolEvent::QuickMaskClear));
    }
}
