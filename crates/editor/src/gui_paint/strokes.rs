//! The brush tools' part of a Paint-screen frame: the viewport's stroke gesture goes to [`tools::Painting`], what
//! it painted is recomposited, and the tools' visual aids are drawn over the views. A gesture from the 2D view goes
//! to the tools with the view's transforms; the 3D-only picks (geometry mask parts, the material picker) ask for
//! the 3D view.

use super::PaintMode;
use super::input::StrokeInput;
use super::panels::PanelEvent;
use super::quick_mask_tint::QuickMaskTint;
use super::shell::tools::Tool;
use super::tools::{PaintFrame, PaintReport};
use super::viewport::StrokeView;
use eframe::egui;
use materialcraft::diag::{Area, SinkExt};

impl PaintMode {
    /// Runs this frame's strokes. Call after the panels were drawn (the viewport recorded the gesture).
    pub(super) fn paint_strokes(&mut self, ctx: &egui::Context) {
        let input = self.viewport.take_stroke_input();
        let uv = match self.viewport.stroke_view() {
            StrokeView::Uv => self.viewport.uv_map(),
            StrokeView::Mesh => None,
        };
        let input = if uv.is_some() {
            self.only_2d_tools(input)
        } else {
            let input = self.geometry_pick(input);
            self.material_pick(input)
        };
        self.screen_image_input(ctx);
        self.projection_gizmo(ctx);
        let Some(doc) = self.document.as_mut() else {
            self.painting.forget_document();
            return;
        };
        let target = self.layers.paint_target(doc.doc());
        let frame = PaintFrame { doc, picking: &self.picking, camera: self.viewport.camera(), tool: self.shell.tools.current, target, uv };
        let report = self.painting.frame(input, frame);
        self.after_painting(report);
        self.draw_screen_images(ctx);
        if let (Some(doc), Some(painter), Some(rect)) = (self.document.as_ref(), self.viewport.overlay_painter(ctx), self.viewport.rect()) {
            let overlay = self.painting.overlay(doc, &self.picking, self.viewport.camera(), self.viewport.hover(), self.shell.tools.current);
            overlay.draw(&painter, rect.min);
        }
        self.draw_uv_overlay(ctx);
        if self.painting.is_stroking() || self.viewport.stroking() {
            ctx.request_repaint();
        }
    }

    /// A 2D-view gesture for the tools, unless a pick that needs the 3D view is under way (then it is dropped with a
    /// note).
    fn only_2d_tools(&mut self, input: Vec<StrokeInput>) -> Vec<StrokeInput> {
        let three_d = if self.layers.geometry_editing.is_some() {
            Some("Pick the parts of a geometry mask in the 3D view.")
        } else if self.shell.tools.current == Tool::MaterialPicker {
            Some("The material picker reads the 3D view; pick there.")
        } else {
            None
        };
        match three_d {
            Some(why) => {
                if input.iter().any(|ev| matches!(ev, StrokeInput::Down(_))) {
                    self.diag.info(Area::General, why);
                }
                Vec::new()
            }
            None => input,
        }
    }

    /// The tools' aids over the 2D view.
    fn draw_uv_overlay(&self, ctx: &egui::Context) {
        let (Some(doc), Some(map), Some(rect)) = (self.document.as_ref(), self.viewport.uv_map(), self.viewport.uv_rect()) else { return };
        let Some(set) = self.layers.current_set(doc.doc()) else { return };
        let Some(painter) = self.viewport.uv_overlay_painter(ctx) else { return };
        let overlay = self.painting.uv_overlay(map, set.resolution, self.viewport.uv_hover(), self.shell.tools.current);
        overlay.draw(&painter, rect.min);
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
            self.scene.painted(doc, self.display, set, report.tiles);
        }
    }
}
