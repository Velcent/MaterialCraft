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

/// A named search of the asset library.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedSearch {
    pub name: String,
    pub query: String,
}

/// Asset library roots and saved searches; the headless library gets them as plain data.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrarySettings {
    pub roots: Vec<PathBuf>,
    pub saved_searches: Vec<SavedSearch>,
}

/// The colour picker's swatches, linear RGBA.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SwatchSettings {
    pub colors: Vec<[f32; 4]>,
}
