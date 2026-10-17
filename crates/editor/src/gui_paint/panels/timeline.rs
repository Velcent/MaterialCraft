//! The history timeline above the viewport (HI-02, HI-03, HI-06): one block per step after a block for the
//! state the document was opened in; done steps in the accent colour, undone ones grey, the current state
//! marked. Clicking a block returns the whole document to the state after that step. Only the blocks in
//! view are drawn, so very long histories stay cheap.

use super::PanelEvent;
use crate::gui_theme;
use eframe::egui::{self, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use materialcraft::history::{PaintHistory, StepInfo};
use std::time::SystemTime;

const HEIGHT: f32 = 12.0;
const MIN_BLOCK: f32 = 8.0;
const MAX_BLOCK: f32 = 28.0;
const GAP: f32 = 1.0;

/// Width of one block when `blocks` share `width` (clamped, so a long history scrolls).
pub fn block_width(blocks: usize, width: f32) -> f32 {
    (width / blocks.max(1) as f32).clamp(MIN_BLOCK, MAX_BLOCK)
}

/// The blocks that overlap the horizontal span `[x0, x1)` of the content.
pub fn visible_blocks(blocks: usize, block: f32, x0: f32, x1: f32) -> std::ops::Range<usize> {
    let first = (x0 / block).floor().max(0.0) as usize;
    let end = ((x1 / block).ceil().max(0.0) as usize).min(blocks);
    first.min(end)..end
}

/// Draws the bar; a click asks for `JumpHistory(done)`.
pub fn show(ui: &mut Ui, history: &PaintHistory, events: &mut Vec<PanelEvent>) {
    let steps: Vec<StepInfo> = history.steps().collect();
    let blocks = steps.len() + 1;
    let block = block_width(blocks, ui.available_width());
    let current = history.done();
    egui::ScrollArea::horizontal()
        .id_salt("paint_timeline")
        .stick_to_right(true)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show_viewport(ui, |ui, viewport| {
            let (rect, response) = ui.allocate_exact_size(vec2(block * blocks as f32, HEIGHT), Sense::click());
            let painter = ui.painter_at(rect.intersect(ui.clip_rect()));
            for i in visible_blocks(blocks, block, viewport.min.x, viewport.max.x) {
                let r = Rect::from_min_size(pos2(rect.min.x + i as f32 * block, rect.min.y), vec2(block - GAP, HEIGHT));
                painter.rect_filled(r, 1.0, fill(i, current));
                if i == current {
                    painter.rect_stroke(r, 1.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Inside);
                }
            }
            let hovered = response.hover_pos().map(|p| ((p.x - rect.min.x) / block).floor().max(0.0) as usize).filter(|i| *i < blocks);
            if let Some(i) = hovered {
                let text = describe(i, &steps, current);
                response.clone().on_hover_text_at_pointer(text);
            }
            if response.clicked()
                && let Some(i) = hovered
                && i != current
            {
                events.push(PanelEvent::JumpHistory(i));
            }
        });
}

/// Block `i` stands for the state after `i` steps.
fn fill(i: usize, current: usize) -> Color32 {
    if i == current {
        gui_theme::ACCENT
    } else if i < current {
        gui_theme::SLIDER_FILL.gamma_multiply(0.6)
    } else {
        Color32::from_gray(70)
    }
}

fn describe(i: usize, steps: &[StepInfo], current: usize) -> String {
    let state = if i == current {
        " (current)"
    } else if i > current {
        " (undone)"
    } else {
        ""
    };
    match i.checked_sub(1).and_then(|s| steps.get(s)) {
        None => format!("Opened{state}"),
        Some(step) => format!("{}{state}\n{}", step.label, ago(step.time)),
    }
}

fn ago(time: SystemTime) -> String {
    let secs = SystemTime::now().duration_since(time).map(|d| d.as_secs()).unwrap_or(0);
    match secs {
        0..=59 => format!("{secs} s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        _ => format!("{} h ago", secs / 3600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_histories_scroll_and_draw_only_what_is_in_view() {
        assert_eq!(block_width(3, 600.0), MAX_BLOCK);
        assert_eq!(block_width(100_000, 600.0), MIN_BLOCK);
        let range = visible_blocks(100_001, MIN_BLOCK, 80_000.0, 80_600.0);
        assert_eq!(range, 10_000..10_075);
        assert_eq!(visible_blocks(5, 10.0, 0.0, 600.0), 0..5);
        assert_eq!(visible_blocks(5, 10.0, 900.0, 1000.0), 5..5);
    }
}
