//! The brush tools' part of a Paint-screen frame: the viewport's stroke gesture goes to [`tools::Painting`], what
//! it painted is recomposited, and the tools' visual aids are drawn over the view.

use super::PaintMode;
use super::panels::PanelEvent;
use super::quick_mask_tint::QuickMaskTint;
use super::tools::{PaintFrame, PaintReport};
use eframe::egui;
use materialcraft::diag::{Area, SinkExt};

impl PaintMode {
    /// Runs this frame's strokes. Call after the panels were drawn (the viewport recorded the gesture).
    pub(super) fn paint_strokes(&mut self, ctx: &egui::Context) {
        let input = self.viewport.take_stroke_input();
        let input = self.geometry_pick(input);
        let input = self.material_pick(input);
        self.screen_image_input(ctx);
        self.projection_gizmo(ctx);
        let Some(doc) = self.document.as_mut() else {
            self.painting.forget_document();
            return;
        };
        let target = self.layers.paint_target(doc.doc());
        let frame = PaintFrame { doc, picking: &self.picking, camera: self.viewport.camera(), tool: self.shell.tools.current, target };
        let report = self.painting.frame(input, frame);
        self.after_painting(report);
        self.draw_screen_images(ctx);
        if let (Some(doc), Some(painter), Some(rect)) = (self.document.as_ref(), self.viewport.overlay_painter(ctx), self.viewport.rect()) {
            let overlay = self.painting.overlay(doc, &self.picking, self.viewport.camera(), self.viewport.hover(), self.shell.tools.current);
            overlay.draw(&painter, rect.min);
        }
        if self.painting.is_stroking() || self.viewport.stroking() {
            ctx.request_repaint();
        }
    }

    /// Ends a stroke in progress before `ev` changes the document under it.
    pub(super) fn end_stroke_before(&mut self, ev: &PanelEvent) {
        let changes_document = matches!(
            ev,
            PanelEvent::Undo
                | PanelEvent::Redo
                | PanelEvent::JumpHistory(_)
                | PanelEvent::Layers(_)
                | PanelEvent::LayerAction(_)
                | PanelEvent::Close
                | PanelEvent::Save
                | PanelEvent::SaveAs
                | PanelEvent::SetVisible(..)
                | PanelEvent::Tool(_)
                | PanelEvent::PreviousTool
        );
        if !(changes_document && self.painting.is_stroking()) {
            return;
        }
        let Some(doc) = self.document.as_mut() else { return };
        let report = self.painting.finish_stroke(doc, &self.picking);
        self.after_painting(report);
    }

    pub(super) fn after_painting(&mut self, report: PaintReport) {
        if let Some(msg) = report.message {
            self.diag.warn(Area::General, msg);
        }
        if report.quick_mask
            && let Some(doc) = &self.document
        {
            self.scene.set_tint(QuickMaskTint::of(&self.painting.quick_mask, doc.doc()));
        }
        if let (Some(set), Some(doc)) = (report.refresh, self.document.as_mut()) {
            self.scene.refresh(doc, self.display, vec![set]);
        }
    }
}
