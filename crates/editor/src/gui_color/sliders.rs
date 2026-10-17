//! The picker's sliders (CP-08): HSV or RGB of the display-encoded colour, in 0–1 floats or in 0–255 / degrees /
//! percent (one choice for every slider), on tracks optionally tinted by the colour.

use eframe::egui::{Color32, Ui};

use super::color::{hsv_of, rgb_of, srgb_color32};
use crate::gui_settings::{ColorPickerSettings, ColorSliders, ColorUnits};
use crate::gui_widgets::SliderRow;

/// What a slider edit changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderEdit {
    /// New HSV (hue 0…1); the hue is kept even where RGB loses it.
    Hsv([f32; 3]),
    /// New display-encoded RGB.
    Rgb([f32; 3]),
}

/// One component: its label, range in the chosen units and display precision.
struct Component {
    label: &'static str,
    max: f32,
    decimals: usize,
    step: f32,
    suffix: &'static str,
}

fn components(sliders: ColorSliders, units: ColorUnits) -> [Component; 3] {
    let unit = |label, max, suffix| match units {
        ColorUnits::Float => Component { label, max: 1.0, decimals: 3, step: 0.001, suffix: "" },
        ColorUnits::Natural => Component { label, max, decimals: 0, step: 1.0, suffix },
    };
    match sliders {
        ColorSliders::Hsv => [unit("H", 360.0, "°"), unit("S", 100.0, "%"), unit("V", 100.0, "%")],
        ColorSliders::Rgb => [unit("R", 255.0, ""), unit("G", 255.0, ""), unit("B", 255.0, "")],
    }
}

/// Track colours of component `i` with the other two held: `steps` + 1 stops.
fn gradient(sliders: ColorSliders, i: usize, hsv: [f32; 3], rgb: [f32; 3]) -> Vec<Color32> {
    let steps = if sliders == ColorSliders::Hsv && i == 0 { 12 } else { 1 };
    (0..=steps)
        .map(|k| {
            let t = k as f32 / steps as f32;
            let c = match sliders {
                ColorSliders::Hsv => {
                    let mut h = hsv;
                    if let Some(v) = h.get_mut(i) {
                        *v = t;
                    }
                    rgb_of(h)
                }
                ColorSliders::Rgb => {
                    let mut c = rgb;
                    if let Some(v) = c.get_mut(i) {
                        *v = t;
                    }
                    c
                }
            };
            srgb_color32(c)
        })
        .collect()
}

/// Draws the three sliders for `hsv` and display-encoded `rgb`; returns the edit, if any.
pub fn show(ui: &mut Ui, settings: &ColorPickerSettings, hsv: [f32; 3], rgb: [f32; 3]) -> Option<SliderEdit> {
    let values = match settings.sliders {
        ColorSliders::Hsv => hsv,
        ColorSliders::Rgb => rgb,
    };
    let mut edit = None;
    for (i, (comp, value)) in components(settings.sliders, settings.units).iter().zip(values).enumerate() {
        let mut v = value * comp.max;
        let stops = if settings.tinted { gradient(settings.sliders, i, hsv, rgb) } else { Vec::new() };
        let resp = SliderRow::new(comp.label, &mut v, 0.0..=comp.max).step(comp.step).decimals(comp.decimals).suffix(comp.suffix).gradient(&stops).show(ui);
        if resp.changed {
            let mut next = values;
            if let Some(slot) = next.get_mut(i) {
                *slot = (v / comp.max).clamp(0.0, 1.0);
            }
            edit = Some(match settings.sliders {
                ColorSliders::Hsv => SliderEdit::Hsv(next),
                ColorSliders::Rgb => SliderEdit::Rgb(next),
            });
        }
    }
    edit
}

/// The HSV to keep after an RGB edit: the old hue (and saturation) survive where the new colour has none.
pub fn hsv_after_rgb(rgb: [f32; 3], old: [f32; 3]) -> [f32; 3] {
    let mut hsv = hsv_of(rgb);
    if hsv[2] <= 0.0 {
        (hsv[0], hsv[1]) = (old[0], old[1]);
    } else if hsv[1] <= 0.0 {
        hsv[0] = old[0];
    }
    hsv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_are_the_same_for_every_slider() {
        for sliders in [ColorSliders::Hsv, ColorSliders::Rgb] {
            assert!(components(sliders, ColorUnits::Float).iter().all(|c| c.max == 1.0));
        }
        let natural = components(ColorSliders::Hsv, ColorUnits::Natural);
        assert_eq!(natural.map(|c| c.max), [360.0, 100.0, 100.0]);
        assert_eq!(components(ColorSliders::Rgb, ColorUnits::Natural).map(|c| c.max), [255.0; 3]);
    }

    #[test]
    fn tinted_tracks_run_through_the_component() {
        let g = gradient(ColorSliders::Rgb, 0, [0.0; 3], [0.5, 0.25, 1.0]);
        assert_eq!(g, vec![srgb_color32([0.0, 0.25, 1.0]), srgb_color32([1.0, 0.25, 1.0])]);
        assert_eq!(gradient(ColorSliders::Hsv, 0, [0.3, 1.0, 1.0], [0.0; 3]).len(), 13, "hue shows the spectrum");
    }

    #[test]
    fn greys_and_black_keep_the_hue() {
        assert_eq!(hsv_after_rgb([0.5, 0.5, 0.5], [0.6, 0.8, 0.2]), [0.6, 0.0, 0.5]);
        assert_eq!(hsv_after_rgb([0.0; 3], [0.6, 0.8, 0.2]), [0.6, 0.8, 0.0]);
    }
}
