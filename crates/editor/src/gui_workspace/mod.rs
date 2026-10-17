//! The editor's workspace shell, shared by its screens: docking with saved layouts on
//! craft-ui's docking model (a thin adapter), the panel registry trait, the rebindable shortcut
//! table and its page. Screens supply their panels, actions and built-in layouts.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod convert;
pub mod dock;
mod panel;
pub mod presets;
pub mod shortcuts;
mod style;

pub use dock::Workspace;
pub use panel::PanelKind;
pub use presets::Preset;
pub use style::dock_style;
