//! The edit mode inside `PaintMode`: each frame the excluded parts are handed to the viewport and the picking scene
//! (hidden and skipped while the mode is on, shown again after), viewport clicks pick parts instead of painting, and
//! the window's changes become layer panel events.

use super::{MaskTarget, window};
use crate::gui_paint::PaintMode;
use crate::gui_paint::input::StrokeInput;
use crate::gui_paint::panels::PanelEvent;
use eframe::egui;
use materialcraft::diag::{Area, SinkExt};

impl PaintMode {
    /// Ends the mode when its layer is gone and hides what the mask excludes (nothing outside the mode). Call once per
    /// frame, after the picking scene was polled.
    pub(in crate::gui_paint) fn sync_geometry_editing(&mut self) {
        let doc = self.document.as_ref().map(|d| d.doc());
        let r = self.layers.geometry_editing.as_ref();
        let hidden = match doc.zip(r).map(|(doc, r)| MaskTarget::find(doc, r)) {
            Some(Some(target)) => target.hidden_parts(),
            Some(None) => {
                self.layers.geometry_editing = None;
                Vec::new()
            }
            None => Vec::new(),
        };
        self.scene.set_hidden_parts(hidden.clone(), self.viewport.gpu());
        self.picking.set_hidden_parts(hidden);
    }

    /// The edit mode's window, while the mode is on: the changes it asks for.
    pub(in crate::gui_paint) fn geometry_mask_window(&self, ctx: &egui::Context) -> Vec<PanelEvent> {
        let mut out = Vec::new();
        let doc = self.document.as_ref().map(|d| d.doc());
        if let Some(target) = doc.zip(self.layers.geometry_editing.as_ref()).and_then(|(doc, r)| MaskTarget::find(doc, r)) {
            window::show(ctx, &target, &mut out);
        }
        out.into_iter().map(PanelEvent::Layers).collect()
    }

    /// While the mode is on, a click in the viewport excludes the part under it (hidden parts are not picked, so the
    /// part behind comes next) and nothing is painted; otherwise `input` is passed on.
    pub(in crate::gui_paint) fn geometry_pick(&mut self, input: Vec<StrokeInput>) -> Vec<StrokeInput> {
        let Some(r) = self.layers.geometry_editing.clone() else { return input };
        let at = input.iter().find_map(|ev| match ev {
            StrokeInput::Down(s) => Some(s.pos),
            _ => None,
        });
        let Some(at) = at else { return Vec::new() };
        let event = {
            let (Some(doc), Some(view)) = (self.document.as_ref(), self.view_projector()) else {
                self.diag.info(Area::General, "The view is still being prepared; try again in a moment.");
                return Vec::new();
            };
            let hit = materialcraft::projection::ViewProjector::pick(&view, at);
            MaskTarget::find(doc.doc(), &r).zip(hit).and_then(|(target, hit)| target.exclude_mesh_part(hit.part))
        };
        match event {
            Some(ev) => self.edit_layers(ev),
            None => self.diag.info(Area::General, "There is no part of this layer's texture set under the pointer."),
        }
        Vec::new()
    }
}
