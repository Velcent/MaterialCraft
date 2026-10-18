//! Changes to the open document from the panels: layer stack edits and moves through the history, each
//! followed by recompositing what changed.

use super::PaintMode;
use super::document_state::OpenDocument;
use super::panels::layers::LayerEvent;
use super::stack_editor;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::edit::EditError;

impl PaintMode {
    /// A layer panel request: an edit of the stack as one history step, then the set is recomposited.
    pub(super) fn edit_layers(&mut self, ev: LayerEvent) {
        if let LayerEvent::SaveAsset(what) = ev {
            self.save_asset(what);
            return;
        }
        let Some(doc) = &mut self.document else { return };
        if let LayerEvent::Isolate(layer) = ev {
            self.layers.isolated = layer.clone();
            self.scene.isolate(doc, self.display, layer);
            return;
        }
        if let LayerEvent::EditGeometryMask(layer) = ev {
            self.layers.geometry_editing = layer;
            return;
        }
        match stack_editor::apply(doc, ev) {
            Ok(applied) => {
                if let Some(ids) = applied.select {
                    self.layers.select(ids);
                }
                if let Some(set) = applied.set {
                    self.scene.refresh(doc, self.display, vec![set]);
                }
            }
            Err(e) => self.diag.warn(Area::Project, format!("That layer change was not made: {e}.")),
        }
    }

    /// Undo, redo or a timeline jump, then recompositing: the history does not say which sets changed.
    pub(super) fn move_in_history(&mut self, what: &str, step: impl FnOnce(&mut OpenDocument) -> Result<(), EditError>) {
        let Some(doc) = &mut self.document else { return };
        if let Err(e) = step(doc) {
            self.diag.warn(Area::Project, format!("{what} failed: {e}. The history before that point was dropped."));
        }
        if doc.sync_sets() {
            self.scene.rebuild(doc, self.display);
        } else {
            let all = (0..doc.doc().texture_sets.len()).collect();
            self.scene.refresh(doc, self.display, all);
        }
    }
}
