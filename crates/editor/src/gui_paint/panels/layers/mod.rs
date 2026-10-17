//! The layer panel (roadmap step 11): the focused texture set's stack with thumbnails, the view channel,
//! effects, mask states, create/duplicate/delete/group/rename, drag-and-drop reordering with a landing marker
//! and multi-selection. It changes nothing itself: every request is a [`LayerEvent`] that `PaintMode` applies
//! through `paint::edit` and the history.
//!
//! - [`rows`]: the listed rows and pointer → drop target; [`selection`]: the selection and paint target;
//! - [`row_ui`], [`bars`], [`menus`]: drawing; [`thumbnails`]: thumbnails off the UI thread;
//! - [`list`]: the scrolling list with drags; [`StackDropTarget`]: what the asset tray calls while dragging
//!   an asset over the stack (LS-18, LS-19).

mod bars;
pub mod events;
mod list;
mod menus;
mod row_ui;
pub mod rows;
pub mod selection;
pub mod thumbnails;

#[cfg(test)]
mod tests;

pub use events::LayerEvent;

use super::PanelEvent;
use crate::gui_paint::document_state::OpenDocument;
use eframe::egui::{Pos2, Ui};
use materialcraft::history::CoalesceKey;
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::edit::ops::{DropPayload, DropTarget, NewLayer};
use materialcraft::paint::{Channel, Document, LayerKind, TextureSet};
use rows::{Folding, Rows};
use selection::{Part, Selection};
use thumbnails::Thumbnails;

/// Keyboard actions on the selection (from the shortcut table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerAction {
    New(NewLayer),
    Duplicate,
    Group,
    Delete,
    Rename,
}

/// What the asset tray (step 15) calls while it drags an asset: where it would land and showing that.
// Nothing drags assets yet; the tray is built in step 15. Tested in `tests`.
#[allow(dead_code)]
pub trait StackDropTarget {
    /// The drop target under the screen position `pos`, if it is over the list and the drop would do
    /// something.
    fn target_at(&self, doc: &Document, pos: Pos2, payload: &DropPayload) -> Option<DropTarget>;
    /// Shows the landing marker for `payload` while it is dragged (`None` when the drag ends).
    fn hover(&mut self, payload: Option<DropPayload>);
}

/// Panel state.
#[derive(Default)]
pub struct LayerPanel {
    set: Option<String>,
    channel: Option<Channel>,
    pub selection: Selection,
    folding: Folding,
    thumbs: Thumbnails,
    list: list::ListState,
    /// The running opacity drag's history key.
    drag_key: Option<CoalesceKey>,
    next_key: u64,
    /// The layer whose mask the viewport shows alone.
    pub isolated: Option<LayerRef>,
    /// The asset being dragged over the stack from elsewhere.
    external: Option<DropPayload>,
}

impl LayerPanel {
    /// The texture set shown, falling back to the first one.
    pub fn current_set<'a>(&self, doc: &'a Document) -> Option<&'a TextureSet> {
        self.set.as_deref().and_then(|id| doc.texture_set(id)).or_else(|| doc.texture_sets.first())
    }

    /// The channel the thumbnails show and the blend controls edit.
    fn view_channel(&self, set: &TextureSet) -> Option<Channel> {
        self.channel.filter(|c| set.has_channel(*c)).or_else(|| set.channels.first().map(|c| c.channel))
    }

    /// Whether the tools can paint: the focused layer's mask, or a paint layer's content (ADR 0008,
    /// mc2-q-pt-02c).
    pub fn paintable(&self, doc: &Document) -> bool {
        let Some(layer) = self.current_set(doc).and_then(|s| s.layer(self.selection.focus()?)) else {
            return false;
        };
        match self.selection.part {
            Part::Mask => layer.mask.is_some(),
            Part::Content => matches!(layer.kind, LayerKind::Paint(_)),
        }
    }

    /// Selects `ids` (new layers after an edit).
    pub fn select(&mut self, ids: Vec<String>) {
        self.selection.set(ids);
    }

    /// Forgets everything about the previous document.
    pub fn reset(&mut self) {
        *self = LayerPanel::default();
    }

    /// A keyboard action on the selection.
    pub fn action(&mut self, doc: &Document, action: LayerAction) -> Option<LayerEvent> {
        let set = self.current_set(doc)?.id.clone();
        let ids = self.selection.ids().to_vec();
        Some(match action {
            LayerAction::New(what) => LayerEvent::Create { set, what, above: self.selection.focus().map(str::to_string) },
            LayerAction::Duplicate => LayerEvent::Duplicate { set, ids },
            LayerAction::Group => LayerEvent::Group { set, ids },
            LayerAction::Delete => LayerEvent::Delete { set, ids },
            LayerAction::Rename => {
                let id = self.selection.focus()?.to_string();
                let name = doc.texture_set(&set)?.layer(&id)?.name.clone();
                self.list.start_rename(id, name);
                return None;
            }
        })
    }

    /// Draws the panel.
    pub fn show(&mut self, ui: &mut Ui, open: &OpenDocument, events: &mut Vec<PanelEvent>) {
        let doc = open.doc();
        let Some(set) = self.current_set(doc) else {
            ui.weak("This document has no texture sets.");
            return;
        };
        self.selection.retain(|id| set.layer(id).is_some());
        let channel = self.view_channel(set);
        let mut out: Vec<LayerEvent> = Vec::new();
        let (new_set, new_channel) = bars::pickers(ui, doc, set, channel);
        if let Some(focus) = self.selection.focus()
            && let (Some(layer), Some(c)) = (set.layer(focus), channel)
        {
            let r = LayerRef::new(set.id.clone(), focus);
            bars::blending(ui, &r, layer, c, &mut self.drag_key, &mut self.next_key, &mut out);
        }
        ui.separator();
        let bottom = 30.0;
        let rows = Rows::build(set, &self.folding);
        let height = (ui.available_height() - bottom).max(60.0);
        if let Some(c) = channel {
            self.thumbs.begin(ui.ctx(), &set.id, c);
        }
        let ctx = list::ListCtx { set, rows: &rows, channel, height, external: self.external.as_ref() };
        self.list.show(ui, ctx, &mut self.selection, &mut self.folding, &mut self.thumbs, self.isolated.as_ref(), &mut out);
        if let Some(c) = channel {
            self.thumbs.end(ui.ctx(), set, c);
        }
        ui.separator();
        let focus = self.selection.focus().and_then(|id| set.layer(id).map(|l| (LayerRef::new(set.id.clone(), id), l)));
        bars::buttons(ui, &set.id, focus.as_ref().map(|(r, l)| (r, *l)), self.selection.ids(), &mut out);
        if let Some(s) = new_set {
            self.set = Some(s);
            self.selection.clear();
        }
        if new_channel.is_some() {
            self.channel = new_channel;
        }
        events.extend(out.into_iter().map(PanelEvent::Layers));
    }
}

impl StackDropTarget for LayerPanel {
    fn target_at(&self, doc: &Document, pos: Pos2, payload: &DropPayload) -> Option<DropTarget> {
        let set = self.current_set(doc)?;
        let rows = Rows::build(set, &self.folding);
        let landing = self.list.landing_at(&rows, pos)?;
        materialcraft::paint::edit::ops::accepts(doc, &landing.target, payload).then_some(landing.target)
    }

    fn hover(&mut self, payload: Option<DropPayload>) {
        self.external = payload;
    }
}
