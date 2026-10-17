// The row layout (label and value field above a full-width thin track with a round knob), the
// knob painting, the gradient track and the double-click reset follow PhotoCraft's crates/ui-egui/src/widgets.rs
// (`slider_row_prec`, `slider_with`, `RowGestures::reset`), revision 5f65579, rewritten for
// MaterialCraft's theme, typing and modifier rules. See ATTRIBUTION.md.
//
// Copyright (c) 2026 ArtCraft Team and the PhotoCraft contributors.
// Licensed under either of the Apache License, Version 2.0 or the MIT license, at your option.

//! [`SliderRow`]: the one slider every numeric setting uses (SL-01…SL-08).

use std::ops::RangeInclusive;

use eframe::egui::{self, Align, Color32, Event, EventFilter, Key, Layout, Rect, Response, RichText, Sense, Stroke, TextEdit, Ui, pos2, vec2};

use super::math::{self, DragAnchor, Span, StepMods};
use crate::gui_theme;

const FIELD_W: f32 = 64.0;
const TRACK_H: f32 = 18.0;
const KNOB_R: f32 = 6.0;

/// What a slider row did this frame. `began`/`ended` bracket one drag or one typed edit, so a
/// caller with a history records one undo step per gesture (SL-07).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SliderResponse {
    pub changed: bool,
    /// A drag or a typed edit started this frame.
    pub began: bool,
    /// The drag or typed edit ended this frame (also on an arrow step or reset, which are
    /// complete gestures on their own).
    pub ended: bool,
}

/// A labelled slider: label and editable value above a full-width track.
///
/// - Drag: the value follows the pointer; Shift is finer (a tenth of the speed), Ctrl/Cmd moves
///   in coarse steps (ten times the step). A click on the track jumps to the pointer.
/// - After a click the track keeps focus: Up/Down step by the step, Shift a tenth, Ctrl/Cmd ten times.
/// - The field takes only numbers and moves the knob while typing; Up/Down step there too.
/// - A double-click on the track resets to the default, if there is one.
pub struct SliderRow<'a> {
    label: &'a str,
    value: &'a mut f32,
    span: Span,
    step: Option<f32>,
    default: Option<f32>,
    decimals: Option<usize>,
    suffix: &'a str,
    gradient: Option<&'a [Color32]>,
}

impl<'a> SliderRow<'a> {
    pub fn new(label: &'a str, value: &'a mut f32, range: RangeInclusive<f32>) -> SliderRow<'a> {
        SliderRow { label, value, span: Span::new(*range.start(), *range.end()), step: None, default: None, decimals: None, suffix: "", gradient: None }
    }

    /// The arrow step (default: about a hundredth of the range).
    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    /// The value a double-click on the track restores (SL-08).
    pub fn default_value(mut self, value: f32) -> Self {
        self.default = Some(value);
        self
    }

    /// Decimal places shown (default: enough for a fine step).
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }

    /// A unit shown after the number (`%`, `°`).
    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }

    /// Paints the track with these colours, evenly spaced from the start of the range to its end (a colour
    /// slider's tinted background, CP-08); fewer than two colours keep the plain track.
    pub fn gradient(mut self, colors: &'a [Color32]) -> Self {
        self.gradient = Some(colors);
        self
    }

    pub fn show(self, ui: &mut Ui) -> SliderResponse {
        let id = ui.next_auto_id().with(self.label);
        let step = self.step.filter(|s| s.is_finite() && *s > 0.0).unwrap_or_else(|| self.span.default_step());
        let decimals = self.decimals.unwrap_or_else(|| math::decimals_for(step / 10.0));
        let mut out = SliderResponse::default();
        let start = *self.value;
        ui.push_id(id, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(self.label).color(gui_theme::LABEL_TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !self.suffix.is_empty() {
                        ui.label(RichText::new(self.suffix).color(gui_theme::LABEL_TEXT));
                    }
                    field(ui, self.value, self.span, step, decimals, &mut out);
                });
            });
            track(ui, self.value, self.span, step, self.default, self.gradient, &mut out);
        });
        out.changed |= *self.value != start;
        out
    }
}

/// The value field: text while focused, the formatted value otherwise.
fn field(ui: &mut Ui, value: &mut f32, span: Span, step: f32, decimals: usize, out: &mut SliderResponse) {
    let field_id = ui.id().with("field");
    let buf_id = field_id.with("text");
    let focused = ui.memory(|m| m.has_focus(field_id));
    if focused {
        let arrows = take_arrows(ui);
        if !arrows.is_empty() {
            for (presses, mods) in arrows {
                *value = math::arrow(*value, step, presses, mods, span);
            }
            ui.data_mut(|d| d.insert_temp(buf_id, math::format(*value, decimals)));
        }
    }
    let mut text =
        if focused { ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| math::format(*value, decimals)) } else { math::format(*value, decimals) };
    let resp = ui.add(TextEdit::singleline(&mut text).id(field_id).desired_width(FIELD_W).horizontal_align(Align::Max).char_limit(32));
    if resp.changed() {
        text = math::filter_typed(&text);
        if let Some(v) = math::parse_typed(&text, span) {
            *value = v;
        }
    }
    if resp.gained_focus() {
        out.began = true;
    }
    if resp.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(field_id, EventFilter { vertical_arrows: true, ..Default::default() }));
        ui.data_mut(|d| d.insert_temp(buf_id, text));
    } else {
        ui.data_mut(|d| d.remove::<String>(buf_id));
    }
    if resp.lost_focus() {
        out.ended = true;
    }
}

/// The track with its knob, its drag, jump, arrow and reset gestures.
fn track(ui: &mut Ui, value: &mut f32, span: Span, step: f32, default: Option<f32>, gradient: Option<&[Color32]>, out: &mut SliderResponse) {
    let width = ui.available_width().max(60.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, TRACK_H), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), vec2((rect.width() - 2.0 * KNOB_R - 2.0).max(1.0), 3.0));
    let anchor_id = resp.id.with("anchor");
    let mods = StepMods::from(ui.input(|i| i.modifiers));
    let pointer = resp.interact_pointer_pos();
    if resp.clicked() || resp.drag_started() {
        resp.request_focus();
    }
    if resp.drag_started()
        && let Some(p) = pointer
    {
        // A plain drag starts where the pointer is (the jump); with a modifier it starts from
        // the current value, so precision work never jumps.
        let from = if mods == StepMods::NONE { math::jump(p.x, track.left(), track.width(), span) } else { *value };
        *value = from;
        ui.data_mut(|d| d.insert_temp(anchor_id, DragAnchor { x: p.x, value: from, mods }));
        out.began = true;
    }
    if resp.dragged()
        && let Some(p) = pointer
    {
        let mut anchor = ui.data(|d| d.get_temp::<DragAnchor>(anchor_id)).unwrap_or(DragAnchor { x: p.x, value: *value, mods });
        if anchor.mods != mods {
            anchor = DragAnchor { x: p.x, value: *value, mods };
            ui.data_mut(|d| d.insert_temp(anchor_id, anchor));
        }
        *value = math::drag(anchor, p.x, track.width(), step, span);
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<DragAnchor>(anchor_id));
        out.ended = true;
    }
    if resp.clicked()
        && let Some(p) = pointer
    {
        *value = math::jump(p.x, track.left(), track.width(), span);
        out.began = true;
        out.ended = true;
    }
    if let Some(reset) = default
        && resp.double_clicked()
    {
        *value = span.clamp(reset);
        out.ended = true;
    }
    if resp.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, EventFilter { vertical_arrows: true, ..Default::default() }));
        for (presses, mods) in take_arrows(ui) {
            *value = math::arrow(*value, step, presses, mods, span);
            out.began = true;
            out.ended = true;
        }
    }
    paint(ui, &resp, rect, track, span.fraction(*value), gradient.filter(|g| g.len() >= 2));
}

fn paint(ui: &Ui, resp: &Response, rect: Rect, track: Rect, f: f32, gradient: Option<&[Color32]>) {
    let painter = ui.painter();
    let enabled = ui.is_enabled();
    let knob_x = track.left() + f * track.width();
    match gradient {
        Some(colors) => gradient_track(painter, track.expand2(vec2(0.0, 2.0)), colors),
        None => {
            painter.rect_filled(track, 2.0, gui_theme::SLIDER_TRACK);
            let fill = if enabled { gui_theme::SLIDER_FILL } else { gui_theme::SLIDER_TRACK };
            painter.rect_filled(Rect::from_min_max(track.min, pos2(knob_x, track.max.y)), 2.0, fill);
        }
    }
    let knob = pos2(knob_x, rect.center().y);
    painter.circle_filled(knob + vec2(0.0, 1.0), KNOB_R + 0.5, Color32::from_black_alpha(90));
    painter.circle_filled(knob, KNOB_R, if enabled { Color32::from_gray(236) } else { Color32::from_gray(120) });
    painter.circle_stroke(knob, KNOB_R, Stroke::new(1.0, Color32::from_gray(40)));
    if resp.hovered() || resp.dragged() {
        painter.circle_stroke(knob, KNOB_R + 3.0, Stroke::new(2.0, gui_theme::ACCENT.gamma_multiply(0.5)));
    }
    if resp.has_focus() {
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0, gui_theme::ACCENT.gamma_multiply(0.6)), egui::StrokeKind::Inside);
    }
}

/// A track filled with `colors`, evenly spaced, each step a linear blend (at least two colours).
fn gradient_track(painter: &egui::Painter, track: Rect, colors: &[Color32]) {
    let n = colors.len().saturating_sub(1).max(1) as f32;
    let mut mesh = egui::Mesh::default();
    for (i, w) in colors.windows(2).enumerate() {
        let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else { continue };
        let x0 = track.left() + track.width() * i as f32 / n;
        let x1 = track.left() + track.width() * (i + 1) as f32 / n;
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(x0, track.top()), a);
        mesh.colored_vertex(pos2(x1, track.top()), b);
        mesh.colored_vertex(pos2(x1, track.bottom()), b);
        mesh.colored_vertex(pos2(x0, track.bottom()), a);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(mesh);
    painter.rect_stroke(track, 0.0, Stroke::new(1.0, gui_theme::BORDER), egui::StrokeKind::Outside);
}

/// Takes this frame's Up/Down presses out of the input, each with its modifiers.
fn take_arrows(ui: &Ui) -> Vec<(i32, StepMods)> {
    let mut out = Vec::new();
    ui.input_mut(|i| {
        i.events.retain(|e| match e {
            Event::Key { key: key @ (Key::ArrowUp | Key::ArrowDown), pressed: true, modifiers, .. } => {
                out.push((if *key == Key::ArrowUp { 1 } else { -1 }, StepMods::from(*modifiers)));
                false
            }
            _ => true,
        })
    });
    out
}
