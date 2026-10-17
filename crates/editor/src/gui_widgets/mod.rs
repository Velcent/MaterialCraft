//! Widgets shared by the editor's screens: the slider row, icon strips, the overflowing toolbar
//! and the icon set. They know the theme but no screen's state.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod icons;
pub mod slider;
pub mod strip;
pub mod toolbar;

pub use icons::Icon;
pub use slider::SliderRow;
