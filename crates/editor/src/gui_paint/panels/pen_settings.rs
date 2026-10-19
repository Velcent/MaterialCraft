//! Pen settings (PT-07, PT-56): whether pen pressure is used and the pressure curve, chosen from presets (the
//! points stay hand-editable in `config.toml`, shown here as "Custom"), with a preview of the curve. Above them,
//! whether pen pressure reaches the brushes in this session, and on Linux where the window opens in a Wayland
//! session (step 58).

use eframe::egui::{self, Color32, RichText, Sense, Stroke, Ui, pos2, vec2};
use materialcraft_brush::ResponseCurve;

use super::PanelEvent;
use crate::gui_paint::input::pen_status::PenStatus;
use crate::gui_paint::input::pressure_curve::PressurePreset;
use crate::gui_paint::input::window_system::OVERRIDE_VAR;
use crate::gui_settings::{PenSettings, PenWindow};

/// Side of the curve preview, in points.
const PREVIEW: f32 = 72.0;

pub fn show(ui: &mut Ui, pen: &PenSettings, status: &PenStatus, events: &mut Vec<PanelEvent>) {
    ui.add_space(8.0);
    ui.strong("Pen");
    match status {
        PenStatus::Unavailable { .. } => {
            ui.label(RichText::new(status.summary()).color(ui.visuals().warn_fg_color));
        }
        PenStatus::Reading { .. } => {
            ui.weak(status.summary());
        }
        PenStatus::NotStarted => {}
    }
    if cfg!(target_os = "linux") {
        window(ui, pen, events);
    }
    let mut use_pressure = pen.use_pressure;
    if ui.checkbox(&mut use_pressure, "Use pen pressure, tilt and rotation").changed() {
        events.push(PanelEvent::SetPen(PenSettings { use_pressure, ..pen.clone() }));
    }
    ui.add_enabled_ui(pen.use_pressure, |ui| {
        let current = PressurePreset::of(&pen.pressure_curve);
        ui.horizontal(|ui| {
            ui.label("Pressure curve");
            egui::ComboBox::from_id_salt("paint_pen_curve").selected_text(current.map_or("Custom", PressurePreset::label)).show_ui(ui, |ui| {
                for p in PressurePreset::ALL {
                    if ui.selectable_label(current == Some(p), p.label()).clicked() && current != Some(p) {
                        events.push(PanelEvent::SetPen(PenSettings { pressure_curve: p.points(), ..pen.clone() }));
                    }
                }
            });
        });
        preview(ui, &pen.pressure_curve);
    });
}

/// One warning line when pen pressure doesn't reach the brushes (brush panel, under its pressure switches); the
/// full reason is in its tooltip and in the pen settings.
pub fn pressure_notice(ui: &mut Ui, status: &PenStatus) {
    if !status.available() {
        ui.label(RichText::new("No pen pressure in this session (see Interface settings → Pen).").small().color(ui.visuals().warn_fg_color))
            .on_hover_text(status.summary());
    }
}

/// Where the window opens in a Wayland session; read at start.
fn window(ui: &mut Ui, pen: &PenSettings, events: &mut Vec<PanelEvent>) {
    let label = |w: PenWindow| match w {
        PenWindow::Auto => "Automatic",
        PenWindow::Wayland => "Native Wayland",
        PenWindow::Xwayland => "Xwayland",
    };
    ui.horizontal(|ui| {
        ui.label("Window (Wayland)");
        egui::ComboBox::from_id_salt("paint_pen_window").selected_text(label(pen.window)).show_ui(ui, |ui| {
            for w in [PenWindow::Auto, PenWindow::Wayland, PenWindow::Xwayland] {
                if ui.selectable_label(pen.window == w, label(w)).clicked() && pen.window != w {
                    events.push(PanelEvent::SetPen(PenSettings { window: w, ..pen.clone() }));
                }
            }
        });
    })
    .response
    .on_hover_text(format!(
        "In a Wayland session only an Xwayland window gets pen pressure, tilt and rotation. Automatic opens through \
         Xwayland when a pen tablet is connected at start. Takes effect at the next start; {OVERRIDE_VAR}=wayland or \
         xwayland overrides it for one run."
    ));
}

/// The curve over pen pressure (x) → brush pressure (y), with its control points.
fn preview(ui: &mut Ui, points: &[[f32; 2]]) {
    let (rect, _) = ui.allocate_exact_size(vec2(PREVIEW, PREVIEW), Sense::hover());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, 2.0, visuals.extreme_bg_color);
    let at = |x: f32, y: f32| pos2(rect.left() + x * rect.width(), rect.bottom() - y * rect.height());
    painter.line_segment([at(0.0, 0.0), at(1.0, 1.0)], Stroke::new(1.0, visuals.weak_text_color().gamma_multiply(0.4)));
    let curve = ResponseCurve::new(points);
    let line: Vec<_> = (0..=32).map(|i| i as f32 / 32.0).map(|x| at(x, curve.eval(x))).collect();
    painter.add(egui::Shape::line(line, Stroke::new(1.5, visuals.selection.bg_fill)));
    for p in curve.points() {
        painter.circle_filled(at(p[0], p[1]), 2.5, Color32::from_gray(220));
    }
}
