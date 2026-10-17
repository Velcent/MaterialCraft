//! [`ColorField`]: a colour field for any panel: a swatch that opens the picker (CP-01) and a small eyedropper button
//! that picks from the view without opening it (CP-07). The field holds no reference to the picker; edits arrive
//! through [`super::link`] and are written into the caller's value when the field is drawn.

use std::hash::Hash;

use eframe::egui::{Id, RichText, Sense, Ui, vec2};
use materialcraft::paint::{Channel, ColorSpace};

use super::color::FieldColor;
use super::link::{self, FieldRequest};
use super::swatches::paint_swatch;
use crate::gui_theme;
use crate::gui_widgets::Icon;

/// A labelled colour swatch with an eyedropper button.
#[allow(dead_code)] // the brush panel (roadmap step 20) draws the first colour fields
///
/// ```ignore
/// let changed = ColorField::new("brush.color", "Colour", &mut brush.color, ColorSpace::Srgb).show(ui).changed;
/// ```
pub struct ColorField<'a> {
    key: Id,
    label: &'a str,
    rgba: &'a mut [f32; 4],
    space: ColorSpace,
    channel: Channel,
}

/// What a colour field did this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ColorFieldResponse {
    /// The picker or the eyedropper changed the value.
    pub changed: bool,
}

#[allow(dead_code)]
impl<'a> ColorField<'a> {
    /// A field for `rgba`, encoded in `space`; `id_source` must be unique and stable for the field.
    pub fn new(id_source: impl Hash + std::fmt::Debug, label: &'a str, rgba: &'a mut [f32; 4], space: ColorSpace) -> ColorField<'a> {
        ColorField { key: Id::new(("gui_color.field", id_source)), label, rgba, space, channel: Channel::BaseColor }
    }

    /// The channel a stored-value pick (Shift-click) reads; base colour by default.
    pub fn channel(mut self, channel: Channel) -> Self {
        self.channel = channel;
        self
    }

    pub fn show(self, ui: &mut Ui) -> ColorFieldResponse {
        let ctx = ui.ctx().clone();
        let mut out = ColorFieldResponse::default();
        if let Some(c) = link::take_delivered(&ctx, self.key) {
            let next = FieldColor::new(*self.rgba, self.space).with_rgb_in(c.rgb(), c.space);
            let rgba = [next.rgba[0], next.rgba[1], next.rgba[2], c.rgba[3]];
            out.changed = rgba != *self.rgba;
            *self.rgba = rgba;
        }
        link::mark_seen(&ctx, self.key);
        let color = FieldColor::new(*self.rgba, self.space);
        let request = || FieldRequest { key: self.key, color, channel: self.channel, label: self.label.to_owned() };
        ui.horizontal(|ui| {
            if !self.label.is_empty() {
                ui.label(RichText::new(self.label).color(gui_theme::LABEL_TEXT));
            }
            let (rect, swatch) = ui.allocate_exact_size(vec2(40.0, 18.0), Sense::click());
            let [r, g, b] = color.srgb();
            paint_swatch(ui, rect, [r, g, b, color.rgba[3]], swatch.hovered());
            if swatch.on_hover_text(format!("{}: click to edit", color.hex())).clicked() {
                link::request_open(&ctx, request());
            }
            let (rect, drop) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            if drop.hovered() {
                ui.painter().rect_filled(rect, 3.0, gui_theme::STRIP_HOVER);
            }
            Icon::Picker.paint(ui.painter(), rect.shrink(3.0), gui_theme::ICON);
            if drop.on_hover_text("Pick from the view (Shift: the channel's stored value)").clicked() {
                link::request_pick(&ctx, request());
            }
        });
        out
    }
}
