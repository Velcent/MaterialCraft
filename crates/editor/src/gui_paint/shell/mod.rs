//! [`Shell`]: the Paint screen's workspace around the document: docking and named layouts,
//! the shortcut table, the chosen tool and the user settings they are saved in. `PaintMode`
//! hands it the workspace events; document events stay with `PaintMode`.

pub mod actions;
pub mod panels;
pub mod tools;

use std::sync::Arc;

use eframe::egui::{self, Id};
use materialcraft::diag::{Area, Sink, SinkExt};

use super::panels::{PanelEvent, Strip};
use crate::gui_settings::{Side, UserSettings};
use crate::gui_workspace::shortcuts::{PageEdit, ShortcutTable, ShortcutsPage};
use crate::gui_workspace::{Workspace, presets};
use panels::{DEFAULT_LAYOUT, PRESETS, PaintPanel};
use tools::ToolState;

/// The Paint screen's workspace state.
pub struct Shell {
    pub settings: UserSettings,
    pub workspace: Workspace<PaintPanel>,
    /// The name of the layout the workspace started from.
    pub layout_name: String,
    pub shortcuts: ShortcutTable,
    pub page: ShortcutsPage,
    pub tools: ToolState,
    /// The interface scale while its slider is dragged; applied when the drag ends.
    pub scale_draft: f32,
    diag: Arc<dyn Sink>,
}

fn dock_id() -> Id {
    Id::new("paint_dock")
}

impl Shell {
    /// The workspace as the user left it, else the named or default layout.
    pub fn new(settings: UserSettings, diag: Arc<dyn Sink>) -> Shell {
        let s = settings.get();
        let (shortcuts, problems) = ShortcutTable::new(actions::ACTIONS, &s.shortcuts.bindings);
        for p in problems {
            diag.warn(Area::General, p);
        }
        let name = if s.workspace.current_name.is_empty() { DEFAULT_LAYOUT.to_owned() } else { s.workspace.current_name.clone() };
        let restored = s.workspace.current.as_ref().and_then(|c| match Workspace::from_saved(dock_id(), c) {
            Ok(ws) => Some(ws),
            Err(e) => {
                diag.warn(Area::General, format!("The saved workspace could not be restored ({e}); the {name} layout is used."));
                None
            }
        });
        let workspace = restored
            .or_else(|| presets::find(dock_id(), &name, PRESETS, &s.workspace.saved))
            .or_else(|| presets::find(dock_id(), DEFAULT_LAYOUT, PRESETS, &s.workspace.saved))
            .unwrap_or_else(|| Workspace::minimal(dock_id()));
        let scale_draft = s.interface.scale();
        Shell { settings, workspace, layout_name: name, shortcuts, page: ShortcutsPage::default(), tools: ToolState::default(), scale_draft, diag }
    }

    /// Once per frame: saves pending settings and applies the interface scale.
    pub fn frame(&mut self, ctx: &egui::Context) {
        if self.workspace.take_changed() {
            self.save_workspace();
        }
        if let Some(e) = self.settings.tick(ctx) {
            self.diag.warn(Area::General, e);
        }
        let scale = self.ui_scale();
        if (ctx.zoom_factor() - scale).abs() > 1e-3 {
            ctx.set_zoom_factor(scale);
        }
    }

    /// The interface scale from the settings.
    pub fn ui_scale(&self) -> f32 {
        self.settings.get().interface.scale()
    }

    /// The events of this frame's shortcut key presses (none while text is typed or a binding
    /// is recorded).
    pub fn take_shortcuts(&self, ctx: &egui::Context) -> Vec<PanelEvent> {
        if self.page.is_recording() || ctx.egui_wants_keyboard_input() {
            return Vec::new();
        }
        ctx.input_mut(|i| self.shortcuts.take_presses(i)).into_iter().filter_map(actions::event_for).collect()
    }

    /// Applies a workspace event; returns the event back if it is not one. `can_paint` says whether
    /// the selection is a paintable target (tools can only be chosen then).
    pub fn apply(&mut self, ev: PanelEvent, can_paint: bool) -> Option<PanelEvent> {
        match ev {
            PanelEvent::TogglePanel(p) => {
                let r = self.workspace.toggle(p).map_err(|e| e.to_string());
                self.report(r);
            }
            PanelEvent::Popover(p, rect) => {
                let r = self.workspace.toggle_popover(p, rect).map_err(|e| e.to_string());
                self.report(r);
            }
            PanelEvent::DockRefused(e) => self.report(Err(e)),
            PanelEvent::SwitchLayout(name) => self.switch_layout(&name),
            PanelEvent::ResetLayout => {
                let name = self.layout_name.clone();
                self.switch_layout(&name);
            }
            PanelEvent::SaveLayout(name) => self.save_layout_as(name),
            PanelEvent::DeleteLayout(name) => {
                self.settings.update(|s| {
                    s.workspace.saved.remove(&name);
                });
            }
            PanelEvent::MoveStrip(strip, side) => {
                self.settings.update(|s| match strip {
                    Strip::Tools => s.workspace.tools_side = side,
                    Strip::Panels => s.workspace.panels_side = side,
                });
            }
            PanelEvent::SelectTool(tool) => {
                if tools::unavailable(true, can_paint).is_none() {
                    self.tools.select(tool);
                }
            }
            PanelEvent::PreviousTool => {
                if tools::unavailable(true, can_paint).is_none() {
                    self.tools.select(self.tools.previous);
                }
            }
            PanelEvent::Shortcut(edit) => self.edit_shortcuts(edit),
            PanelEvent::SetUiScale(scale) => {
                self.settings.update(|s| s.interface.ui_scale = scale);
                self.scale_draft = self.ui_scale();
            }
            other => return Some(other),
        }
        None
    }

    /// The side of `strip`.
    pub fn strip_side(&self, strip: Strip) -> Side {
        let w = &self.settings.get().workspace;
        match strip {
            Strip::Tools => w.tools_side,
            Strip::Panels => w.panels_side,
        }
    }

    /// Every layout name to offer, and whether each is one the user saved.
    pub fn layout_names(&self) -> Vec<(String, bool)> {
        let saved = &self.settings.get().workspace.saved;
        presets::names(PRESETS, saved).into_iter().map(|n| (n.clone(), saved.contains_key(&n))).collect()
    }

    fn switch_layout(&mut self, name: &str) {
        match presets::find(dock_id(), name, PRESETS, &self.settings.get().workspace.saved) {
            Some(ws) => {
                self.workspace = ws;
                self.layout_name = name.to_owned();
                self.save_workspace();
            }
            None => self.diag.warn(Area::General, format!("There is no layout called {name}.")),
        }
    }

    fn save_layout_as(&mut self, name: String) {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let saved = self.workspace.to_saved();
        self.settings.update(|s| {
            s.workspace.saved.insert(name.clone(), saved);
        });
        self.layout_name = name;
        self.save_workspace();
    }

    fn save_workspace(&mut self) {
        let current = self.workspace.to_saved();
        let name = self.layout_name.clone();
        self.settings.update(|s| {
            s.workspace.current = Some(current);
            s.workspace.current_name = name;
        });
    }

    fn edit_shortcuts(&mut self, edit: PageEdit) {
        match edit {
            PageEdit::Set(id, chords) => {
                self.shortcuts.set(id, chords);
            }
            PageEdit::Reset(id) => self.shortcuts.reset(id),
            PageEdit::ResetAll => self.shortcuts.reset_all(),
        }
        let overrides = self.shortcuts.overrides();
        self.settings.update(|s| s.shortcuts.bindings = overrides);
    }

    fn report(&self, result: Result<(), String>) {
        if let Err(e) = result {
            self.diag.warn(Area::General, format!("That panel can't be moved there: {e}."));
        }
    }

    /// Saves the settings now (on exit).
    pub fn flush(&mut self) {
        if self.workspace.take_changed() {
            self.save_workspace();
        }
        if let Err(e) = self.settings.flush() {
            self.diag.warn(Area::General, e);
        }
    }
}

#[cfg(test)]
mod tests;
