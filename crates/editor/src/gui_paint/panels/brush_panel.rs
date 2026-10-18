//! The brush panel (PT-04 … PT-11, PT-02, PT-23 … PT-28): the active brush tool's settings, the smudge and clone
//! tools' own settings, the values painted per channel (the channels switched on there are the ones smudge and
//! clone work on), the stencil (PT-15) and, for the projection tool, the projection image (PJ-26). Settings take
//! effect from the next stroke; image requests go out as [`ToolEvent::Image`]. The tip is round, a built-in
//! procedural tip or an alpha dropped from the tray (AS-06); "Save as preset…" asks for
//! [`ToolEvent::SaveBrushPreset`].

use super::super::screen_images::ImageEdit;
use super::super::shell::tools::Tool;
use super::super::tools::brush_state::{RetouchSettings, SmudgeMode};
use super::super::tools::screen_image::{ImageSlot, ScreenImages};
use super::super::tools::{BrushTools, ToolEvent};
use super::PanelEvent;
use super::drop_zones::{self, BrushZone};
use crate::gui_assets::brush_item::tip_label;
use crate::gui_theme;
use crate::gui_widgets::SliderRow;
use eframe::egui::{self, RichText, Ui};
use materialcraft::paint::TextureSet;
use materialcraft::paint::brush::{BrushPreset, Placement, SizeSpace, Tip};
use materialcraft::projection::Repeat;
use materialcraft_brush::{PROCEDURAL_TIPS, SourceMode};

/// The images over the view as the panel shows them, and how to edit them.
pub struct ImagesUi<'a> {
    pub images: &'a ScreenImages,
    /// The stencil key's label (it is rebindable).
    pub key: String,
    pub events: &'a mut Vec<PanelEvent>,
}

pub fn show(ui: &mut Ui, brush: &mut BrushTools, tool: Tool, set: Option<&TextureSet>, mut images: ImagesUi) {
    // brushes and alphas dragged from the tray land here (AS-06)
    drop_zones::mark(ui.ctx(), BrushZone::BrushPanel, ui.max_rect());
    let projection = tool == Tool::Projection;
    if brush.preset(tool).is_none() {
        ui.weak("The brush settings belong to the paint, eraser, smudge and clone tools.");
        return;
    }
    egui::ScrollArea::vertical().id_salt("paint_brush_panel").show(ui, |ui| {
        let mut occlusion = brush.occlusion;
        if tool == Tool::Smudge || tool == Tool::Clone {
            heading(ui, if tool == Tool::Smudge { "Smudge" } else { "Clone" });
            retouch(ui, &mut brush.retouch, tool);
        }
        if let Some(preset) = brush.preset_mut(tool) {
            heading(ui, if tool == Tool::Eraser { "Eraser" } else { "Brush" });
            ui.horizontal(|ui| {
                ui.label(&preset.name);
                if ui.button("Save as preset…").on_hover_text("Save these settings as a brush in your asset library").clicked() {
                    images.events.push(PanelEvent::Tool(ToolEvent::SaveBrushPreset));
                }
            });
            tip(ui, preset);
            shape(ui, preset);
            heading(ui, "Variation");
            variation(ui, preset);
            heading(ui, "Placement");
            placement(ui, preset, &mut occlusion);
        }
        brush.occlusion = occlusion;
        if projection {
            screen_image(ui, ImageSlot::Projection, &mut images);
        }
        screen_image(ui, ImageSlot::Stencil, &mut images);
        if tool != Tool::Eraser {
            // smudge and clone work on the channels switched on here (PT-23)
            heading(ui, if matches!(tool, Tool::Paint | Tool::Projection) { "Painted values" } else { "Channels" });
            if projection {
                ui.weak("The projection tool paints the image's channels; channels switched off here stay unpainted.");
            }
            values(ui, brush, set);
        }
    });
}

/// The smudge tool's mode and strength (PT-23, PT-26), the clone tool's source behaviour (PT-27, PT-28).
fn retouch(ui: &mut Ui, r: &mut RetouchSettings, tool: Tool) {
    if tool == Tool::Smudge {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut r.smudge_mode, SmudgeMode::Smudge, "Smudge").on_hover_text("Drag and mix the paint along the stroke");
            ui.selectable_value(&mut r.smudge_mode, SmudgeMode::Blur, "Blur").on_hover_text("Soften the paint under the stroke");
        });
        SliderRow::new("Strength", &mut r.strength, 0.0..=1.0).default_value(RetouchSettings::default().strength).show(ui);
        return;
    }
    ui.horizontal(|ui| {
        ui.selectable_value(&mut r.clone_mode, SourceMode::Aligned, "Aligned").on_hover_text("The source moves on with every stroke");
        ui.selectable_value(&mut r.clone_mode, SourceMode::Fixed, "Fixed").on_hover_text("Every stroke starts again at the source");
    });
    ui.weak("Hover the model and press V to mark the source.");
}

/// The stencil's or the projection image's controls.
fn screen_image(ui: &mut Ui, slot: ImageSlot, images: &mut ImagesUi) {
    heading(ui, if slot == ImageSlot::Stencil { "Stencil" } else { "Projection image" });
    let mut send = |edit| images.events.push(PanelEvent::Tool(ToolEvent::Image(slot, edit)));
    match images.images.get(slot) {
        None => {
            let what = if slot == ImageSlot::Stencil { "an alpha or texture" } else { "a texture or a material" };
            ui.weak(format!("None. Load a PNG, or drop {what} from the assets onto the view."));
        }
        Some(image) => {
            ui.label(&image.name);
            let mut repeat = image.repeat;
            egui::ComboBox::from_id_salt(("screen_image_repeat", slot == ImageSlot::Stencil)).selected_text(repeat_label(repeat)).show_ui(ui, |ui| {
                for r in [Repeat::None, Repeat::U, Repeat::V, Repeat::Both] {
                    ui.selectable_value(&mut repeat, r, repeat_label(r));
                }
            });
            if repeat != image.repeat {
                send(ImageEdit::Repeat(repeat));
            }
        }
    }
    ui.horizontal(|ui| {
        if ui.button("Load…").clicked() {
            send(ImageEdit::Load);
        }
        let has = images.images.get(slot).is_some();
        if ui.add_enabled(has, egui::Button::new("Reset")).on_hover_text("Fit it to the view again, unturned").clicked() {
            send(ImageEdit::Reset);
        }
        if ui.add_enabled(has, egui::Button::new("Remove")).clicked() {
            send(ImageEdit::Clear);
        }
    });
    let key = if images.key.is_empty() { "the stencil key".to_string() } else { images.key.clone() };
    ui.weak(format!("Hold {key} and drag: left turns (with Shift in 90° steps), right scales, middle moves."));
}

fn repeat_label(r: Repeat) -> &'static str {
    match r {
        Repeat::None => "Not repeated",
        Repeat::U => "Repeated across",
        Repeat::V => "Repeated down",
        Repeat::Both => "Repeated both ways",
    }
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

/// The tip (PT-11): round, one of the built-in procedural tips, or the alpha image dropped here.
fn tip(ui: &mut Ui, p: &mut BrushPreset) {
    egui::ComboBox::from_label("Tip").selected_text(tip_label(&p.tip)).show_ui(ui, |ui| {
        if let Tip::Alpha(path) = p.tip.clone() {
            let label = tip_label(&p.tip);
            ui.selectable_value(&mut p.tip, Tip::Alpha(path.clone()), label).on_hover_text(path);
        }
        ui.selectable_value(&mut p.tip, Tip::Round, "Round");
        for t in PROCEDURAL_TIPS {
            ui.selectable_value(&mut p.tip, Tip::Procedural(t.id.into()), t.label);
        }
    });
    if let Tip::Alpha(path) = &p.tip {
        ui.weak(format!("Alpha {path}"));
    }
    ui.weak("Drop an alpha from the assets here to make it the tip, or a brush to load it.");
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
