//! Changes to the open document from the panels: layer stack edits and moves through the history, each
//! followed by recompositing what changed.

use super::PaintMode;
use super::channel_view::ChannelView;
use super::document_state::OpenDocument;
use super::panels::layers::LayerEvent;
use super::scene_sync::SceneSync;
use super::stack_editor;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::edit::EditError;
use materialcraft::paint::set_changes::CompositeState;

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
                    self.scene.refresh(doc, self.display, std::iter::once(set).chain(applied.also).collect());
                }
            }
            Err(e) => self.diag.warn(Area::Project, format!("That layer change was not made: {e}.")),
        }
    }

    /// Undo, redo or a timeline jump, then recompositing the sets that changed (the history does not say which, so
    /// the state before the move is compared with the state after it).
    pub(super) fn move_in_history(&mut self, what: &str, step: impl FnOnce(&mut OpenDocument) -> Result<(), EditError>) {
        let Some(doc) = &mut self.document else { return };
        let (result, moved) = history_move(doc, step);
        if let Err(e) = result {
            self.diag.warn(Area::Project, format!("{what} failed: {e}. The history before that point was dropped."));
        }
        moved.recomposite(&mut self.scene, doc, self.display);
    }
}

/// What a move in the history needs recomposited.
#[derive(Debug, PartialEq)]
pub(super) enum Moved {
    /// The number of texture sets changed: the scene is rebuilt.
    SetsAddedOrRemoved,
    /// These sets changed.
    Changed(Vec<usize>),
}

impl Moved {
    /// Asks `scene` for what the move needs: the whole scene, or the maps of the sets that changed.
    pub(super) fn recomposite(self, scene: &mut SceneSync, doc: &mut OpenDocument, view: ChannelView) {
        match self {
            Moved::SetsAddedOrRemoved => scene.rebuild(doc, view),
            Moved::Changed(sets) => scene.refresh(doc, view, sets),
        }
    }
}

/// Runs a move in the history (`step`) and finds out what it changed.
pub(super) fn history_move(doc: &mut OpenDocument, step: impl FnOnce(&mut OpenDocument) -> Result<(), EditError>) -> (Result<(), EditError>, Moved) {
    let before = CompositeState::of(doc.doc());
    let result = step(doc);
    let moved = if doc.sync_sets() { Moved::SetsAddedOrRemoved } else { Moved::Changed(before.changed_sets(doc.doc())) };
    (result, moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_paint::panels::layers::LayerEvent;
    use crate::gui_paint::stack_editor;
    use materialcraft::paint::{Document, TextureSet};
    use std::path::PathBuf;

    #[test]
    fn undo_names_only_the_set_it_changed() {
        let mut d = Document::default();
        for id in ["a", "b", "c"] {
            d.texture_sets.push(TextureSet::new(id, id, (8, 8)));
        }
        let mut doc = OpenDocument::new(d, PathBuf::from("t.mcpaint"), None);
        let create = LayerEvent::Create { set: "b".into(), what: materialcraft::paint::edit::ops::NewLayer::Fill, above: None };
        stack_editor::apply(&mut doc, create).expect("create");
        let (r, moved) = history_move(&mut doc, |d| d.undo().map(|_| ()));
        assert!(r.is_ok());
        assert_eq!(moved, Moved::Changed(vec![1]));
        let (_, moved) = history_move(&mut doc, |d| d.redo().map(|_| ()));
        assert_eq!(moved, Moved::Changed(vec![1]));
    }
}
