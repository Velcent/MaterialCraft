//! The volume of the volume modes (PJ-09 … PJ-11) and each mode's own settings: the tri-planar blend (PJ-16), the
//! planar depth and facing falloffs (PJ-17), the cylindrical coverage, facing falloff and caps (PJ-19), the warp
//! grid (PJ-20: depth, size while regular, resetting points and handles, and the switch that puts its points in
//! the view to drag), and fill symmetry (PJ-22), which takes over the symmetry tool's settings.

use super::heading;
use crate::gui_paint::projection_edit::GizmoSettings;
use crate::gui_paint::tools::helpers::SymmetrySettings;
use eframe::egui::{Button, DragValue, Slider, Ui};
use materialcraft::projection::warp::MAX_SIDE;
use materialcraft::projection::{FacingFalloff, Falloff, Projection, ProjectionMode, Volume, WarpGrid};
use materialcraft::symmetry::SymmetrySpec;

const AXES: [&str; 3] = ["x", "y", "z"];

pub fn show(ui: &mut Ui, p: &mut Projection, symmetry: SymmetrySettings, gizmo: &mut GizmoSettings) {
    if let Some(v) = p.mode.volume_mut() {
        heading(ui, "Volume");
        volume(ui, v);
    }
    match &mut p.mode {
        ProjectionMode::TriPlanar(m) => {
            ui.add(Slider::new(&mut m.blend_hardness, 0.0..=1.0).text("Blend hardness"));
        }
        ProjectionMode::Planar(m) => {
            depth_falloff(ui, &mut m.depth_falloff, "Depth falloff");
            facing_falloff(ui, &mut m.facing_falloff);
        }
        ProjectionMode::Cylindrical(m) => {
            ui.add(Slider::new(&mut m.coverage_angle, 1.0..=360.0).suffix("°").text("Coverage"));
            ui.checkbox(&mut m.hide_caps, "Hide the caps");
            facing_falloff(ui, &mut m.facing_falloff);
        }
        ProjectionMode::Warp(m) => {
            ui.horizontal(|ui| {
                ui.label("Projection depth");
                ui.add(DragValue::new(&mut m.grid.depth).speed(0.005).range(0.0..=f32::MAX).max_decimals(4));
            });
            depth_falloff(ui, &mut m.depth_falloff, "Depth falloff");
            warp_grid(ui, &mut m.grid, gizmo);
        }
        _ => {}
    }
    if p.mode.takes_symmetry() {
        fill_symmetry(ui, p, symmetry);
    }
}

fn volume(ui: &mut Ui, v: &mut Volume) {
    let row = |ui: &mut Ui, label: &str, values: &mut [f32; 3], speed: f64, suffix: &str| {
        ui.horizontal(|ui| {
            ui.label(label);
            for (k, value) in values.iter_mut().enumerate() {
                ui.add(DragValue::new(value).speed(speed).prefix(format!("{} ", AXES.get(k).copied().unwrap_or(""))).suffix(suffix).max_decimals(4));
            }
        });
    };
    row(ui, "Offset", &mut v.offset, 0.005, "");
    row(ui, "Rotation", &mut v.rotation, 0.5, "°");
    row(ui, "Scale", &mut v.scale, 0.005, "");
    ui.horizontal(|ui| {
        ui.label("Mirror");
        for (k, name) in AXES.iter().enumerate() {
            if ui.button(name.to_uppercase()).on_hover_text("Mirror the volume along this axis").clicked()
                && let Some(s) = v.scale.get_mut(k)
            {
                *s = -*s;
            }
        }
        if ui.button("Reset").on_hover_text("Centred on the scene, unturned, enclosing it").clicked() {
            *v = Volume { clip: v.clip, ..Volume::default() };
        }
    });
    ui.checkbox(&mut v.clip, "Clip to the volume").on_hover_text("Stop the projection at the volume's edge instead of continuing beyond it");
}

/// The warp grid: its size (only while no point was moved), resets, and dragging its points in the view.
fn warp_grid(ui: &mut Ui, g: &mut WarpGrid, gizmo: &mut GizmoSettings) {
    heading(ui, "Warp grid");
    let regular = g.is_regular();
    ui.horizontal(|ui| {
        let (mut rows, mut columns) = (g.rows, g.columns);
        ui.add_enabled_ui(regular, |ui| {
            ui.label("Rows");
            ui.add(DragValue::new(&mut rows).range(2..=MAX_SIDE));
            ui.label("Columns");
            ui.add(DragValue::new(&mut columns).range(2..=MAX_SIDE));
        })
        .response
        .on_disabled_hover_text("The size is fixed once a point was moved; reset the points to change it");
        if (rows, columns) != (g.rows, g.columns)
            && let Some(resized) = g.resized(rows, columns)
        {
            *g = resized;
        }
    });
    ui.checkbox(&mut gizmo.warp_points, "Drag the grid points in the view")
        .on_hover_text("Shows the grid's points instead of the volume gizmo; each drag is one undo step");
    ui.horizontal(|ui| {
        if ui.add_enabled(!regular, Button::new("Reset points")).on_hover_text("Back to the regular grid of this size").clicked() {
            *g = g.reset_points();
        }
        let bent = g.points.iter().any(|p| p.tangents != [[0.0; 3]; 4]);
        if ui.add_enabled(bent, Button::new("Automatic handles")).on_hover_text("Smooth handles through every point").clicked() {
            *g = g.automatic_tangents();
        }
    });
}

fn depth_falloff(ui: &mut Ui, f: &mut Falloff, label: &str) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut f.on, label);
        ui.add_enabled(f.on, Slider::new(&mut f.hardness, 0.0..=1.0).text("hardness"));
    });
}

fn facing_falloff(ui: &mut Ui, f: &mut FacingFalloff) {
    ui.checkbox(&mut f.on, "Facing cut-off").on_hover_text("Hide the projection on faces turned away from it");
    ui.add_enabled_ui(f.on, |ui| {
        ui.add(Slider::new(&mut f.angle, 0.0..=180.0).suffix("°").text("angle"));
        ui.add(Slider::new(&mut f.hardness, 0.0..=1.0).text("hardness"));
    });
}

/// Fill symmetry (PJ-22): the fill repeated at the symmetry tool's copies.
fn fill_symmetry(ui: &mut Ui, p: &mut Projection, tool: SymmetrySettings) {
    heading(ui, "Symmetry");
    let summary = match p.symmetry {
        SymmetrySpec::None => "Off".to_string(),
        SymmetrySpec::Mirror { axis, offset } => format!("Mirrored across {} at {offset:.3}", AXES.get(axis).copied().unwrap_or("?").to_uppercase()),
        SymmetrySpec::Radial { axis, count, .. } => format!("{count} copies around {}", AXES.get(axis).copied().unwrap_or("?").to_uppercase()),
    };
    ui.label(summary);
    ui.horizontal(|ui| {
        if ui.button("Use the symmetry settings").on_hover_text("Repeat the fill as the symmetry tool's settings say (mirror or radial)").clicked() {
            p.symmetry = SymmetrySettings { on: true, ..tool }.spec();
        }
        if ui.add_enabled(p.symmetry != SymmetrySpec::None, eframe::egui::Button::new("Off")).clicked() {
            p.symmetry = SymmetrySpec::None;
        }
    });
}
