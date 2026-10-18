//! The shader settings panel (roadmap step 41, VS-02 … VS-12): the shader instance of the focused texture set.
//!
//! - Header (VS-04): undo and redo of the settings' own history (VS-06), the instance the set uses (a new one,
//!   another one, or the built-in defaults; sets can share one, VS-03), its name, its shader file, restore
//!   defaults, and the outdated flag with its update action (VS-05).
//! - [`params`]: the parameters the shader file declares, by group (VS-02).
//! - [`surface`]: displacement (channel, scale, unit, midlevel, the mesh's unit; VS-07, VS-08) and tessellation
//!   (levels or edge length, smoothing, the triangle count; VS-09 … VS-11).
//!
//! It changes nothing itself: every change goes out as a [`ShaderEvent`], which `PaintMode` applies to the
//! shader settings' history (a slider drag as one step).

mod params;
mod surface;

use super::PanelEvent;
use crate::gui_paint::scene_shading::ModelScale;
use crate::gui_theme;
use eframe::egui::{self, RichText, Ui};
use materialcraft::history::ShaderHistory;
use materialcraft::paint::{Document, TextureSet};
use materialcraft::shading::{ShaderEdit, ShaderInstance, ShaderLibrary, ShaderSettings};

/// What the panel asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum ShaderEvent {
    /// Apply `edit` as a step named `label`; with `gesture` (a slider drag, typing) it joins the running step.
    Edit {
        label: &'static str,
        edit: Box<ShaderEdit>,
        gesture: bool,
    },
    /// The drag or typing ended.
    EndGesture,
    Undo,
    Redo,
}

/// What the panel shows this frame.
pub struct ShaderUi<'a> {
    pub doc: &'a Document,
    /// The focused texture set.
    pub set: Option<&'a TextureSet>,
    pub history: &'a ShaderHistory,
    pub library: &'a ShaderLibrary,
    pub model: &'a ModelScale,
}

pub fn show(ui: &mut Ui, p: ShaderUi, events: &mut Vec<PanelEvent>) {
    let mut out = Vec::new();
    history_buttons(ui, p.history, &mut out);
    egui::ScrollArea::vertical().id_salt("paint_shader_settings").show(ui, |ui| match p.set {
        None => {
            ui.weak("Select a texture set to see its shader.");
        }
        Some(set) => body(ui, &p, set, &mut out),
    });
    if ui.input(|i| i.pointer.any_released()) {
        out.push(ShaderEvent::EndGesture);
    }
    events.extend(out.into_iter().map(PanelEvent::Shader));
}

fn history_buttons(ui: &mut Ui, h: &ShaderHistory, out: &mut Vec<ShaderEvent>) {
    ui.horizontal(|ui| {
        let undo = ui.add_enabled(h.can_undo(), egui::Button::new("\u{21B6} Undo"));
        let undo = match h.undo_label() {
            Some(l) => undo.on_hover_text(format!("Undo {l} (shader settings only)")),
            None => undo,
        };
        if undo.clicked() {
            out.push(ShaderEvent::Undo);
        }
        let redo = ui.add_enabled(h.can_redo(), egui::Button::new("\u{21B7} Redo"));
        let redo = match h.redo_label() {
            Some(l) => redo.on_hover_text(format!("Redo {l} (shader settings only)")),
            None => redo,
        };
        if redo.clicked() {
            out.push(ShaderEvent::Redo);
        }
    });
}

fn edit(out: &mut Vec<ShaderEvent>, label: &'static str, edit: ShaderEdit, gesture: bool) {
    out.push(ShaderEvent::Edit { label, edit: Box::new(edit), gesture });
}

fn body(ui: &mut Ui, p: &ShaderUi, set: &TextureSet, out: &mut Vec<ShaderEvent>) {
    let settings = &p.doc.shading;
    ui.label(RichText::new(format!("Texture set “{}”", set.name)).color(gui_theme::LABEL_TEXT));
    instance_choice(ui, settings, set, p.library, out);
    let Some(instance) = p.doc.shader_instance(&set.id) else {
        ui.weak("This texture set is shaded with the built-in shader and its defaults. Make an instance to change them.");
        return;
    };
    let file = p.library.find_or_default(&instance.shader);
    let mut edited = instance.clone();
    header(ui, &mut edited, p.library, out);
    if file.id() != instance.shader {
        ui.colored_label(gui_theme::ACCENT, format!("The shader file {:?} isn't installed; the built-in shader is used.", instance.shader));
    }
    params::show(ui, &file.decl, &mut edited);
    surface::show(ui, &mut edited, settings.mesh_unit, p.model, out);
    let gesture = ui.input(|i| i.pointer.any_down()) || ui.memory(|m| m.focused().is_some());
    diff(instance, &edited, gesture, out);
}

/// Which instance the set uses: the built-in defaults or one of the document's, a new one, or deleting it.
fn instance_choice(ui: &mut Ui, settings: &ShaderSettings, set: &TextureSet, library: &ShaderLibrary, out: &mut Vec<ShaderEvent>) {
    let current = settings.instance_for(&set.id);
    ui.horizontal(|ui| {
        ui.label("Instance");
        let text = current.map_or("Built-in defaults", |i| i.name.as_str());
        egui::ComboBox::from_id_salt("shader_instance").width(150.0).selected_text(text).show_ui(ui, |ui| {
            if ui.selectable_label(current.is_none(), "Built-in defaults").clicked() && current.is_some() {
                edit(out, "Use the built-in defaults", ShaderEdit::Assign { set: set.id.clone(), instance: None }, false);
            }
            for i in &settings.instances {
                let users = settings.sets_using(&i.id).len();
                let label = if users > 0 { format!("{} ({users})", i.name) } else { i.name.clone() };
                if ui.selectable_label(current.is_some_and(|c| c.id == i.id), label).clicked() && current.is_none_or(|c| c.id != i.id) {
                    edit(out, "Use shader instance", ShaderEdit::Assign { set: set.id.clone(), instance: Some(i.id.clone()) }, false);
                }
            }
        });
        if ui.button("New").on_hover_text("A new instance for this texture set, starting from its current values").clicked() {
            let id = settings.new_instance_id();
            let base = match current {
                Some(c) => ShaderInstance { id: id.clone(), name: format!("{} copy", c.name), ..c.clone() },
                None => ShaderInstance::new(id.clone(), set.name.clone(), &library.default_shader()),
            };
            let at = settings.instances.len();
            edit(out, "New shader instance", ShaderEdit::Insert { at, instance: Box::new(base), sets: vec![set.id.clone()] }, false);
        }
        if let Some(c) = current
            && ui.button("Delete").on_hover_text("Delete this instance; the texture sets using it go back to the built-in defaults").clicked()
        {
            edit(out, "Delete shader instance", ShaderEdit::Remove { instance: c.id.clone(), restore: Vec::new() }, false);
        }
    });
    if let Some(c) = current {
        let users: Vec<&str> = settings.sets_using(&c.id);
        if users.len() > 1 {
            ui.weak(format!("Shared by {} texture sets.", users.len()));
        }
    }
}

/// Name, shader file, restore defaults, outdated flag.
fn header(ui: &mut Ui, edited: &mut ShaderInstance, library: &ShaderLibrary, out: &mut Vec<ShaderEvent>) {
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut edited.name).desired_width(160.0));
    });
    let current = library.find(&edited.shader);
    ui.horizontal(|ui| {
        ui.label("Shader");
        let text = current.map_or(edited.shader.as_str(), |f| f.decl.name.as_str()).to_string();
        egui::ComboBox::from_id_salt("shader_file").width(150.0).selected_text(text).show_ui(ui, |ui| {
            for f in library.files() {
                if ui.selectable_label(f.id() == edited.shader, f.decl.name.as_str()).clicked() && f.id() != edited.shader {
                    edit(out, "Change shader file", ShaderEdit::Replace { instance: Box::new(edited.updated_to(f)) }, false);
                }
            }
        });
        if let Some(f) = current
            && ui.button("Restore defaults").clicked()
        {
            edit(out, "Restore shader defaults", ShaderEdit::Replace { instance: Box::new(edited.with_defaults(f)) }, false);
        }
    });
    if let Some(f) = current
        && edited.is_outdated(f)
    {
        ui.horizontal(|ui| {
            ui.colored_label(gui_theme::ACCENT, "The shader file changed since these values were set.");
            if ui.button("Update").clicked() {
                edit(out, "Update shader instance", ShaderEdit::Replace { instance: Box::new(edited.updated_to(f)) }, false);
            }
        });
    }
}

/// The edits that turn `old` into `new`.
fn diff(old: &ShaderInstance, new: &ShaderInstance, gesture: bool, out: &mut Vec<ShaderEvent>) {
    let id = || old.id.clone();
    if new.name != old.name {
        edit(out, "Rename shader instance", ShaderEdit::Rename { instance: id(), name: new.name.clone() }, gesture);
    }
    for (param, value) in &new.values {
        if old.stored(param) != Some(*value) {
            edit(out, "Shader parameter", ShaderEdit::SetValue { instance: id(), param: param.clone(), value: Some(*value) }, gesture);
        }
    }
    if new.displacement != old.displacement {
        edit(out, "Displacement", ShaderEdit::SetDisplacement { instance: id(), value: new.displacement }, gesture);
    }
    if new.tessellation != old.tessellation {
        edit(out, "Tessellation", ShaderEdit::SetTessellation { instance: id(), value: new.tessellation }, gesture);
    }
}

pub(super) fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).strong().color(gui_theme::HEADER_TEXT));
}
