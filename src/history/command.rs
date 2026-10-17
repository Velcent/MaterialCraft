//! [`Command`]: what the history needs from an edit type.

/// A reversible change to a `Target`. The paint document's [`crate::paint::edit::Edit`] is one; shader settings
/// get their own command type and their own [`super::History`] (they never appear on the project timeline).
pub trait Command: Sized {
    type Target;
    type Error;

    /// Applies the command and returns its inverse. On error the target must be unchanged.
    fn apply(self, target: &mut Self::Target) -> Result<Self, Self::Error>;

    /// Folds `later`, an inverse recorded right after `self` in the same coalesced step, into `self`, which is
    /// the older inverse and wins. Returns `later` if it cannot be folded (the step then keeps both).
    fn absorb(&mut self, later: Self) -> Option<Self> {
        Some(later)
    }

    /// Calls `f(id, bytes)` for every shared allocation the command keeps alive (tiles). Allocations with the
    /// same id are counted once against the byte budget.
    fn retained(&self, _f: &mut dyn FnMut(usize, usize)) {}

    /// Calls `f(id, bytes)` for every shared allocation the target holds itself: memory the history shares
    /// with the target costs nothing extra.
    fn target_retained(_target: &Self::Target, _f: &mut dyn FnMut(usize, usize)) {}
}
