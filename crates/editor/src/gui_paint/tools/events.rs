//! [`ToolEvent`]: what the tool keys and the quick-access bar ask of the tools' session state (quick mask, polygon
//! fill settings, the clone source).

use super::super::screen_images::ImageEdit;
use super::screen_image::ImageSlot;
use materialcraft::paint::effect::PickMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolEvent {
    /// Quick mask: off → editing, editing or applied → off; the mask is kept (ADR 0008 mc2-q-pt-07c).
    QuickMaskToggle,
    /// Swap protected and free (editing only).
    QuickMaskInvert,
    /// Empty the quick mask of the selected texture set.
    QuickMaskClear,
    /// Polygon fill: swap white (fill) and black (un-pick).
    SwapFill,
    /// Polygon fill: what a click grows to.
    PickMode(PickMode),
    /// Clone: mark the surface under the pointer as the source (PT-27).
    MarkCloneSource,
    /// The stencil or the projection image (PT-15, PJ-26).
    Image(ImageSlot, ImageEdit),
}
