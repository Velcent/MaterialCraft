//! The Paint screen's panels. Each draws one part of the screen and reports what the user asked
//! for as [`PanelEvent`]s; `PaintMode` applies them.

pub mod channel_bar;
pub mod import_report;
pub mod layout;
pub mod menu_bar;
pub mod status_bar;
pub mod texture_sets;
pub mod view_settings;

use super::channel_view::ChannelView;
use eframe::egui::Color32;

/// Fill of the side panels (as in the node editor).
pub const PANEL_FILL: Color32 = Color32::from_rgb(40, 40, 40);

/// A request from a panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelEvent {
    /// Pick a `.mcpaint` or `.spp` file.
    Open,
    /// Pick a Substance Painter project.
    Import,
    Close,
    Home,
    ResetCamera,
    ShowChannel(ChannelView),
    SetVisible(usize, bool),
    /// Cancel the running load or import.
    CancelLoad,
}
