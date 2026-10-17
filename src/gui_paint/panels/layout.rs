//! Arranges the panels of the Paint screen: menu and status bars, texture sets and import report on
//! the left, view settings on the right, display modes and the viewport in the middle.

use super::super::channel_view::ChannelView;
use super::super::document_state::OpenDocument;
use super::super::viewport::Viewport;
use super::import_report::ReportPanel;
use super::status_bar::{Activity, Status};
use super::{PANEL_FILL, PanelEvent, channel_bar, menu_bar, status_bar, texture_sets, view_settings};
use eframe::egui;

/// Everything one frame of the screen shows.
pub struct Screen<'a> {
    pub document: Option<&'a OpenDocument>,
    pub report: &'a mut ReportPanel,
    pub viewport: &'a mut Viewport,
    pub display: ChannelView,
    pub status: &'a Status,
    pub activity: Activity<'a>,
}

/// Draws the screen and returns what the user asked for.
pub fn show(ctx: &egui::Context, s: Screen) -> Vec<PanelEvent> {
    let mut events = Vec::new();
    let title = s.document.map(|d| d.name()).unwrap_or_default();
    menu_bar::show(ctx, &title, s.document.is_some(), &mut events);
    status_bar::show(ctx, s.status, &s.activity, &mut events);
    let side = egui::Frame::none().fill(PANEL_FILL).inner_margin(egui::Margin::same(4.0));
    if let Some(doc) = s.document {
        egui::SidePanel::left("paint_left").frame(side).default_width(300.0).show(ctx, |ui| {
            texture_sets::show(ui, doc, &mut events);
            ui.add_space(8.0);
            s.report.show(ui, doc.doc().import_report.as_ref(), doc.summary.as_deref());
        });
        egui::SidePanel::right("paint_right").frame(side).default_width(250.0).show(ctx, |ui| {
            let (view, gpu) = s.viewport.settings_mut();
            view_settings::show(ui, view, gpu);
        });
    }
    egui::CentralPanel::default().frame(egui::Frame::none().fill(PANEL_FILL)).show(ctx, |ui| {
        if s.document.is_some() {
            channel_bar::show(ui, s.display, &mut events);
            s.viewport.ui(ui, s.display);
        } else {
            empty_state(ui, &mut events);
        }
    });
    events
}

/// Shown when no document is open.
fn empty_state(ui: &mut egui::Ui, events: &mut Vec<PanelEvent>) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.35);
        ui.label(egui::RichText::new("Paint mode").size(19.0).strong().color(egui::Color32::from_gray(235)));
        ui.add_space(8.0);
        ui.label("View a MaterialCraft paint document or a Substance 3D Painter project.");
        ui.add_space(12.0);
        if ui.button("Open in Paint mode...").clicked() {
            events.push(PanelEvent::Open);
        }
        if ui.button("Import Substance Painter project...").clicked() {
            events.push(PanelEvent::Import);
        }
    });
}
