//! The parameters a shader file declares, by group, with the widget their type asks for: sliders for numbers,
//! check boxes, colour buttons and option lists (VS-02).

use super::heading;
use crate::gui_widgets::SliderRow;
use eframe::egui::{self, Ui};
use materialcraft::shading::{ParamDecl, ParamKind, ParamValue, ShaderDecl, ShaderInstance};

pub fn show(ui: &mut Ui, decl: &ShaderDecl, edited: &mut ShaderInstance) {
    for group in decl.groups() {
        heading(ui, if group.is_empty() { "Parameters" } else { group });
        for p in decl.params.iter().filter(|p| p.group == group) {
            let before = edited.value(p);
            let after = widget(ui, p, before);
            if after != before {
                edited.set_value(&p.id, Some(after));
            }
        }
    }
}

/// One parameter's widget; returns the (possibly changed) value.
fn widget(ui: &mut Ui, p: &ParamDecl, value: ParamValue) -> ParamValue {
    match (&p.kind, value) {
        (ParamKind::Float { min, max, default, step }, ParamValue::Float(mut v)) => {
            let mut row = SliderRow::new(&p.label, &mut v, *min..=*max).default_value(*default);
            if let Some(s) = step {
                row = row.step(*s);
            }
            row.show(ui);
            ParamValue::Float(v)
        }
        (ParamKind::Int { min, max, default }, ParamValue::Int(i)) => {
            let mut v = i as f32;
            SliderRow::new(&p.label, &mut v, *min as f32..=*max as f32).step(1.0).decimals(0).default_value(*default as f32).show(ui);
            ParamValue::Int((v.round() as i32).clamp(*min, *max))
        }
        (ParamKind::Bool { .. }, ParamValue::Bool(mut b)) => {
            ui.checkbox(&mut b, p.label.as_str());
            ParamValue::Bool(b)
        }
        (ParamKind::Color { .. }, ParamValue::Color(mut c)) => {
            ui.horizontal(|ui| {
                ui.color_edit_button_rgb(&mut c);
                ui.label(p.label.as_str());
            });
            ParamValue::Color(c)
        }
        (ParamKind::Choice { options, .. }, ParamValue::Int(i)) => {
            let mut chosen = i;
            ui.horizontal(|ui| {
                ui.label(p.label.as_str());
                let text = usize::try_from(i).ok().and_then(|i| options.get(i)).map_or("?", String::as_str);
                egui::ComboBox::from_id_salt(("shader_param", p.id.as_str())).selected_text(text).show_ui(ui, |ui| {
                    for (k, o) in options.iter().enumerate() {
                        let k = k as i32;
                        ui.selectable_value(&mut chosen, k, o.as_str());
                    }
                });
            });
            ParamValue::Int(chosen)
        }
        // `ShaderInstance::value` makes values match their declaration; anything else is shown as nothing.
        _ => value,
    }
}
