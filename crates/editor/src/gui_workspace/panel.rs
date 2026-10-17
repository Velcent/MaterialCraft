//! What a screen's panel type must tell the workspace.

use std::fmt::Debug;
use std::hash::Hash;

use craft_ui::docking::Permissions;

use crate::gui_widgets::Icon;

/// A screen's panels, as a small `Copy` enum. The workspace stores them by [`PanelKind::id`].
pub trait PanelKind: Copy + Eq + Hash + Debug + Send + Sync + 'static {
    /// Every panel, in panel-strip order.
    const ALL: &'static [Self];

    /// Stable, non-localised id for the settings file.
    fn id(self) -> &'static str;

    fn title(self) -> &'static str;

    fn icon(self) -> Icon;

    /// The document canvas: it stays docked in the main area and can't be moved, floated,
    /// closed or covered by tabs (the viewport). Its siblings may still split beside it.
    fn is_canvas(self) -> bool {
        false
    }

    /// The panel with `id`.
    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.id() == id)
    }

    /// What the docking model allows for this panel.
    fn permissions(self) -> Permissions {
        if self.is_canvas() { Permissions::PROTECTED } else { Permissions::default() }
    }
}
