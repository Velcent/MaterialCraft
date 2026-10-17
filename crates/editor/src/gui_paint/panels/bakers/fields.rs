//! [`Fields`]: one frame's setting rows of one texture set: sliders, check boxes and choices that turn a change
//! into a value for the caller, join a slider drag into one history step, and outline the row a log entry led to.

use crate::gui_theme;
use crate::gui_widgets::SliderRow;
use eframe::egui::{self, Ui};
use materialcraft::history::CoalesceKey;

use super::BakerEvent;

/// A slider's range, arrow step and reset value.
pub struct Num {
    pub range: std::ops::RangeInclusive<f32>,
    pub step: f32,
    pub default: f32,
}

/// `Num` from `(min..=max, step, default)`.
pub fn num(range: std::ops::RangeInclusive<f32>, step: f32, default: f32) -> Num {
    Num { range, step, default }
}

pub struct Fields<'a> {
    /// The field to outline (`<section>.<field>`), and whether to scroll to it this frame.
    pub highlight: Option<(&'a str, bool)>,
    pub drag: &'a mut Option<CoalesceKey>,
    pub next_key: &'a mut u64,
    pub events: &'a mut Vec<BakerEvent>,
}

impl Fields<'_> {
    /// Draws `add` as the row of `field`, outlined when it is the highlighted one.
    pub fn row<R>(&mut self, ui: &mut Ui, field: &str, add: impl FnOnce(&mut Ui, &mut Self) -> R) -> R {
        let inner = ui.scope(|ui| add(ui, self));
        if let Some((wanted, scroll)) = self.highlight
            && wanted == field
        {
            let rect = inner.response.rect.expand(2.0);
            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.5, gui_theme::ACCENT), egui::StrokeKind::Outside);
            if scroll {
                inner.response.scroll_to_me(Some(egui::Align::Center));
            }
        }
        inner.inner
    }

    /// A slider for `field`; returns the new value and the drag's key when it changed.
    pub fn slider(&mut self, ui: &mut Ui, field: &str, label: &str, value: f32, n: Num) -> Option<(f32, CoalesceKey)> {
        let Num { range, step, default } = n;
        self.row(ui, field, |ui, f| {
            let mut v = value;
            let decimals = if step >= 1.0 { 0 } else { (-step.log10()).ceil().max(0.0) as usize + 1 };
            let resp = SliderRow::new(label, &mut v, range).step(step).default_value(default).decimals(decimals).show(ui);
            if resp.began || f.drag.is_none() && resp.changed {
                *f.next_key += 1;
                *f.drag = Some(CoalesceKey(*f.next_key));
            }
            let out = if resp.changed { f.drag.map(|k| (v, k)) } else { None };
            if resp.ended {
                *f.drag = None;
                f.events.push(BakerEvent::EndGesture);
            }
            out
        })
    }

    /// A whole-number slider.
    pub fn count(
        &mut self,
        ui: &mut Ui,
        field: &str,
        label: &str,
        value: u32,
        range: std::ops::RangeInclusive<u32>,
        default: u32,
    ) -> Option<(u32, CoalesceKey)> {
        let r = (*range.start() as f32)..=(*range.end() as f32);
        self.slider(ui, field, label, value as f32, Num { range: r, step: 1.0, default: default as f32 }).map(|(v, k)| (v.round().max(0.0) as u32, k))
    }

    /// A check box; the new state when clicked.
    pub fn check(&mut self, ui: &mut Ui, field: &str, label: &str, value: bool) -> Option<bool> {
        self.row(ui, field, |ui, _| {
            let mut v = value;
            ui.checkbox(&mut v, label).changed().then_some(v)
        })
    }

    /// A choice among `options` (value, label); the new value when another was picked.
    pub fn choice<T: Copy + PartialEq>(&mut self, ui: &mut Ui, field: &str, label: &str, value: T, options: &[(T, &str)]) -> Option<T> {
        self.row(ui, field, |ui, _| {
            let mut picked = None;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).color(gui_theme::LABEL_TEXT));
                let current = options.iter().find(|(v, _)| *v == value).map_or("?", |(_, l)| l);
                egui::ComboBox::from_id_salt(("bakers_choice", field)).selected_text(current).show_ui(ui, |ui| {
                    for (v, l) in options {
                        if ui.selectable_label(*v == value, *l).clicked() && *v != value {
                            picked = Some(*v);
                        }
                    }
                });
            });
            picked
        })
    }
}
