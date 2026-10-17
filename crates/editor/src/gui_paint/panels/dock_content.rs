//! What each docked panel shows: the bridge from the workspace's panel ids to the Paint
//! screen's panels.

use craft_ui::docking::DockContent;
use eframe::egui::{self, Ui};

use super::super::channel_view::ChannelView;
use super::super::document_state::OpenDocument;
use super::super::shell::panels::PaintPanel;
use super::super::shell::tools::ToolState;
use super::super::viewport::Viewport;
use super::import_report::ReportPanel;
use super::{PanelEvent, interface_settings, texture_sets, view_settings, viewport_toolbar};
use crate::gui_workspace::shortcuts::{ShortcutTable, ShortcutsPage};

/// Everything the panel bodies need for one frame.
pub struct Bodies<'a> {
    pub document: Option<&'a OpenDocument>,
    pub report: &'a mut ReportPanel,
    pub viewport: &'a mut Viewport,
    pub display: ChannelView,
    pub shortcuts: &'a ShortcutTable,
    pub page: &'a mut ShortcutsPage,
    pub tools: &'a ToolState,
    pub scale_draft: &'a mut f32,
    pub events: &'a mut Vec<PanelEvent>,
}

impl DockContent<PaintPanel> for Bodies<'_> {
    fn body(&mut self, ui: &mut Ui, panel: &PaintPanel) {
        let margin = egui::Frame::NONE.inner_margin(egui::Margin::same(6));
        match panel {
            PaintPanel::Viewport => self.viewport_body(ui),
            PaintPanel::TextureSets => {
                margin.show(ui, |ui| match self.document {
                    Some(doc) => texture_sets::show(ui, doc, self.events),
                    None => no_document(ui),
                });
            }
            PaintPanel::ImportReport => {
                margin.show(ui, |ui| match self.document {
                    Some(doc) => self.report.show(ui, doc.doc().import_report.as_ref(), doc.summary.as_deref()),
                    None => no_document(ui),
                });
            }
            PaintPanel::Display => {
                margin.show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("paint_display").show(ui, |ui| {
                        let (view, gpu) = self.viewport.settings_mut();
                        view_settings::show(ui, view, gpu);
                    })
                });
            }
            PaintPanel::Shortcuts => {
                margin.show(ui, |ui| {
                    if let Some(edit) = self.page.show(ui, self.shortcuts) {
                        self.events.push(PanelEvent::Shortcut(edit));
                    }
                });
            }
            PaintPanel::Interface => {
                margin.show(ui, |ui| interface_settings::show(ui, self.scale_draft, self.events));
            }
        }
    }
}

impl Bodies<'_> {
    fn viewport_body(&mut self, ui: &mut Ui) {
        let has_document = self.document.is_some();
        viewport_toolbar::show(ui, self.tools, self.shortcuts, self.display, has_document, self.events);
        if has_document {
            self.viewport.ui(ui, self.display, self.shortcuts);
        } else {
            super::layout::empty_state(ui, self.events);
        }
    }
}

fn no_document(ui: &mut Ui) {
    ui.weak("No document is open.");
}
