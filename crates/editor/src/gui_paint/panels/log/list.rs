//! The virtualised entry list: only the rows in view are laid out and painted (LG-08). One row
//! per entry: time, level, text (first line), and a link to what the entry concerns (LG-10).

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Ui, pos2, vec2};
use materialcraft::diag::{Entry, EntryId, Level, RingBuffer, Target};

use super::index::LogIndex;
use super::rows::{Follow, bottom_offset, is_at_bottom, row_window};
use super::selection::{Click, Selection};
use crate::gui_paint::panels::status_bar;
use crate::gui_theme;

/// Longest text drawn in a row; the tooltip has the rest.
const MAX_ROW_CHARS: usize = 400;
const TIME_WIDTH: f32 = 84.0;
const LEVEL_WIDTH: f32 = 58.0;
const LINK_WIDTH: f32 = 46.0;

/// The list's scroll position between frames.
#[derive(Debug, Default)]
pub struct Scroll {
    offset: f32,
    viewport: f32,
}

/// What the user did in the list this frame.
#[derive(Debug, Default)]
pub struct ListOutput {
    pub click: Option<(EntryId, Click)>,
    /// Go to what an entry concerns.
    pub reveal: Option<Target>,
    /// Copy these entries (a row's context menu).
    pub copy: Option<Vec<EntryId>>,
    /// Rows laid out this frame.
    pub laid_out: usize,
}

/// The height of one row in `ui`'s style.
pub fn row_height(ui: &Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body) + 4.0
}

/// Draws the list of `index`'s entries from `history`; `follow` keeps the newest in view.
pub fn show(ui: &mut Ui, history: &RingBuffer, index: &LogIndex, selection: &Selection, follow: &mut Follow, scroll: &mut Scroll) -> ListOutput {
    let row_h = row_height(ui);
    let count = index.len();
    let requested = follow.sticks().then(|| bottom_offset(count, row_h, scroll.viewport));
    let mut area = egui::ScrollArea::vertical().id_salt("paint_log_list").auto_shrink([false, false]);
    if let Some(o) = requested {
        area = area.vertical_scroll_offset(o);
    }
    let mut out = ListOutput::default();
    let shown = area.show_viewport(ui, |ui, viewport| {
        ui.set_height(count as f32 * row_h);
        let range = row_window(count, row_h, viewport.min.y, viewport.height());
        let (left, top, width) = (ui.max_rect().left(), ui.max_rect().top(), ui.max_rect().width());
        out.laid_out = range.len();
        for i in range {
            let Some(id) = index.shown().get(i).copied() else { continue };
            let Some(entry) = history.get(id) else { continue };
            let rect = Rect::from_min_size(pos2(left, top + i as f32 * row_h), vec2(width, row_h));
            row(ui, rect, id, entry, selection.contains(id), selection, &mut out);
        }
    });
    let offset = shown.state.offset.y;
    scroll.viewport = shown.inner_rect.height();
    let user_scrolled = (offset - requested.unwrap_or(scroll.offset)).abs() > 0.5;
    scroll.offset = offset;
    follow.observe(is_at_bottom(count, row_h, offset, scroll.viewport), user_scrolled);
    out
}

fn row(ui: &mut Ui, rect: Rect, id: EntryId, entry: &Entry, selected: bool, selection: &Selection, out: &mut ListOutput) {
    let resp = ui.interact(rect, ui.id().with(("log_row", id.0)), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 0.0, gui_theme::STRIP_SELECTED);
    } else if resp.hovered() {
        painter.rect_filled(rect, 0.0, gui_theme::STRIP_HOVER.gamma_multiply(0.5));
    } else if entry.level == Level::Error {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(120, 30, 30, 40));
    }
    let font = FontId::proportional(12.5);
    let y = rect.center().y;
    let color = status_bar::color(entry.level);
    let time = materialcraft::diag::timestamp(entry.time);
    painter.text(pos2(rect.left() + 4.0, y), Align2::LEFT_CENTER, time.get(11..23).unwrap_or(&time), FontId::monospace(11.0), Color32::from_gray(120));
    painter.text(pos2(rect.left() + 4.0 + TIME_WIDTH, y), Align2::LEFT_CENTER, entry.level.name(), font.clone(), color);
    let text_left = rect.left() + 4.0 + TIME_WIDTH + LEVEL_WIDTH;
    let text_right = if entry.target.is_some() { rect.right() - LINK_WIDTH } else { rect.right() };
    let clip = Rect::from_min_max(pos2(text_left, rect.top()), pos2(text_right - 4.0, rect.bottom())).intersect(painter.clip_rect());
    let line = first_line(&entry.text);
    let text_color = if entry.level == Level::Info || entry.level == Level::Debug { Color32::from_gray(205) } else { color };
    painter.with_clip_rect(clip).text(pos2(text_left, y), Align2::LEFT_CENTER, line, font, text_color);

    let mut link_clicked = false;
    if let Some(target) = &entry.target {
        let link_rect = Rect::from_min_max(pos2(text_right, rect.top()), rect.max);
        let link = ui.interact(link_rect, ui.id().with(("log_link", id.0)), Sense::click());
        let col = if link.hovered() { gui_theme::ACCENT } else { Color32::from_gray(150) };
        ui.painter().text(link_rect.left_center(), Align2::LEFT_CENTER, "Go to", FontId::proportional(11.5), col);
        let link = link.on_hover_text(format!("Go to {target}"));
        if link.clicked() {
            out.reveal = Some(target.clone());
            link_clicked = true;
        }
    }
    let resp = resp.on_hover_ui(|ui| tooltip(ui, entry));
    if resp.double_clicked()
        && let Some(t) = &entry.target
    {
        out.reveal = Some(t.clone());
    } else if resp.clicked() && !link_clicked {
        let m = ui.input(|i| i.modifiers);
        let how = if m.shift {
            Click::Range
        } else if m.command {
            Click::Toggle
        } else {
            Click::Plain
        };
        out.click = Some((id, how));
    }
    resp.context_menu(|ui| {
        if ui.button("Copy").clicked() {
            let ids = if selected { selection.ids().collect() } else { vec![id] };
            out.copy = Some(ids);
            ui.close();
        }
        if let Some(t) = &entry.target
            && ui.button(format!("Go to {t}")).clicked()
        {
            out.reveal = Some(t.clone());
            ui.close();
        }
    });
}

/// The first line of `text`, cut to [`MAX_ROW_CHARS`], with a mark when something was left out.
fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or("");
    let cut: String = line.chars().take(MAX_ROW_CHARS).collect();
    if cut.len() < text.trim_end().len() { format!("{cut} …") } else { cut }
}

fn tooltip(ui: &mut Ui, entry: &Entry) {
    ui.set_max_width(520.0);
    ui.label(egui::RichText::new(format!("{}  {}  [{}]", materialcraft::diag::timestamp(entry.time), entry.level, entry.area)).small().weak());
    ui.label(&entry.text);
    if let Some(t) = &entry.target {
        ui.label(egui::RichText::new(format!("About {t} (double-click to go there)")).small().weak());
    }
}

#[cfg(test)]
mod tests {
    use super::first_line;

    #[test]
    fn rows_show_the_first_line_and_mark_the_rest() {
        assert_eq!(first_line("one line"), "one line");
        assert_eq!(first_line("shader compile failed:\n0:12 error\n"), "shader compile failed: …");
        assert_eq!(first_line(&"x".repeat(1000)).chars().count(), super::MAX_ROW_CHARS + 2);
        assert_eq!(first_line(""), "");
    }
}
