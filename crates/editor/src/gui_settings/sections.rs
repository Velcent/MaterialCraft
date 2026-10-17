//! The sections of `config.toml`. Every struct has `#[serde(default)]`, so a missing key falls
//! back to its default and a damaged section resets alone (craft-settings rules).

use std::collections::BTreeMap;
use std::path::PathBuf;

use craft_ui::docking::{Layout, Location};
use serde::{Deserialize, Serialize};

/// A docking layout as stored: panels by their stable id strings, so this module doesn't know
/// the screens' panel types. The workspace validates ids when it loads one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedLayout {
    pub layout: Layout<String>,
    /// Where each closed panel was, so reopening puts it back.
    pub hidden: BTreeMap<String, Location<String>>,
}

/// Which window edge a vertical strip sits on (UI-01, UI-03).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    #[default]
    Left,
    Right,
}

/// The Paint screen's workspace (WS-01…05, UI-01, UI-03).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceSettings {
    /// The layout as the user left it; `None` until the first change (the built-in default applies).
    pub current: Option<SavedLayout>,
    /// The name of the layout `current` started from.
    pub current_name: String,
    /// Layouts the user saved by name; a name of a built-in layout overrides it.
    pub saved: BTreeMap<String, SavedLayout>,
    pub tools_side: Side,
    pub panels_side: Side,
}

impl Default for WorkspaceSettings {
    fn default() -> WorkspaceSettings {
        WorkspaceSettings { current: None, current_name: String::new(), saved: BTreeMap::new(), tools_side: Side::Left, panels_side: Side::Right }
    }
}

/// The user's changes to the shortcut table (WS-12): action id → bindings in text form. Only
/// actions that differ from their defaults are stored; an empty list means "unbound".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutSettings {
    pub bindings: BTreeMap<String, Vec<String>>,
}

/// Interface settings (WS-10 as renamed by ADR 0008).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InterfaceSettings {
    /// Scale of the whole interface on top of the system's (1 = 100 %).
    pub ui_scale: f32,
}

impl InterfaceSettings {
    pub const MIN_SCALE: f32 = 0.5;
    pub const MAX_SCALE: f32 = 3.0;

    /// The scale to apply: a damaged value falls back to 100 %.
    pub fn scale(&self) -> f32 {
        if self.ui_scale.is_finite() { self.ui_scale.clamp(Self::MIN_SCALE, Self::MAX_SCALE) } else { 1.0 }
    }
}

impl Default for InterfaceSettings {
    fn default() -> InterfaceSettings {
        InterfaceSettings { ui_scale: 1.0 }
    }
}

/// A named search of the asset library (AS-16): type filter, folder and query text, hand-editable.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedSearch {
    pub name: String,
    pub query: String,
    /// Usage keywords (`material`, `smart-mask`, …); empty shows every usage.
    pub usages: Vec<String>,
    /// The library id the search is limited to (`user`, `extra:<folder>`); empty searches every library.
    pub library: String,
    /// Root-relative folder inside that library; empty for the whole library.
    pub folder: String,
    /// Shown as its own tab in the asset tray (AS-17).
    pub tab: bool,
}

/// How the asset tray arranges itself (AS-09).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayLayout {
    /// Wide when the panel is wide enough, else narrow.
    #[default]
    Auto,
    /// Type icons and the folder tree beside the assets.
    Wide,
    /// Types in one menu, the folder tree folded away.
    Narrow,
}

/// The size of the asset tray's thumbnails, or a list (AS-09).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbSize {
    Small,
    #[default]
    Medium,
    Large,
    List,
}

/// The asset tray's view choices.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraySettings {
    pub layout: TrayLayout,
    pub thumbnails: ThumbSize,
    /// Leave out folders that hold nothing matching the type filter and search (AS-13).
    pub hide_empty: bool,
}

/// Asset library roots and saved searches; the headless library gets them as plain data. The user's
/// own library is `library/` next to the settings file; `roots` are the further ones (AS-01, AS-18).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrarySettings {
    pub roots: Vec<PathBuf>,
    pub saved_searches: Vec<SavedSearch>,
    pub tray: TraySettings,
}

/// The colour picker's swatches (CP-11), sRGB-encoded RGBA, kept across sessions and projects.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SwatchSettings {
    pub colors: Vec<[f32; 4]>,
}

/// Which components the colour picker's sliders show (CP-08).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSliders {
    #[default]
    Hsv,
    Rgb,
}

/// How colour values are written, the same in every colour slider (CP-08).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorUnits {
    /// 0–1 floats.
    Float,
    /// 0–255 for RGB, degrees for hue, percent for saturation and value.
    #[default]
    Natural,
}

/// The colour picker's view choices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorPickerSettings {
    pub sliders: ColorSliders,
    pub units: ColorUnits,
    /// Slider tracks tinted by the colour.
    pub tinted: bool,
}

impl Default for ColorPickerSettings {
    fn default() -> ColorPickerSettings {
        ColorPickerSettings { sliders: ColorSliders::Hsv, units: ColorUnits::Natural, tinted: true }
    }
}
