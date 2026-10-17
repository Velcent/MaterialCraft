//! Look shared by the node editor and the Paint screen: panel headers and their colours.
//! Lives in its own module so the Paint screen does not reach into the editor.

use eframe::egui::Color32;

/// Background of panel headers, status and view areas.
pub const HEADER_BG: Color32 = Color32::from_rgb(30, 30, 30);
/// Text of panel headers.
pub const HEADER_TEXT: Color32 = Color32::from_gray(225);
/// The orange used for the active tab, selection and the "PAINT" mark.
pub const ACCENT: Color32 = Color32::from_rgb(235, 130, 20);
/// Fill of panels and dock areas.
pub const PANEL_FILL: Color32 = Color32::from_rgb(40, 40, 40);
/// The clear border between docked panels (WS-14).
pub const BORDER: Color32 = Color32::from_gray(22);
/// Labels of controls (slider rows, settings).
pub const LABEL_TEXT: Color32 = Color32::from_gray(190);
/// The empty part of a slider track.
pub const SLIDER_TRACK: Color32 = Color32::from_gray(70);
/// The part of a slider track up to the knob.
pub const SLIDER_FILL: Color32 = Color32::from_rgb(200, 112, 24);
/// Background of the tools and panel strips.
pub const STRIP_BG: Color32 = Color32::from_rgb(30, 30, 30);
/// A strip button under the pointer.
pub const STRIP_HOVER: Color32 = Color32::from_gray(55);
/// The active tool, or the panel whose popover is open.
pub const STRIP_SELECTED: Color32 = Color32::from_gray(62);
/// Icons in strips and toolbars.
pub const ICON: Color32 = Color32::from_gray(200);
/// Icons of a disabled strip.
pub const ICON_DISABLED: Color32 = Color32::from_gray(85);
