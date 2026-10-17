//! The brush panel (PT-04 … PT-11, PT-02): the active brush tool's settings and the values it paints per channel.
//! Settings take effect from the next stroke.

use super::super::shell::tools::Tool;
use super::super::tools::BrushTools;
use crate::gui_theme;
use crate::gui_widgets::SliderRow;
use eframe::egui::{self, RichText, Ui};
use materialcraft::paint::TextureSet;
use materialcraft::paint::brush::{BrushPreset, Placement, SizeSpace};

pub fn show(ui: &mut Ui, brush: &mut BrushTools, tool: Tool, set: Option<&TextureSet>) {
    let erase = tool == Tool::Eraser;
    if brush.preset(tool).is_none() {
        ui.weak("The brush settings belong to the paint and eraser tools.");
        return;
    }
    egui::ScrollArea::vertical().id_salt("paint_brush_panel").show(ui, |ui| {
        let mut occlusion = brush.occlusion;
        if let Some(preset) = brush.preset_mut(tool) {
            heading(ui, if erase { "Eraser" } else { "Brush" });
            shape(ui, preset);
            heading(ui, "Variation");
            variation(ui, preset);
            heading(ui, "Placement");
            placement(ui, preset, &mut occlusion);
        }
        brush.occlusion = occlusion;
        if !erase {
            heading(ui, "Painted values");
            values(ui, brush, set);
        }
    });
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).strong().color(gui_theme::HEADER_TEXT));
}

/// Size slider bounds and unit for the preset's size space.
pub fn size_range(space: SizeSpace) -> (std::ops::RangeInclusive<f32>, &'static str, f32) {
    match space {
        SizeSpace::Model => (0.001..=1.0, "of model", 0.08),
        SizeSpace::Screen | SizeSpace::Texture => (1.0..=1000.0, "px", 60.0),
    }
}

fn shape(ui: &mut Ui, p: &mut BrushPreset) {
    let d = BrushPreset::default();
    let (range, unit, default) = size_range(p.size_space);
    SliderRow::new("Size", &mut p.size, range).default_value(default).suffix(unit).show(ui);
    egui::ComboBox::from_label("Size measured").selected_text(space_label(p.size_space)).show_ui(ui, |ui| {
        for s in [SizeSpace::Model, SizeSpace::Screen] {
            if ui.selectable_value(&mut p.size_space, s, space_label(s)).changed() {
                p.size = size_range(s).2;
            }
        }
    });
    SliderRow::new("Flow", &mut p.flow, 0.0..=1.0).default_value(d.flow).show(ui);
    SliderRow::new("Stroke opacity", &mut p.opacity, 0.0..=1.0).default_value(d.opacity).show(ui);
    SliderRow::new("Spacing", &mut p.spacing, 0.01..=2.0).default_value(d.spacing).show(ui);
    SliderRow::new("Hardness", &mut p.hardness, 0.0..=1.0).default_value(d.hardness).show(ui);
    SliderRow::new("Angle", &mut p.angle, -180.0..=180.0).default_value(0.0).suffix("°").show(ui);
    SliderRow::new("Roundness", &mut p.roundness, 0.01..=1.0).default_value(1.0).show(ui);
    ui.checkbox(&mut p.follow_direction, "Turn with the stroke");
    ui.checkbox(&mut p.pressure.size, "Pen pressure drives size");
    ui.checkbox(&mut p.pressure.flow, "Pen pressure drives flow");
}

fn variation(ui: &mut Ui, p: &mut BrushPreset) {
    let j = &mut p.jitter;
    SliderRow::new("Size jitter", &mut j.size, 0.0..=1.0).default_value(0.0).show(ui);
    SliderRow::new("Flow jitter", &mut j.flow, 0.0..=1.0).default_value(0.0).show(ui);
    SliderRow::new("Angle jitter", &mut j.angle, 0.0..=1.0).default_value(0.0).show(ui);
    SliderRow::new("Position jitter", &mut j.position, 0.0..=2.0).default_value(0.0).show(ui);
}

fn placement(ui: &mut Ui, p: &mut BrushPreset, occlusion: &mut bool) {
    egui::ComboBox::from_label("Stamp placement").selected_text(placement_label(p.placement)).show_ui(ui, |ui| {
        for pl in Placement::ALL {
            ui.selectable_value(&mut p.placement, pl, placement_label(pl));
        }
    });
    SliderRow::new("Facing limit", &mut p.max_angle, 0.0..=180.0).default_value(BrushPreset::default().max_angle).suffix("°").show(ui);
    ui.checkbox(occlusion, "Paint only what the view shows");
}

fn values(ui: &mut Ui, brush: &mut BrushTools, set: Option<&TextureSet>) {
    let Some(set) = set else {
        ui.weak("Open a document to set the painted values.");
        return;
    };
    for spec in &set.channels {
        let v = brush.value_mut(spec.channel);
        ui.horizontal(|ui| {
            ui.checkbox(&mut v.enabled, "");
            if spec.channel.is_color() {
                let mut rgb = [v.value[0], v.value[1], v.value[2]];
                // stored values of colour channels are sRGB-encoded, as the picker shows them
                if ui.color_edit_button_rgb(&mut rgb).changed() {
                    v.value = [rgb[0], rgb[1], rgb[2], 1.0];
                }
                ui.label(channel_label(spec.channel));
            }
        });
        if !spec.channel.is_color() {
            let mut x = v.value[0];
            if SliderRow::new(&channel_label(spec.channel), &mut x, 0.0..=1.0).default_value(spec.channel.default_value()[0]).show(ui).changed {
                v.value = [x, x, x, 1.0];
            }
        }
    }
    SliderRow::new("Mask value", &mut brush.mask_value, 0.0..=1.0).default_value(1.0).show(ui);
}

fn channel_label(c: materialcraft::paint::Channel) -> String {
    let id = c.id().replace('_', " ");
    let mut chars = id.chars();
    chars.next().map(|f| f.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}

fn space_label(s: SizeSpace) -> &'static str {
    match s {
        SizeSpace::Model => "On the model",
        SizeSpace::Screen => "On screen",
        SizeSpace::Texture => "In texels",
    }
}

fn placement_label(p: Placement) -> &'static str {
    match p {
        Placement::FacingView => "Facing the view",
        Placement::Wrapped => "Wrapped on the surface",
        Placement::Flat => "Flat on the surface",
        Placement::UvAligned => "Along the UVs",
    }
}
