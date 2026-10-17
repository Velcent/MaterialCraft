//! A horizontal toolbar of groups that fits narrow windows by moving trailing groups into an
//! overflow menu, never by hiding them (UI-04, UI-05), on craft-ui's `fit_toolbar`.

use craft_ui::layout::fit_toolbar;
use eframe::egui::{self, Id, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use super::icons::Icon;
use crate::gui_theme;

/// Toolbar height.
pub const HEIGHT: f32 = 28.0;
const OVERFLOW_W: f32 = 26.0;
const SPACING: f32 = 10.0;
/// Width assumed for a group before it was first measured.
const ESTIMATE: f32 = 90.0;

/// Which end of the toolbar a group sits at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Content of the active tool or edited property.
    Left,
    /// Fixed view controls.
    Right,
}

/// One indivisible group of controls.
pub struct Group<'a> {
    pub side: Side,
    pub draw: Box<dyn FnMut(&mut Ui) + 'a>,
}

impl<'a> Group<'a> {
    pub fn new(side: Side, draw: impl FnMut(&mut Ui) + 'a) -> Group<'a> {
        Group { side, draw: Box::new(draw) }
    }
}

/// Draws the toolbar across the full width of `ui`. Right-hand groups get priority; the
/// left-hand groups that don't fit move, from the last one back, into the "more" menu.
pub fn show(ui: &mut Ui, id: Id, mut groups: Vec<Group>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, gui_theme::HEADER_BG);
    // Fitting order: right groups first (they stay), then left groups in order.
    let order: Vec<usize> = (0..groups.len())
        .filter(|i| groups.get(*i).is_some_and(|g| g.side == Side::Right))
        .chain((0..groups.len()).filter(|i| groups.get(*i).is_some_and(|g| g.side == Side::Left)))
        .collect();
    let widths_id = id.with("widths");
    let mut widths: Vec<f32> =
        ui.data(|d| d.get_temp::<Vec<f32>>(widths_id)).filter(|w| w.len() == groups.len()).unwrap_or_else(|| vec![ESTIMATE; groups.len()]);
    let natural: Vec<f32> = order.iter().map(|i| widths.get(*i).copied().unwrap_or(ESTIMATE).max(1.0)).collect();
    let inner = rect.shrink2(vec2(6.0, 0.0));
    let (shown, overflow): (Vec<usize>, Vec<usize>) = match fit_toolbar(&natural, inner.width(), SPACING, OVERFLOW_W) {
        Some(fit) => {
            (fit.shown.iter().filter_map(|(k, _)| order.get(*k).copied()).collect(), fit.overflow.iter().filter_map(|k| order.get(*k).copied()).collect())
        }
        // Not even the overflow button fits: everything goes into its menu.
        None => (Vec::new(), order.clone()),
    };
    let right_edge = if overflow.is_empty() { inner.right() } else { inner.right() - OVERFLOW_W - SPACING };
    let (mut left_x, mut right_x) = (inner.left(), right_edge);
    for i in 0..groups.len() {
        if !shown.contains(&i) {
            continue;
        }
        let Some(group) = groups.get_mut(i) else { continue };
        let w = widths.get(i).copied().unwrap_or(ESTIMATE);
        let x = match group.side {
            Side::Left => left_x,
            Side::Right => right_x - w,
        };
        let slot = Rect::from_min_max(pos2(x, rect.top()), pos2((x + w.max(ESTIMATE)).min(rect.right()), rect.bottom()));
        let mut child = ui.new_child(UiBuilder::new().id_salt(("group", i)).max_rect(slot).layout(Layout::left_to_right(egui::Align::Center)));
        (group.draw)(&mut child);
        let measured = child.min_rect().width().max(1.0);
        if let Some(slot) = widths.get_mut(i) {
            *slot = measured;
        }
        match group.side {
            Side::Left => left_x += measured + SPACING,
            Side::Right => right_x -= measured + SPACING,
        }
    }
    if !overflow.is_empty() {
        let at = Rect::from_center_size(pos2(inner.right() - OVERFLOW_W / 2.0, rect.center().y), vec2(OVERFLOW_W, HEIGHT - 6.0));
        let resp = ui.interact(at, id.with("more"), Sense::click()).on_hover_text("More");
        let color = if resp.hovered() { egui::Color32::WHITE } else { gui_theme::ICON };
        Icon::More.paint(ui.painter(), at.shrink(4.0), color);
        egui::Popup::menu(&resp).show(|ui| {
            for i in &overflow {
                if let Some(group) = groups.get_mut(*i) {
                    ui.horizontal(|ui| (group.draw)(ui));
                }
            }
        });
    }
    ui.data_mut(|d| d.insert_temp(widths_id, widths));
}
