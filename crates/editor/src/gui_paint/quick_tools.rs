//! The Paint screen's side of the tool keys (PT-43, PT-45): quick mask on/off, invert, clear, back to the previous
//! tool (which applies it), its own undo while it is edited, and the polygon fill settings keys. The state lives in
//! the brush tools ([`super::tools::Painting`]); this keeps the tint in step with it.

use super::PaintMode;
use super::quick_mask_tint::QuickMaskTint;
use super::tools::ToolEvent;
use materialcraft::diag::{Area, SinkExt};

impl PaintMode {
    pub(super) fn tool_event(&mut self, ev: ToolEvent) {
        let mask = &mut self.painting.quick_mask;
        match ev {
            ToolEvent::QuickMaskToggle => {
                mask.toggle();
                self.update_tint();
            }
            ToolEvent::QuickMaskInvert => {
                if mask.invert() {
                    self.update_tint();
                } else {
                    self.diag.info(Area::General, "The quick mask can be inverted while it is edited.");
                }
            }
            ToolEvent::QuickMaskClear => {
                let set = self.document.as_ref().and_then(|d| self.layers.current_set(d.doc()).map(|s| s.id.clone()));
                if let Some(set) = set
                    && self.painting.quick_mask.clear(&set)
                {
                    self.update_tint();
                }
            }
            ToolEvent::SwapFill => self.painting.brush.polygon.white = !self.painting.brush.polygon.white,
            ToolEvent::PickMode(m) => self.painting.brush.polygon.mode = m,
        }
    }

    /// The previous-tool key also applies a quick mask being edited.
    pub(super) fn apply_quick_mask(&mut self) {
        if self.painting.quick_mask.apply() {
            self.update_tint();
        }
    }

    /// Undo (or redo) while the quick mask is edited steps through its strokes, not the document. Returns whether
    /// it did.
    pub(super) fn quick_mask_step(&mut self, undo: bool) -> bool {
        let mask = &mut self.painting.quick_mask;
        if !mask.is_editing() {
            return false;
        }
        let changed = if undo { mask.undo() } else { mask.redo() };
        if changed.is_some() {
            self.update_tint();
        }
        true
    }

    /// Hands the quick mask's current look to the view and recomposites every set.
    pub(super) fn update_tint(&mut self) {
        let Some(doc) = &mut self.document else { return };
        self.scene.set_tint(QuickMaskTint::of(&self.painting.quick_mask, doc.doc()));
        self.scene.remap(doc, self.display);
    }
}
