//! The toolbar fixed to the top of the viewport (UI-04): the active tool's content on the left,
//! fixed view controls on the right; groups that don't fit go into the "more" menu (UI-05).

use eframe::egui::{self, Id, RichText, Ui};

use super::super::channel_view::ChannelView;
use super::super::shell::tools::{self, ToolState};
use super::{PanelEvent, channel_bar};
use crate::gui_theme;
use crate::gui_widgets::toolbar::{self, Group, Side};
use crate::gui_workspace::shortcuts::ShortcutTable;

pub fn show(
    ui: &mut Ui,
    tool_state: &ToolState,
    keys: &ShortcutTable,
    display: ChannelView,
    has_document: bool,
    paintable: bool,
    events: &mut Vec<PanelEvent>,
) {
    let tool = tool_state.current;
    let unavailable = tools::unavailable(has_document, paintable);
    let reset_key = keys.label("paint.view.reset_camera");
    let mut view_events = Vec::new();
    let mut camera_events = Vec::new();
    let groups = vec![
        Group::new(Side::Left, move |ui: &mut Ui| {
            ui.label(RichText::new(tool.label()).strong().color(gui_theme::HEADER_TEXT));
            if let Some(why) = unavailable {
                ui.label(RichText::new(why).color(gui_theme::ICON_DISABLED));
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
        }),
        Group::new(Side::Right, |ui: &mut Ui| {
            ui.add_enabled_ui(has_document, |ui| channel_bar::show(ui, display, &mut view_events));
        }),
    ];
    toolbar::show(ui, Id::new("paint_viewport_toolbar"), groups);
    events.append(&mut view_events);
    events.append(&mut camera_events);
}
