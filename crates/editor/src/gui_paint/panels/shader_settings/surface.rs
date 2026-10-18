//! Displacement (VS-07, VS-08) and tessellation (VS-09 … VS-11) of the instance, with the mesh's length unit.

use super::{ShaderEvent, edit, heading};
use crate::gui_paint::scene_shading::{ModelScale, plan};
use crate::gui_theme;
use crate::gui_widgets::SliderRow;
use eframe::egui::{self, Ui};
use materialcraft::mesh::LengthUnit;
use materialcraft::shading::{
    Displacement, DisplacementSource, DisplacementUnit, MAX_LEVELS, SceneScale, ShaderEdit, ShaderInstance, TessellationMode, WARN_LEVELS,
};

pub fn show(ui: &mut Ui, edited: &mut ShaderInstance, mesh_unit: Option<LengthUnit>, model: &ModelScale, out: &mut Vec<ShaderEvent>) {
    heading(ui, "Displacement");
    displacement(ui, &mut edited.displacement, mesh_unit, model, out);
    heading(ui, "Tessellation");
    tessellation(ui, edited, model);
}

fn combo<T: Copy + PartialEq>(ui: &mut Ui, id: &str, label: &str, value: &mut T, choices: &[(T, &str)]) {
    ui.horizontal(|ui| {
        ui.label(label);
        let current = choices.iter().find(|(v, _)| v == value).map_or("?", |(_, l)| *l);
        egui::ComboBox::from_id_salt(id).width(140.0).selected_text(current).show_ui(ui, |ui| {
            for (v, l) in choices {
                ui.selectable_value(value, *v, *l);
            }
        });
    });
}

fn displacement(ui: &mut Ui, d: &mut Displacement, mesh_unit: Option<LengthUnit>, model: &ModelScale, out: &mut Vec<ShaderEvent>) {
    ui.checkbox(&mut d.enabled, "Displace the surface").on_hover_text("Off: the display settings' height view applies");
    if !d.enabled {
        return;
    }
    combo(ui, "shader_disp_source", "Channel", &mut d.source, &DisplacementSource::ALL);
    combo(ui, "shader_disp_unit", "Unit", &mut d.unit, &DisplacementUnit::ALL);
    let (range, suffix) = match d.unit {
        DisplacementUnit::Relative => (0.0..=0.2, " of size"),
        DisplacementUnit::SceneUnits => (0.0..=(model.diagonal * 0.2).max(1e-3), " units"),
        DisplacementUnit::Centimetres => (0.0..=50.0, " cm"),
    };
    SliderRow::new("Scale", &mut d.scale, range).decimals(4).suffix(suffix).default_value(Displacement::default().scale).show(ui);
    SliderRow::new("Midlevel", &mut d.midlevel, 0.0..=1.0).default_value(0.0).show(ui);
    if d.unit == DisplacementUnit::Centimetres {
        let mut unit = mesh_unit;
        let mut choices: Vec<(Option<LengthUnit>, &str)> = vec![(None, "Unknown")];
        choices.extend(LengthUnit::ALL.iter().map(|(u, l)| (Some(*u), *l)));
        combo(ui, "shader_mesh_unit", "Mesh unit", &mut unit, &choices);
        if unit != mesh_unit {
            edit(out, "Mesh unit", ShaderEdit::SetMeshUnit(unit), false);
        }
        let scene = SceneScale { diagonal: model.diagonal, unit: mesh_unit };
        if let (_, Some(w)) = d.distance(&scene) {
            ui.colored_label(gui_theme::ACCENT, w.message());
        }
    }
}

fn tessellation(ui: &mut Ui, edited: &mut ShaderInstance, model: &ModelScale) {
    let t = &mut edited.tessellation;
    let kinds = [(0u8, "Off"), (1, "Uniform"), (2, "By edge length")];
    let mut kind = match t.mode {
        TessellationMode::Off => 0u8,
        TessellationMode::Uniform { .. } => 1,
        TessellationMode::EdgeLength { .. } => 2,
    };
    let before = kind;
    combo(ui, "shader_tess_mode", "Mode", &mut kind, &kinds);
    if kind != before {
        t.mode = match kind {
            1 => TessellationMode::Uniform { levels: 1 },
            2 => TessellationMode::EdgeLength { max_relative: 0.01 },
            _ => TessellationMode::Off,
        };
    }
    match &mut t.mode {
        TessellationMode::Off => return,
        TessellationMode::Uniform { levels } => {
            // each level doubles the segments per original edge: 2, 4, 8, 16, 32
            let mut lv = (*levels).clamp(1, MAX_LEVELS) as f32;
            SliderRow::new("Levels", &mut lv, 1.0..=MAX_LEVELS as f32).step(1.0).decimals(0).default_value(1.0).show(ui);
            *levels = (lv.round() as u32).clamp(1, MAX_LEVELS);
        }
        TessellationMode::EdgeLength { max_relative } => {
            let mut percent = *max_relative * 100.0;
            SliderRow::new("Longest edge", &mut percent, 0.1..=10.0).decimals(2).suffix(" % of size").default_value(1.0).show(ui);
            *max_relative = (percent / 100.0).clamp(0.001, 0.1);
        }
    }
    ui.checkbox(&mut t.smooth, "Smooth (round off the surface)");
    let p = plan(edited, model);
    ui.weak(format!("{} → {} triangles ({} per original edge)", model.triangles, p.triangles, 1u32 << p.levels));
    if p.limited {
        ui.colored_label(gui_theme::ACCENT, "Limited by the triangle budget: fewer levels are used than asked for.");
    } else if p.levels >= WARN_LEVELS {
        ui.colored_label(gui_theme::ACCENT, "High subdivision: the viewport may become slow.");
    }
}
