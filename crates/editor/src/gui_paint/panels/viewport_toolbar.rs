//! The toolbar fixed to the top of the viewport (UI-04): the active tool's content on the left,
//! fixed view controls on the right; groups that don't fit go into the "more" menu (UI-05).

use eframe::egui::{self, Id, RichText, Ui};

use super::super::channel_view::ChannelView;
use super::super::shell::tools::{self, ToolState};
use super::super::tools::BrushTools;
use super::{PanelEvent, channel_bar, polygon_bar, quick_bar, quick_mask_bar};
use crate::gui_theme;
use crate::gui_widgets::toolbar::{self, Group, Side};
use crate::gui_workspace::shortcuts::ShortcutTable;
use materialcraft_brush::QuickMaskState;

#[allow(clippy::too_many_arguments)] // the toolbar shows a little of everything; a struct would only repeat these names
pub fn show(
    ui: &mut Ui,
    tool_state: &ToolState,
    brush: &mut BrushTools,
    quick_mask: QuickMaskState,
    keys: &ShortcutTable,
    display: ChannelView,
    orthographic: bool,
    has_document: bool,
    paintable: bool,
    events: &mut Vec<PanelEvent>,
) {
    let tool = tool_state.current;
    let unavailable = tools::unavailable(has_document, paintable);
    let reset_key = keys.label("paint.view.reset_camera");
    let projection_key = keys.label("paint.view.projection");
    let mut view_events = Vec::new();
    let mut camera_events = Vec::new();
    let mut mask_events = Vec::new();
    let groups = vec![
        Group::new(Side::Left, move |ui: &mut Ui| {
            ui.label(RichText::new(tool.label()).strong().color(gui_theme::HEADER_TEXT));
            if let Some(why) = unavailable {
                ui.label(RichText::new(why).color(gui_theme::ICON_DISABLED));
            }
        }),
        Group::new(Side::Left, |ui: &mut Ui| {
            if unavailable.is_some() {
                return;
            }
            if tool == tools::Tool::PolygonFill {
                polygon_bar::show(ui, &mut brush.polygon);
            } else if let Some((preset, helpers)) = brush.quick_mut(tool) {
                quick_bar::show(ui, preset, helpers);
            }
        }),
        Group::new(Side::Left, |ui: &mut Ui| {
            if unavailable.is_none() {
                quick_mask_bar::show(ui, quick_mask, &mut mask_events);
            }
        }),
        Group::new(Side::Right, |ui: &mut Ui| {
            let mut button = egui::Button::new("Reset camera");
            if !reset_key.is_empty() {
                button = button.shortcut_text(reset_key.as_str());
            }
            if ui.add_enabled(has_document, button).clicked() {
                camera_events.push(PanelEvent::ResetCamera);
            }
            let tip = if projection_key.is_empty() { "Orthographic projection".to_string() } else { format!("Orthographic projection ({projection_key})") };
            if ui.add_enabled(has_document, egui::Button::selectable(orthographic, "Ortho")).on_hover_text(tip).clicked() {
                camera_events.push(PanelEvent::ToggleProjection);
            }
        }),
        Group::new(Side::Right, |ui: &mut Ui| {
            ui.add_enabled_ui(has_document, |ui| channel_bar::show(ui, display, &mut view_events));
        }),
    ];
    toolbar::show(ui, Id::new("paint_viewport_toolbar"), groups);
    events.append(&mut view_events);
    events.append(&mut camera_events);
    events.append(&mut mask_events);
}
