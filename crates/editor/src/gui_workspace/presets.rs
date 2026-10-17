//! Named layouts: the screen's built-in ones and those the user saved (WS-05; "bake mode" is
//! one of them, ADR 0008 VQ18).

use std::collections::BTreeMap;

use craft_ui::docking::Layout;
use eframe::egui::Id;

use super::dock::Workspace;
use super::panel::PanelKind;
use crate::gui_settings::SavedLayout;

/// A built-in layout.
pub struct Preset<P> {
    pub name: &'static str,
    pub build: fn() -> Layout<P>,
}

/// Every layout name to offer: built-ins first, then the user's own in name order.
pub fn names<P>(presets: &[Preset<P>], saved: &BTreeMap<String, SavedLayout>) -> Vec<String> {
    let mut out: Vec<String> = presets.iter().map(|p| p.name.to_owned()).collect();
    out.extend(saved.keys().filter(|k| !presets.iter().any(|p| p.name == k.as_str())).cloned());
    out
}

/// The layout called `name`: a saved one (which may override a built-in), else the built-in.
/// A saved layout that no longer loads falls back to the built-in of that name, if any.
pub fn find<P: PanelKind>(area: Id, name: &str, presets: &[Preset<P>], saved: &BTreeMap<String, SavedLayout>) -> Option<Workspace<P>> {
    if let Some(ws) = saved.get(name).and_then(|s| Workspace::from_saved(area, s).ok()) {
        return Some(ws);
    }
    presets.iter().find(|p| p.name == name).and_then(|p| Workspace::new(area, (p.build)()).ok())
}
