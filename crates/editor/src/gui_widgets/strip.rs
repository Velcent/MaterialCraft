//! A vertical strip of icon buttons on craft-ui's `IconButton`: the tools strip (UI-01, UI-02)
//! and the panel strip (UI-03).

use craft_ui::buttons::{ButtonVisuals, IconButton, IconButtonStyle};
use eframe::egui::{self, Color32, CornerRadius, Rect, Stroke, Ui, vec2};

use super::icons::Icon;
use crate::gui_theme;

/// Width of a strip, button plus margins.
pub const STRIP_WIDTH: f32 = 38.0;
const BUTTON: f32 = 30.0;

/// One button of a strip.
pub struct StripItem<'a> {
    pub icon: Icon,
    /// Accessible name and first tooltip line.
    pub label: &'a str,
    /// The current shortcut, shown in the tooltip.
    pub shortcut: Option<String>,
    pub selected: bool,
}

/// What the strip did: the clicked button, and every button's rectangle (to place popovers).
#[derive(Default)]
pub struct StripResponse {
    pub clicked: Option<usize>,
    pub rects: Vec<Rect>,
    /// The empty part of the strip below the buttons, for a context menu (moving the strip).
    pub background: Option<egui::Response>,
}

fn style() -> IconButtonStyle {
    let v = |fill: Color32, icon: Color32| ButtonVisuals { fill, stroke: Stroke::NONE, icon };
    IconButtonStyle {
        normal: v(Color32::TRANSPARENT, gui_theme::ICON),
        hovered: v(gui_theme::STRIP_HOVER, Color32::WHITE),
        pressed: Some(v(gui_theme::STRIP_HOVER, gui_theme::ACCENT)),
        selected: v(gui_theme::STRIP_SELECTED, gui_theme::ACCENT),
        corner_radius: CornerRadius::same(4),
        focus_stroke: Stroke::new(1.0, gui_theme::ACCENT),
    }
}

/// Draws `items` top to bottom. While `disabled` holds a reason, every button is greyed out and
/// hovering explains why (UI-02).
pub fn vertical(ui: &mut Ui, items: &[StripItem], disabled: Option<&str>) -> StripResponse {
    let style = style();
    let mut out = StripResponse::default();
    ui.add_space(4.0);
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.add_enabled_ui(disabled.is_none(), |ui| {
            for (i, item) in items.iter().enumerate() {
                let resp = IconButton::new(item.label, vec2(BUTTON, BUTTON), &style).selected(item.selected).show(ui, |ui, rect, color| {
                    let color = if ui.is_enabled() { color } else { gui_theme::ICON_DISABLED };
                    item.icon.paint(ui.painter(), rect.shrink(7.0), color);
                });
                let tip = match &item.shortcut {
                    Some(k) if !k.is_empty() => format!("{}  ({k})", item.label),
                    _ => item.label.to_owned(),
                };
                let resp = match disabled {
                    Some(why) => resp.on_disabled_hover_text(format!("{tip}\n{why}")),
                    None => resp.on_hover_text(tip),
                };
                if resp.clicked() {
                    out.clicked = Some(i);
                }
                out.rects.push(resp.rect);
            }
        });
    });
    out.background = Some(ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click()));
    out
}
