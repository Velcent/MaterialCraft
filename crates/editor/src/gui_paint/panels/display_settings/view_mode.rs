//! View mode (DS-03, DS-07, DS-08): what the views show (the same setting as the toolbar's and the 2D view's
//! dropdown) and, for channels and mesh maps, which components, a display scale for high-range values and signed
//! values in two colours.

use eframe::egui::{self, Ui};

use super::super::super::channel_display::{Component, SCALES};
use super::super::super::channel_view::ChannelView;
use super::super::super::view_state::ViewState;
use super::super::{PanelEvent, channel_bar};
use super::choice;

pub fn show(ui: &mut Ui, state: &mut ViewState, display: ChannelView, events: &mut Vec<PanelEvent>) {
    ui.horizontal(|ui| channel_bar::show_with_id(ui, "paint_view_mode_settings", display, events));
    let material = display == ChannelView::Material;
    ui.add_enabled_ui(!material, |ui| {
        let c = &mut state.channel;
        choice(ui, "paint_component", "Components", &mut c.component, &Component::ALL);
        let mut bits = c.scale.to_bits();
        let scales: Vec<(u32, &str)> = SCALES.iter().map(|(s, l)| (s.to_bits(), *l)).collect();
        choice(ui, "paint_display_scale", "Divide by", &mut bits, &scales).on_hover_text("Shows values beyond 0 … 1 (height, displacement) divided by this");
        c.scale = f32::from_bits(bits);
        ui.checkbox(&mut c.signed, "Signed colours").on_hover_text("Negative values in one colour, positive in the other, zero black");
        if c.signed {
            ui.horizontal(|ui| {
                ui.label("Negative");
                egui::color_picker::color_edit_button_rgb(ui, &mut c.negative);
                ui.label("Positive");
                egui::color_picker::color_edit_button_rgb(ui, &mut c.positive);
            });
        }
    })
    .response
    .on_disabled_hover_text("Choose a channel or mesh map to change how it is shown");
}
