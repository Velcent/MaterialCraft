//! The 2D view's part of a Paint-screen frame: it shows the texture set selected in the layer panel (V2-01), so the
//! screen tells it which set that is, hands it the set's island outlines and leaves the other sets' parts out of
//! its GPU picture.

use super::PaintMode;
use super::uv_view::UvScene;

impl PaintMode {
    /// Points the 2D view at the current texture set. Call once per frame before the views are drawn.
    pub(super) fn sync_uv_view(&mut self, ctx: &eframe::egui::Context) {
        let Some(doc) = self.document.as_mut() else {
            self.uv_outlines.clear();
            self.viewport.uv_mut().scene = UvScene::default();
            return;
        };
        let current = self.layers.current_set(doc.doc()).map(|s| (s.id.clone(), s.name.clone(), s.resolution, s.material_slot, s.uv_set));
        let Some((id, name, resolution, material, uv_set)) = current else {
            self.viewport.uv_mut().scene = UvScene::default();
            self.scene.show_uv_set(None, self.viewport.gpu());
            return;
        };
        let index = doc.doc().texture_sets.iter().position(|s| s.id == id);
        self.scene.show_uv_set(index, self.viewport.gpu());
        // the outline is only worth building while the 2D view is shown
        let outline = if self.viewport.uv_rect().is_some() { self.uv_outlines.get(doc.shared_mesh(), material, uv_set) } else { None };
        if self.uv_outlines.is_building() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        self.viewport.uv_mut().scene = UvScene { set: Some((name, resolution)), outline };
    }
}
