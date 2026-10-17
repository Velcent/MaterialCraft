//! The Paint screen's panels. Each draws one part of the screen and reports what the user asked
//! for as [`PanelEvent`]s; `PaintMode` applies them.

pub mod channel_bar;
mod dock_content;
pub mod import_report;
mod interface_settings;
pub mod layers;
pub mod layout;
pub mod log;
pub mod menu_bar;
mod panel_strip;
pub mod status_bar;
pub mod texture_sets;
mod timeline;
mod tools_strip;
pub mod view_settings;
mod viewport_toolbar;

use super::channel_view::ChannelView;
use super::shell::panels::PaintPanel;
use super::shell::tools::Tool;
use crate::gui_settings::Side;
use crate::gui_workspace::shortcuts::PageEdit;
use materialcraft::diag::Target;

/// One of the two vertical strips.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strip {
    Tools,
    Panels,
}

/// A request from a panel.
#[derive(Clone, PartialEq, Debug)]
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
    /// Step through the view modes (+1 next, -1 previous).
    CycleChannel(i32),
    /// Show or hide a panel where it was.
    TogglePanel(PaintPanel),
    /// The panel strip's button: show or hide the panel as a popover at `[x, y, w, h]`.
    Popover(PaintPanel, [f32; 4]),
    SwitchLayout(String),
    /// Save the current layout under a name.
    SaveLayout(String),
    DeleteLayout(String),
    /// Go back to the current layout as it was saved.
    ResetLayout,
    MoveStrip(Strip, Side),
    SelectTool(Tool),
    PreviousTool,
    Shortcut(PageEdit),
    SetUiScale(f32),
    /// The workspace refused a docking change (shown in the status bar).
    DockRefused(String),
    /// Go to what a log entry concerns (LG-10).
    Reveal(Target),
    Undo,
    Redo,
    /// Return the document to the state after this many steps (a timeline block).
    JumpHistory(usize),
    /// A change to the layer stack or a layer panel view state.
    Layers(layers::LayerEvent),
    /// A layer shortcut, resolved against the layer panel's selection.
    LayerAction(layers::LayerAction),
}
