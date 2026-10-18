//! The assets: a grid of thumbnails in three sizes or a list (AS-09), scrolled by wheel or scrollbar (AS-10).
//! Only the cells in view are laid out and only they ask for thumbnails, so ten thousand assets cost what a
//! screenful does. Hovering a cell highlights it and shows its details (AS-20); dragging it starts a drag
//! into the layer stack; its context menu manages it ([`super::manage`]).

use eframe::egui::{self, Align2, FontId, Rect, Sense, StrokeKind, Ui, pos2, vec2};

use super::drag::Drag;
use super::icons;
use super::manage::{self, MenuChoice, MoveTarget};
use super::results::{Item, Results};
use super::thumbs::ThumbCache;
use crate::gui_settings::ThumbSize;
use crate::gui_theme;

/// Height of a list row.
const LIST_ROW: f32 = 22.0;
/// Height of a cell's name below its thumbnail.
const LABEL: f32 = 15.0;
const GAP: f32 = 4.0;

/// Side of a thumbnail in a grid of `size`.
fn side(size: ThumbSize) -> f32 {
    match size {
        ThumbSize::Small => 52.0,
        ThumbSize::Medium => 84.0,
        ThumbSize::Large => 132.0,
        ThumbSize::List => 18.0,
    }
}

/// How the cells are laid out at a width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub columns: usize,
    pub cell: egui::Vec2,
    pub rows: usize,
}

impl Grid {
    pub fn new(size: ThumbSize, width: f32, items: usize) -> Grid {
        let (columns, cell) = match size {
            ThumbSize::List => (1, vec2(width.max(1.0), LIST_ROW)),
            s => {
                let side = side(s);
                let columns = ((width + GAP) / (side + GAP)).floor().max(1.0) as usize;
                (columns, vec2(side, side + LABEL))
            }
        };
        Grid { columns, cell, rows: items.div_ceil(columns) }
    }

    fn pitch(&self) -> egui::Vec2 {
        if self.columns == 1 && self.cell.y == LIST_ROW { vec2(self.cell.x, LIST_ROW) } else { self.cell + vec2(GAP, GAP) }
    }

    /// The rows touching `y0..y1` (content coordinates).
    pub fn visible(&self, y0: f32, y1: f32) -> std::ops::Range<usize> {
        let h = self.pitch().y.max(1.0);
        let first = (y0 / h).floor().max(0.0) as usize;
        let last = ((y1 / h).ceil().max(0.0) as usize).min(self.rows);
        first.min(last)..last
    }
}

/// What one frame of the grid did.
#[derive(Default)]
pub struct Shown {
    /// A drag that started this frame.
    pub started: Option<Drag>,
    /// The first asset in view and where it was drawn.
    pub first: Option<(usize, Rect)>,
    /// What was picked in an asset's context menu.
    pub menu: Option<(Item, MenuChoice)>,
}

/// Draws the assets; `targets` are the libraries an asset can be moved into.
pub fn show(ui: &mut Ui, results: &Results, thumbs: &mut ThumbCache, size: ThumbSize, targets: &[MoveTarget]) -> Shown {
    let mut out = Shown::default();
    if results.items.is_empty() {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| ui.weak("No assets match."));
        return out;
    }
    let width = ui.available_width();
    let grid = Grid::new(size, width, results.items.len());
    egui::ScrollArea::vertical().id_salt("asset_tray_grid").auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
        let pitch = grid.pitch();
        let (all, _) = ui.allocate_exact_size(vec2(width, (grid.rows as f32 * pitch.y).max(viewport.height())), Sense::hover());
        let origin = all.min;
        for row in grid.visible(viewport.min.y, viewport.max.y) {
            for col in 0..grid.columns {
                let index = row * grid.columns + col;
                let Some(item) = results.items.get(index) else { break };
                let rect = Rect::from_min_size(origin + vec2(col as f32 * pitch.x, row as f32 * pitch.y), grid.cell);
                out.first.get_or_insert((index, rect));
                let (drag, menu) = cell(ui, rect, item, index, size, thumbs, targets);
                if let Some(d) = drag {
                    out.started = Some(d);
                }
                if let Some(m) = menu {
                    out.menu = Some((item.clone(), m));
                }
            }
        }
    });
    out
}

/// One asset; returns a drag if one started on it, and what was picked in its context menu.
fn cell(
    ui: &mut Ui,
    rect: Rect,
    item: &Item,
    index: usize,
    size: ThumbSize,
    thumbs: &mut ThumbCache,
    targets: &[MoveTarget],
) -> (Option<Drag>, Option<MenuChoice>) {
    let resp = ui.interact(rect, ui.id().with(("asset", index)), Sense::click_and_drag());
    let painter = ui.painter_at(rect.expand(1.0));
    let hovered = resp.hovered() || resp.dragged();
    if hovered {
        painter.rect_filled(rect, 3.0, gui_theme::STRIP_HOVER);
    }
    let list = size == ThumbSize::List;
    let pic = if list {
        Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(LIST_ROW - 4.0, LIST_ROW - 4.0))
    } else {
        Rect::from_min_size(rect.min, vec2(rect.width(), rect.width()))
    };
    match thumbs.get(ui.ctx(), item) {
        Some(tex) => {
            let size = tex.size_vec2();
            let fit = (pic.width() / size.x).min(pic.height() / size.y);
            let shown = Rect::from_center_size(pic.center(), size * fit);
            painter.image(tex.id(), shown, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        None => {
            if !list {
                painter.rect_filled(pic.shrink(1.0), 3.0, gui_theme::HEADER_BG);
            }
            icons::paint(&painter, pic.shrink(pic.width() * 0.25), item.usage, gui_theme::ICON);
        }
    }
    if item.importable_only() {
        let badge = if list { pic } else { Rect::from_min_size(pic.min, vec2(pic.width(), 14.0)) };
        painter.rect_filled(badge, 2.0, gui_theme::ACCENT.gamma_multiply(0.75));
        if !list {
            painter.text(badge.center(), Align2::CENTER_CENTER, "importable", FontId::proportional(9.5), egui::Color32::BLACK);
        }
    }
    let text_color = if item.importable_only() { gui_theme::LABEL_TEXT.gamma_multiply(0.7) } else { gui_theme::LABEL_TEXT };
    if list {
        let x = pic.right() + 6.0;
        painter.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, &item.name, FontId::proportional(12.5), text_color);
        let usage = format!("{}  ·  {}", item.usage, item.library);
        painter.text(pos2(rect.right() - 4.0, rect.center().y), Align2::RIGHT_CENTER, usage, FontId::proportional(11.0), gui_theme::ICON_DISABLED);
    } else {
        let label = Rect::from_min_max(pos2(rect.min.x, pic.max.y), rect.max);
        let clipped = ui.painter_at(label);
        clipped.text(pos2(label.min.x + 2.0, label.center().y), Align2::LEFT_CENTER, &item.name, FontId::proportional(11.0), text_color);
    }
    if hovered {
        painter.rect_stroke(rect, 3.0, egui::Stroke::new(1.0, gui_theme::ACCENT), StrokeKind::Inside);
    }
    let started = resp.drag_started().then(|| Drag::new(item.clone()));
    let mut choice = None;
    resp.context_menu(|ui| choice = manage::menu(ui, item, targets));
    resp.on_hover_ui(|ui| details(ui, item));
    (started, choice)
}

/// The tooltip of a cell.
fn details(ui: &mut Ui, item: &Item) {
    ui.strong(&item.name);
    ui.label(format!("{} · .{} · {}", icons::label(item.usage), item.format, item.library));
    ui.weak(&item.path);
    if !item.tags.is_empty() {
        ui.label(format!("Tags: {}", item.tags.join(", ")));
    }
    if let Some(d) = &item.description {
        ui.label(d);
    }
    match super::payload::payload(item) {
        Ok(_) => ui.weak("Drag onto the layer stack to add it."),
        Err(why) => ui.weak(why),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_fits_whole_cells_and_lists_only_rows_in_view() {
        let g = Grid::new(ThumbSize::Medium, 300.0, 10_000);
        assert_eq!(g.columns, 3);
        assert_eq!(g.rows, 3334);
        let r = g.visible(0.0, 600.0);
        assert_eq!(r.start, 0);
        assert!(r.len() <= 7, "{r:?}");
        assert_eq!(g.visible(1e9, 1e9 + 600.0).len(), 0);
        let l = Grid::new(ThumbSize::List, 300.0, 5);
        assert_eq!((l.columns, l.rows), (1, 5));
        assert_eq!(Grid::new(ThumbSize::Large, 10.0, 3).columns, 1);
    }
}
