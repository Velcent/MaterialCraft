//! Viewport: how height is shown, normal strength, lit channel views (DS-06), wireframe (DS-11), texture
//! sharpness (DS-10, four named mip-bias steps) and anisotropic filtering (DS-09).

use eframe::egui::{RichText, Ui};

use super::super::super::view_state::ViewState;
use super::choice;
use crate::gui_theme;
use crate::gui_widgets::SliderRow;
use materialcraft::gpu3d::{MIP_BIAS_STEPS, Shared};
use materialcraft::render3d::{RELIEF_BUMP, RELIEF_DISPLACE, RELIEF_NONE, RELIEF_PARALLAX, View};

const RELIEF: [(u32, &str); 4] =
    [(RELIEF_NONE, "Normal map only"), (RELIEF_BUMP, "Bump"), (RELIEF_PARALLAX, "Parallax (outward)"), (RELIEF_DISPLACE, "Displacement")];

/// Anisotropic filtering levels; 0 asks for the most the GPU offers.
const ANISO: [(u32, &str); 6] = [(1, "Off"), (2, "2×"), (4, "4×"), (8, "8×"), (16, "16×"), (0, "Highest")];

pub fn show(ui: &mut Ui, state: &mut ViewState, gpu: &Shared) {
    let defaults = View::default();
    let view = &mut state.view;
    choice(ui, "paint_relief", "Height", &mut view.relief, &RELIEF);
    if view.relief == RELIEF_BUMP {
        SliderRow::new("Bump strength", &mut view.bump, 0.0..=12.0).default_value(defaults.bump).show(ui);
    }
    if view.relief >= RELIEF_PARALLAX {
        SliderRow::new("Height scale", &mut view.height_scale, 0.0..=0.5).default_value(defaults.height_scale).show(ui);
        SliderRow::new("Neutral level", &mut view.height_level, 0.0..=1.0).default_value(defaults.height_level).show(ui);
    }
    SliderRow::new("Normal strength", &mut view.normal_amt, 0.0..=1.0).default_value(defaults.normal_amt).show(ui);
    ui.checkbox(&mut state.lit_channels, "Light channel views").on_hover_text("Show single channels and mesh maps lit by the scene instead of as they are");
    ui.checkbox(&mut state.view.wireframe, "Wireframe");
    let mut bias =
        MIP_BIAS_STEPS.iter().map(|(b, _)| *b).min_by(|a, b| (a - state.display.mip_bias).abs().total_cmp(&(b - state.display.mip_bias).abs())).unwrap_or(0.0);
    let steps: Vec<(u32, &str)> = MIP_BIAS_STEPS.iter().map(|(b, l)| (b.to_bits(), *l)).collect();
    let mut bits = bias.to_bits();
    choice(ui, "paint_mip_bias", "Texture sharpness", &mut bits, &steps);
    bias = f32::from_bits(bits);
    state.display.mip_bias = bias;
    if bias < -1.5 {
        ui.label(RichText::new("Sharp settings shimmer on fine detail while moving.").small().color(gui_theme::ICON_DISABLED));
    }
    if let Ok(mut g) = gpu.lock() {
        let mut level = g.opts.aniso.round().clamp(0.0, 16.0) as u32;
        choice(ui, "paint_aniso", "Anisotropic filtering", &mut level, &ANISO);
        g.opts.aniso = level as f32;
    }
}
