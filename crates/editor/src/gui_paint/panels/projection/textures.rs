//! The fill's channel textures (roadmap step 48): for each channel a uniform or texture fill writes, a constant or
//! one of MaterialCraft's procedural textures with its shared parameters (scale, seed, disorder, tone). Picking a
//! texture turns a uniform fill into a texture fill that keeps the other channels' values. Material and image fills
//! get their channels elsewhere and are only named here. Edits a copy of the source; the caller sends the change.

use super::heading;
use eframe::egui::{self, DragValue, Slider, Ui};
use materialcraft::paint::effect::Procedural;
use materialcraft::paint::{Channel, FillSource, TextureFill};
use materialcraft::proc_textures::{self, COMMON_PARAMS, ParamKind};

/// The value a channel switched from a texture back to a constant starts at.
const MID_GREY: [f32; 4] = [0.5, 0.5, 0.5, 1.0];

/// What a channel takes its value from.
#[derive(Clone, Copy, PartialEq)]
enum Pick<'a> {
    Value,
    Texture(&'a str),
}

pub fn show(ui: &mut Ui, source: &mut FillSource, original: Option<&Procedural>) {
    heading(ui, "Channel textures");
    if let Some(o) = original {
        let name = o.id.strip_prefix("substance:").unwrap_or(&o.id);
        ui.weak(format!("Stands in for the imported procedural “{name}”."));
    }
    let channels = match &*source {
        FillSource::Uniform(values) => values.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        FillSource::Texture(t) => t.channels(),
        FillSource::Material { .. } => {
            ui.weak("A material fill: its channels come from the material.");
            return;
        }
        FillSource::Image(_) => {
            ui.weak("An image fill: its channels come from images.");
            return;
        }
    };
    if channels.is_empty() {
        ui.weak("This fill sets no channel.");
    }
    for c in channels {
        channel_row(ui, source, c);
    }
}

fn channel_row(ui: &mut Ui, source: &mut FillSource, c: Channel) {
    let current = match &*source {
        FillSource::Texture(t) => t.texture(c).cloned(),
        _ => None,
    };
    let selected = current.as_ref().map_or(Pick::Value, |t| Pick::Texture(t.id.as_str()));
    let mut chosen = selected;
    ui.horizontal(|ui| {
        ui.label(label(c));
        let text = match selected {
            Pick::Value => "Value".to_string(),
            Pick::Texture(id) => proc_textures::info(id).map_or(id.to_string(), |i| i.label.to_string()),
        };
        egui::ComboBox::from_id_salt(("fill_texture", c.id())).selected_text(text).height(320.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut chosen, Pick::Value, "Value");
            for info in proc_textures::TEXTURES {
                ui.selectable_value(&mut chosen, Pick::Texture(info.id), format!("{} · {}", info.family.label(), info.label));
            }
        });
    });
    if chosen != selected {
        let next = match chosen {
            Pick::Value => None,
            // a new texture keeps the shared parameters of the one it replaces
            Pick::Texture(id) => Some(Procedural::new(id).with_params(current.as_ref().map(|t| t.params.clone()).unwrap_or_default())),
        };
        set_channel(source, c, next);
        return;
    }
    if let (Some(mut texture), FillSource::Texture(fill)) = (current, &mut *source) {
        ui.indent(("fill_texture_params", c.id()), |ui| {
            if params(ui, &mut texture) {
                fill.set_texture(c, texture);
            }
        });
    }
}

/// Gives `c` `texture`, or a constant for `None`; a uniform fill becomes a texture fill on its first texture.
fn set_channel(source: &mut FillSource, c: Channel, texture: Option<Procedural>) {
    if let (FillSource::Uniform(values), Some(_)) = (&*source, &texture) {
        *source = FillSource::Texture(TextureFill::from_values(values));
    }
    if let FillSource::Texture(fill) = source {
        match texture {
            Some(t) => fill.set_texture(c, t),
            None => fill.set_value(c, MID_GREY),
        }
    }
}

/// The shared parameters; true if one changed.
fn params(ui: &mut Ui, texture: &mut Procedural) -> bool {
    let mut changed = false;
    for info in COMMON_PARAMS {
        let mut v = info.read(&texture.params);
        let before = v;
        match info.kind {
            ParamKind::Int { min, max } => {
                let mut n = v as i32;
                ui.horizontal(|ui| {
                    ui.label(info.label);
                    ui.add(DragValue::new(&mut n).range(min..=max));
                });
                v = n as f32;
            }
            ParamKind::Range { min, max } => {
                ui.add(Slider::new(&mut v, min..=max).text(info.label));
            }
            ParamKind::Toggle => {
                let mut on = v != 0.0;
                ui.checkbox(&mut on, info.label);
                v = if on { 1.0 } else { 0.0 };
            }
        }
        if v != before {
            texture.set_param(info.name, info.text(v));
            changed = true;
        }
    }
    changed
}

/// "Base color", "Roughness", …
fn label(c: Channel) -> String {
    let id = c.id().replace('_', " ");
    let mut chars = id.chars();
    chars.next().map(|f| f.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}
