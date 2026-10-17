//! The quick mask (PT-42..PT-44, ADR 0008 mc2-q-pt-07c): a session mask per texture set that every brush stroke
//! honours through the stamp stage's [`TexelFactor`](materialcraft::stamp::TexelFactor), painted with the brush tools in an edit state and shown as a
//! tint. It is session state: never part of the document or saved with it.
//!
//! States ([`QuickMaskState`]): **off** (the mask is kept, nothing honours it) → toggle → **editing** (the brush
//! tools paint the mask: the paint tool protects, the eraser frees) → back to the previous tool → **applied** (every
//! stroke is held back where the mask protects) → toggle → off. Toggling while editing switches it off as well; the
//! mask is kept throughout. Clearing is a separate action. Invert (editing only) swaps protected and free.
//!
//! The image stores how much a texel is *protected* (0 free … 1 protected), so a new, empty mask frees everything
//! without allocating a pixel. Quick-mask strokes undo and redo in their own short history ([`QuickMask::undo`]),
//! which the screen uses while editing; the document's timeline never sees them.

mod stroke;
#[cfg(test)]
mod tests;

pub use stroke::QuickMaskStroke;

use materialcraft::paint::{PixelFormat, TiledImage};
use materialcraft::stamp::QuickMask as MaskFactor;
use std::collections::BTreeMap;

/// How many quick-mask strokes can be undone.
pub const UNDO_DEPTH: usize = 64;

/// What the quick mask is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuickMaskState {
    /// Kept but not used.
    #[default]
    Off,
    /// The brush tools paint the mask.
    Editing,
    /// Every stroke honours the mask.
    Applied,
}

/// One texture set's mask as it was before a change (an undo or redo step).
struct Snapshot {
    set: String,
    image: Option<TiledImage>,
}

/// The quick masks of an open document's texture sets, by set id.
#[derive(Default)]
pub struct QuickMask {
    masks: BTreeMap<String, TiledImage>,
    state: QuickMaskState,
    inverted: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl QuickMask {
    pub fn state(&self) -> QuickMaskState {
        self.state
    }

    /// True while the brush tools paint the mask.
    pub fn is_editing(&self) -> bool {
        self.state == QuickMaskState::Editing
    }

    /// True while the mask is shown (editing or applied).
    pub fn is_shown(&self) -> bool {
        self.state != QuickMaskState::Off
    }

    /// True if protected and free are swapped.
    pub fn is_inverted(&self) -> bool {
        self.inverted
    }

    /// The toggle key: off → editing; editing or applied → off (the mask is kept).
    pub fn toggle(&mut self) {
        self.state = match self.state {
            QuickMaskState::Off => QuickMaskState::Editing,
            QuickMaskState::Editing | QuickMaskState::Applied => QuickMaskState::Off,
        };
    }

    /// Back to the previous tool: editing → applied. Returns whether the state changed.
    pub fn apply(&mut self) -> bool {
        let editing = self.is_editing();
        if editing {
            self.state = QuickMaskState::Applied;
        }
        editing
    }

    /// Swaps protected and free (editing only). Returns whether it did.
    pub fn invert(&mut self) -> bool {
        let editing = self.is_editing();
        if editing {
            self.inverted = !self.inverted;
        }
        editing
    }

    /// Empties texture set `set`'s mask (undoable). Returns whether there was one.
    pub fn clear(&mut self, set: &str) -> bool {
        let Some(old) = self.masks.remove(set) else { return false };
        self.push_undo(Snapshot { set: set.to_string(), image: Some(old) });
        true
    }

    /// Forgets every mask and its history (another document, or the document closed).
    pub fn forget(&mut self) {
        *self = QuickMask::default();
    }

    /// Texture set `set`'s mask, if it has one.
    pub fn image(&self, set: &str) -> Option<&TiledImage> {
        self.masks.get(set)
    }

    /// The factor every stroke into `set` is multiplied with while the mask is applied; `size` is the set's
    /// resolution (a mask of another size, from before a resolution change, is not used).
    pub fn factor(&self, set: &str, size: (u32, u32)) -> Option<MaskFactor<'_>> {
        if self.state != QuickMaskState::Applied {
            return None;
        }
        let image = self.masks.get(set).filter(|i| i.size() == size)?;
        // the image holds protection: the factor is its complement
        Some(MaskFactor { image, inverted: !self.inverted })
    }

    /// How much texel `(x, y)` of `set` is held back, 0 … 1 (for the tint), with the inversion applied.
    pub fn protection(&self, set: &str, x: u32, y: u32) -> f32 {
        let v = self.masks.get(set).map_or(0.0, |i| i.get(x, y)[0].clamp(0.0, 1.0));
        if self.inverted { 1.0 - v } else { v }
    }

    /// Texture set `set`'s mask to paint into at `size`, made (empty) or remade at that size when needed; the state
    /// before is kept for undo (one step per stroke). `None` for a size no image can have.
    pub(crate) fn begin_edit(&mut self, set: &str, size: (u32, u32)) -> Option<&mut TiledImage> {
        let old = self.masks.get(set).cloned();
        let fits = old.as_ref().is_some_and(|i| i.size() == size);
        if !fits {
            let format = PixelFormat::from_id("gray8")?;
            let image = TiledImage::new(size.0, size.1, format).ok()?;
            self.masks.insert(set.to_string(), image);
        }
        self.push_undo(Snapshot { set: set.to_string(), image: old });
        self.masks.get_mut(set)
    }

    pub(crate) fn image_mut(&mut self, set: &str) -> Option<&mut TiledImage> {
        self.masks.get_mut(set)
    }

    fn push_undo(&mut self, s: Snapshot) {
        self.undo.push(s);
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Undoes the last quick-mask stroke or clear; returns the texture set it changed.
    pub fn undo(&mut self) -> Option<String> {
        let s = self.undo.pop()?;
        let back = self.swap(s);
        let set = back.set.clone();
        self.redo.push(back);
        Some(set)
    }

    /// Redoes what [`Self::undo`] undid; returns the texture set it changed.
    pub fn redo(&mut self) -> Option<String> {
        let s = self.redo.pop()?;
        let back = self.swap(s);
        let set = back.set.clone();
        self.undo.push(back);
        Some(set)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Puts `s` in place and returns what was there.
    fn swap(&mut self, s: Snapshot) -> Snapshot {
        let now = match s.image {
            Some(image) => self.masks.insert(s.set.clone(), image),
            None => self.masks.remove(&s.set),
        };
        Snapshot { set: s.set, image: now }
    }
}
