//! [`ColorPicker`]: the one floating colour picker of a screen (CP-01…CP-04, CP-06…CP-08, CP-11). It edits the colour
//! of the field that opened it and sends every change back at once (WS-08); it stays open until that field is no
//! longer drawn (the context changed) or the user closes it.

use eframe::egui::{self, Context, Id, RichText, Sense, TextEdit, Ui, vec2};
use materialcraft::paint::{Channel, ColorSpace};

use super::color::{FieldColor, hsv_of, rgb_of};
use super::eyedropper::{Eyedropper, PickInto, Sample, SampleRequest};
use super::link::{self, FieldRequest};
use super::sliders::{self, SliderEdit};
use super::spectrum;
use super::swatches::{self, SwatchAction};
use crate::gui_settings::{ColorSliders, ColorUnits, UserSettings};
use crate::gui_theme;
use crate::gui_widgets::Icon;

/// The key of the picker opened without a field (from the menu): it never closes on its own.
fn unbound_key() -> Id {
    Id::new("gui_color.unbound")
}

/// Opens the picker without a field (its colour stays in the picker; the eyedropper and swatches work).
pub fn open_unbound(ctx: &Context) {
    let req = FieldRequest { key: unbound_key(), color: FieldColor::new([1.0; 4], ColorSpace::Srgb), channel: Channel::BaseColor, label: String::new() };
    link::request_open(ctx, req);
}

pub struct ColorPicker {
    open: bool,
    /// The field being edited (its colour at opening is `previous`).
    target: Option<FieldRequest>,
    current: FieldColor,
    previous: FieldColor,
    /// HSV of the display-encoded current colour, kept so hue and saturation survive greys and black.
    hsv: [f32; 3],
    /// The hex text while it is typed.
    hex_edit: Option<String>,
    eyedropper: Eyedropper,
}

impl Default for ColorPicker {
    fn default() -> ColorPicker {
        let white = FieldColor::new([1.0; 4], ColorSpace::Srgb);
        ColorPicker { open: false, target: None, current: white, previous: white, hsv: [0.0, 0.0, 1.0], hex_edit: None, eyedropper: Eyedropper::default() }
    }
}

impl ColorPicker {
    /// Takes the fields' requests, closes when the field is gone and draws the window. Call once per frame after
    /// the panels are drawn.
    pub fn frame(&mut self, ctx: &Context, settings: &mut UserSettings) {
        if let Some(req) = link::take_open(ctx) {
            self.open_for(req);
        }
        if let Some(req) = link::take_pick(ctx) {
            let channel = req.channel;
            self.eyedropper.arm(PickInto::Field(req), channel);
        }
        if self.open && self.field_gone(ctx) {
            self.close();
        }
        if self.open {
            self.window(ctx, settings);
        }
    }

    /// The eyedropper's click on the view (`view`: where it is drawn), for the screen to sample.
    pub fn poll_eyedropper(&mut self, ctx: &Context, view: egui::Rect) -> Option<SampleRequest> {
        self.eyedropper.poll_click(ctx, view)
    }

    /// True while the eyedropper waits for a click on the view: tools must leave that click alone.
    #[allow(dead_code)] // first checked by the brush tools (roadmap step 20)
    pub fn eyedropper_armed(&self) -> bool {
        self.eyedropper.is_armed()
    }

    /// The sample asked for by [`ColorPicker::poll_eyedropper`]: into the picker or straight into the field. RGB is
    /// converted to the field's space (exact when it is the sample's), alpha is kept.
    pub fn accept_sample(&mut self, ctx: &Context, sample: Sample) {
        match self.eyedropper.finish() {
            Some(PickInto::Picker) => {
                let c = self.current.with_rgb_in(sample.rgb, sample.space);
                self.set(ctx, c, None);
            }
            Some(PickInto::Field(req)) => link::deliver(ctx, req.key, req.color.with_rgb_in(sample.rgb, sample.space)),
            None => {}
        }
    }

    /// The asked-for sample could not be taken; the eyedropper stops.
    pub fn sample_failed(&mut self) {
        self.eyedropper.cancel();
    }

    fn open_for(&mut self, req: FieldRequest) {
        if req.key != unbound_key() || !self.open {
            self.current = if req.key == unbound_key() { self.current } else { req.color };
            self.previous = self.current;
            self.hsv = hsv_of(self.current.srgb());
        }
        self.hex_edit = None;
        self.target = Some(req);
        self.open = true;
    }

    fn close(&mut self) {
        self.open = false;
        self.target = None;
        if self.eyedropper.is_armed_for_picker() {
            self.eyedropper.cancel();
        }
    }

    /// True when the bound field was not drawn in this frame or the last.
    fn field_gone(&self, ctx: &Context) -> bool {
        let Some(t) = &self.target else { return false };
        if t.key == unbound_key() {
            return false;
        }
        let now = ctx.cumulative_pass_nr();
        link::last_seen(ctx, t.key).is_none_or(|seen| now.saturating_sub(seen) > 1)
    }

    /// Makes `color` current, with `hsv` kept if given, and sends it to the field.
    fn set(&mut self, ctx: &Context, color: FieldColor, hsv: Option<[f32; 3]>) {
        self.current = color;
        self.hsv = hsv.unwrap_or_else(|| sliders::hsv_after_rgb(color.srgb(), self.hsv));
        self.hex_edit = None;
        if let Some(t) = self.target.as_ref().filter(|t| t.key != unbound_key()) {
            link::deliver(ctx, t.key, color);
        }
    }

    fn window(&mut self, ctx: &Context, settings: &mut UserSettings) {
        let title = match self.target.as_ref().map(|t| t.label.as_str()).filter(|l| !l.is_empty()) {
            Some(label) => format!("Colour: {label}"),
            None => "Colour".to_owned(),
        };
        let mut open = true;
        egui::Window::new(title)
            .id(Id::new("gui_color.window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(260.0)
            .min_width(200.0)
            .show(ctx, |ui| self.body(ui, settings));
        if !open {
            self.close();
        }
    }

    fn body(&mut self, ui: &mut Ui, settings: &mut UserSettings) {
        let ctx = ui.ctx().clone();
        let mut hsv = self.hsv;
        if spectrum::show(ui, &mut hsv) {
            let c = self.current.with_srgb(rgb_of(hsv));
            self.set(&ctx, c, Some(hsv));
        }
        ui.add_space(4.0);
        self.slider_options(ui, settings);
        let view = settings.get().color_picker.clone();
        match sliders::show(ui, &view, self.hsv, self.current.srgb()) {
            Some(SliderEdit::Hsv(h)) => {
                let c = self.current.with_srgb(rgb_of(h));
                self.set(&ctx, c, Some(h));
            }
            Some(SliderEdit::Rgb(rgb)) => {
                let c = self.current.with_srgb(rgb);
                self.set(&ctx, c, None);
            }
            None => {}
        }
        if self.current.space == ColorSpace::Linear {
            let [r, g, b] = self.current.rgb();
            ui.label(RichText::new(format!("Stored (linear): {r:.4}  {g:.4}  {b:.4}")).small().color(gui_theme::LABEL_TEXT));
        }
        ui.add_space(4.0);
        self.hex_row(ui);
        ui.separator();
        let can_add = self.current != self.previous;
        let saved = settings.get().swatches.colors.clone();
        if let Some(action) = swatches::show(ui, &saved, can_add) {
            let srgb = self.current.srgb();
            let mut load = None;
            settings.update(|s| load = swatches::apply(&mut s.swatches.colors, action, [srgb[0], srgb[1], srgb[2], self.current.rgba[3]]));
            if let (Some(sw), SwatchAction::Load(_)) = (load, action) {
                let c = self.current.with_srgb([sw[0], sw[1], sw[2]]);
                self.set(&ctx, c, None);
            }
        }
    }

    /// HSV/RGB, units and tinting: one choice for every colour slider, kept in the settings.
    fn slider_options(&mut self, ui: &mut Ui, settings: &mut UserSettings) {
        let mut view = settings.get().color_picker.clone();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut view.sliders, ColorSliders::Hsv, "HSV");
            ui.selectable_value(&mut view.sliders, ColorSliders::Rgb, "RGB");
            ui.separator();
            ui.selectable_value(&mut view.units, ColorUnits::Natural, "0–255").on_hover_text("0–255, degrees and percent");
            ui.selectable_value(&mut view.units, ColorUnits::Float, "0–1");
            ui.separator();
            ui.checkbox(&mut view.tinted, "Tint").on_hover_text("Slider tracks show the colour");
        });
        if view != settings.get().color_picker {
            settings.update(|s| s.color_picker = view);
        }
    }

    /// The hex field, current and previous colour, and the eyedropper button.
    fn hex_row(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Hex").color(gui_theme::LABEL_TEXT));
            let mut text = self.hex_edit.clone().unwrap_or_else(|| self.current.hex());
            let resp = ui.add(TextEdit::singleline(&mut text).desired_width(72.0).char_limit(7).font(egui::TextStyle::Monospace));
            if resp.changed() {
                // A complete code applies at once; the text stays as typed while the field has focus.
                if let Some(c) = self.current.with_hex(&text) {
                    self.set(&ctx, c, None);
                }
                self.hex_edit = Some(text);
            }
            if !resp.has_focus() {
                self.hex_edit = None;
            }
            ui.add_space(6.0);
            self.current_previous(ui);
            ui.add_space(6.0);
            let armed = self.eyedropper.is_armed_for_picker();
            let (rect, b) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            if armed || b.hovered() {
                ui.painter().rect_filled(rect, 3.0, if armed { gui_theme::STRIP_SELECTED } else { gui_theme::STRIP_HOVER });
            }
            Icon::Picker.paint(ui.painter(), rect.shrink(4.0), if armed { gui_theme::ACCENT } else { gui_theme::ICON });
            let b = b.on_hover_text("Eyedropper: click the view for the colour shown there; Shift-click for the channel's stored value");
            if b.clicked() {
                if armed {
                    self.eyedropper.cancel();
                } else {
                    let channel = self.target.as_ref().map_or(Channel::BaseColor, |t| t.channel);
                    self.eyedropper.arm(PickInto::Picker, channel);
                }
            }
        });
    }

    /// Current and previous colour side by side; clicking the previous one restores it (CP-03).
    fn current_previous(&mut self, ui: &mut Ui) {
        let size = vec2(26.0, 20.0);
        let (now, _) = ui.allocate_exact_size(size, Sense::hover());
        let [r, g, b] = self.current.srgb();
        swatches::paint_swatch(ui, now, [r, g, b, self.current.rgba[3]], false);
        let (before, resp) = ui.allocate_exact_size(size, Sense::click());
        let [r, g, b] = self.previous.srgb();
        swatches::paint_swatch(ui, before, [r, g, b, self.previous.rgba[3]], resp.hovered());
        if resp.on_hover_text("Previous colour: click to restore it").clicked() {
            let ctx = ui.ctx().clone();
            self.set(&ctx, self.previous, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(key: &str, rgba: [f32; 4], space: ColorSpace) -> FieldRequest {
        FieldRequest { key: Id::new(key), color: FieldColor::new(rgba, space), channel: Channel::BaseColor, label: key.into() }
    }

    #[test]
    fn opening_for_a_field_takes_its_colour_as_current_and_previous() {
        let ctx = Context::default();
        let mut p = ColorPicker::default();
        p.open_for(request("a", [0.2, 0.4, 0.6, 1.0], ColorSpace::Linear));
        assert!(p.open);
        assert_eq!(p.current, p.previous);
        assert_eq!(p.current.space, ColorSpace::Linear);
        let c = p.current.with_srgb([1.0, 0.0, 0.0]);
        p.set(&ctx, c, None);
        assert_eq!(link::take_delivered(&ctx, Id::new("a")), Some(c), "edits go to the field at once");
        assert_ne!(p.current, p.previous);
    }

    #[test]
    fn samples_go_to_the_picker_or_straight_to_a_field() {
        let ctx = Context::default();
        let mut p = ColorPicker::default();
        p.open_for(request("a", [0.0, 0.0, 0.0, 0.5], ColorSpace::Linear));
        p.eyedropper.arm(PickInto::Picker, Channel::BaseColor);
        p.accept_sample(&ctx, Sample { rgb: [0.25, 0.5, 0.75], space: ColorSpace::Linear });
        assert_eq!(p.current.rgba, [0.25, 0.5, 0.75, 0.5], "same space: exact, alpha kept");
        assert!(!p.eyedropper_armed());
        let field = request("b", [0.0, 0.0, 0.0, 1.0], ColorSpace::Srgb);
        p.eyedropper.arm(PickInto::Field(field), Channel::BaseColor);
        p.accept_sample(&ctx, Sample::displayed([255, 0, 0, 255]));
        assert_eq!(link::take_delivered(&ctx, Id::new("b")).map(|c| c.rgba), Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(p.current.rgba, [0.25, 0.5, 0.75, 0.5], "the picker's own colour is untouched");
    }

    #[test]
    fn the_picker_closes_when_its_field_is_no_longer_drawn() {
        let ctx = Context::default();
        let mut p = ColorPicker::default();
        p.open_for(request("a", [0.0, 0.0, 0.0, 1.0], ColorSpace::Srgb));
        assert!(p.field_gone(&ctx), "never drawn");
        link::mark_seen(&ctx, Id::new("a"));
        assert!(!p.field_gone(&ctx));
        p.open_for(FieldRequest { key: unbound_key(), ..request("x", [0.0; 4], ColorSpace::Srgb) });
        assert!(!p.field_gone(&ctx), "the unbound picker stays");
        assert_eq!(p.current.rgba, [0.0, 0.0, 0.0, 1.0], "it keeps the colour it had");
    }
}
