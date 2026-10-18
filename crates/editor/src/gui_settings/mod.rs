//! The editor's user settings, stored through craft-settings in `config.toml` (TOML, hand-editable,
//! atomic saves, damaged files moved aside). Layouts, shortcuts, interface, library roots and
//! saved searches, swatches, the Paint viewport's display defaults. Headless code gets plain values from here, never the store.
//!
//! Where the file lives: `MATERIALCRAFT_CONFIG_DIR` if set (empty = in memory only, for tests),
//! a `portable` marker next to the executable, else the platform folder
//! (`~/.config/materialcraft/` on Linux).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod display;
pub mod sections;

use std::path::Path;
use std::time::Instant;

use craft_settings::{Location, SaveStatus, Settings, SettingsStore};
use eframe::egui;
use serde::{Deserialize, Serialize};

pub use sections::{
    ColorPickerSettings, ColorSliders, ColorUnits, InterfaceSettings, LibrarySettings, MissingMaps, PenSettings, PromptSettings, SavedLayout, SavedSearch,
    ShortcutSettings, Side, SwatchSettings, ThumbSize, TrayLayout, TraySettings, WorkspaceSettings,
};

/// Everything the editor remembers between runs. Each field is one `[section]` of the file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorSettings {
    pub workspace: WorkspaceSettings,
    pub shortcuts: ShortcutSettings,
    pub interface: InterfaceSettings,
    pub library: LibrarySettings,
    pub swatches: SwatchSettings,
    pub color_picker: ColorPickerSettings,
    pub prompts: PromptSettings,
    pub pen: PenSettings,
    pub display: display::DisplaySettings,
}

impl Settings for EditorSettings {
    const APP_ID: &'static str = "materialcraft";
    const SCHEMA_VERSION: u32 = 1;
}

/// The open settings store. Reads are in memory; changes are saved shortly after they happen
/// (call [`UserSettings::tick`] every frame) and on exit ([`UserSettings::flush`]).
pub struct UserSettings {
    store: SettingsStore<EditorSettings>,
}

impl UserSettings {
    /// Opens the store at `location`. Never fails: the returned messages describe anything the
    /// user should know (a damaged file moved aside, a section reset, a read-only folder).
    pub fn open(location: Location) -> (UserSettings, Vec<String>) {
        let (store, report) = SettingsStore::open(location);
        let mut problems = Vec::new();
        if let Some(p) = &report.broken_moved_to {
            problems.push(format!("The settings file was damaged and was moved to {}; defaults are used.", p.display()));
        }
        for e in &report.section_errors {
            problems.push(format!("Settings section [{}] was reset to its defaults: {}", e.section, e.message));
        }
        if let Some(why) = &report.read_only {
            problems.push(format!("Settings will not be saved: {why}"));
        }
        (UserSettings { store }, problems)
    }

    /// The folder holding `config.toml`; `None` when settings live in memory only.
    pub fn dir(&self) -> Option<&Path> {
        self.store.dir()
    }

    pub fn get(&self) -> &EditorSettings {
        self.store.get()
    }

    /// Changes the settings; they are saved after a short pause. Returns true if anything changed.
    pub fn update(&mut self, f: impl FnOnce(&mut EditorSettings)) -> bool {
        self.store.update(f)
    }

    /// Saves pending changes when due and asks for a repaint at the next deadline, so an idle
    /// window still saves. Returns a save error to report (the store keeps retrying).
    pub fn tick(&mut self, ctx: &egui::Context) -> Option<String> {
        let now = Instant::now();
        let tick = self.store.tick(now);
        if let Some(deadline) = tick.next_deadline {
            ctx.request_repaint_after(deadline.saturating_duration_since(now));
        }
        match tick.status {
            SaveStatus::Failed(e) => Some(e.to_string()),
            _ => None,
        }
    }

    /// Saves now (on exit).
    pub fn flush(&mut self) -> Result<(), String> {
        self.store.flush().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_ui::docking::{Layout, Node};
    use std::collections::BTreeMap;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("materialcraft-settings-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn every_section_survives_a_save_and_reopen() {
        let dir = temp_dir("roundtrip");
        let layout = Layout { root: Some(Node::Tabs { panels: vec!["viewport".to_string()], active: 0 }), floating: Vec::new() };
        let mut want = EditorSettings::default();
        want.workspace.current = Some(SavedLayout { layout, hidden: BTreeMap::new() });
        want.workspace.current_name = "Painting".into();
        want.workspace.panels_side = Side::Left;
        want.shortcuts.bindings.insert("paint.tool.paint".into(), vec!["B".into()]);
        want.interface.ui_scale = 1.25;
        want.library.roots.push("/tmp/assets".into());
        want.library.saved_searches.push(sections::SavedSearch {
            name: "rust".into(),
            query: "!rust".into(),
            usages: vec!["material".into()],
            library: "user".into(),
            folder: "materials".into(),
            tab: true,
        });
        want.library.tray = TraySettings { layout: TrayLayout::Narrow, thumbnails: ThumbSize::List, hide_empty: true };
        want.swatches.colors.push([0.5, 0.25, 0.0, 1.0]);
        want.color_picker = ColorPickerSettings { sliders: ColorSliders::Rgb, units: ColorUnits::Float, tinted: false };
        want.prompts.missing_maps = MissingMaps::Skip;
        want.pen = PenSettings { use_pressure: false, pressure_curve: vec![[0.0, 0.0], [0.25, 0.5], [1.0, 1.0]] };
        want.display.tone_curve = "aces".into();
        want.display.project.exposure_stops = -1.5;
        {
            let (mut s, problems) = UserSettings::open(Location::Dir(dir.clone()));
            assert!(problems.is_empty(), "{problems:?}");
            s.update(|v| *v = want.clone());
            s.flush().expect("flush");
        }
        let (s, problems) = UserSettings::open(Location::Dir(dir.clone()));
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(s.get(), &want);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_section_resets_alone_and_is_reported() {
        let dir = temp_dir("damaged");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("config.toml"), "schema_version = 1\n[interface]\nui_scale = \"big\"\n[shortcuts.bindings]\n\"paint.open\" = [\"Mod+O\"]\n")
            .expect("write");
        let (s, problems) = UserSettings::open(Location::Dir(dir.clone()));
        assert_eq!(s.get().interface, InterfaceSettings::default());
        assert_eq!(s.get().shortcuts.bindings.get("paint.open"), Some(&vec!["Mod+O".to_string()]));
        assert!(problems.iter().any(|p| p.contains("[interface]")), "{problems:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_scale_falls_back_to_one() {
        assert_eq!(InterfaceSettings { ui_scale: f32::NAN }.scale(), 1.0);
        assert_eq!(InterfaceSettings { ui_scale: 99.0 }.scale(), InterfaceSettings::MAX_SCALE);
    }
}
