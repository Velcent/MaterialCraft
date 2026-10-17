//! The colour picker and its eyedroppers (CP-01…CP-11 without the screen eyedropper and colour management).
//!
//! - [`ColorPicker`]: the floating picker window: spectrum, HSV/RGB sliders, sRGB hex, current and previous colour,
//!   swatches kept in the user settings, and the viewport eyedropper's state.
//! - [`ColorField`]: the colour field any panel draws; it opens the picker or picks straight from the view.
//! - [`eyedropper::Eyedropper`] requests ([`eyedropper::SampleRequest`]) are answered by the screen that owns the view: the displayed colour
//!   from a GPU read-back, or with Shift the channel's stored value through the shared texel lookup
//!   (`materialcraft::paint::probe`), handed back as a [`Sample`].
//!
//! Colours are kept in the field's own encoding ([`color::FieldColor`]); the spectrum, sliders, hex code and swatches work
//! on the display-encoded (sRGB) colour.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod color;
pub mod eyedropper;
pub mod field;
pub mod link;
mod painters;
pub mod picker;
mod sliders;
mod spectrum;
mod swatches;

pub use eyedropper::{Sample, Source};
#[allow(unused_imports)] // the brush panel (roadmap step 20) draws the first colour fields
pub use field::{ColorField, ColorFieldResponse};
pub use picker::{ColorPicker, open_unbound};
