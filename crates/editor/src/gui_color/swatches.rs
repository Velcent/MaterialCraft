//! The picker's swatches (CP-11): saved colours, sRGB-encoded, kept in the user settings across sessions and
//! projects. Click to load, hover for the hex code, right-click to delete one or all; a colour can be added only when
//! the current colour differs from the previous one.

use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

use super::color::{hex, srgb_color32};
use super::painters::checker;
use crate::gui_theme;

const CELL: f32 = 18.0;

/// What the user did in the swatch section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwatchAction {
    /// Save the current colour.
    Add,
    /// Load swatch `i` as the current colour.
    Load(usize),
    Delete(usize),
    DeleteAll,
}

/// Applies `action` to the saved swatches; `current` is the current colour (sRGB, with alpha). Returns the swatch to
/// load, if any.
pub fn apply(colors: &mut Vec<[f32; 4]>, action: SwatchAction, current: [f32; 4]) -> Option<[f32; 4]> {
    match action {
        SwatchAction::Add => colors.push(current),
        SwatchAction::Load(i) => return colors.get(i).copied(),
        SwatchAction::Delete(i) => {
            if i < colors.len() {
                colors.remove(i);
            }
        }
        SwatchAction::DeleteAll => colors.clear(),
    }
    None
}

/// Draws the swatches with the add button; `can_add` enables it.
pub fn show(ui: &mut Ui, colors: &[[f32; 4]], can_add: bool) -> Option<SwatchAction> {
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
        for (i, c) in colors.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(CELL, CELL), Sense::click());
            paint_swatch(ui, rect, *c, resp.hovered());
            let resp = resp.on_hover_text(hex([c[0], c[1], c[2]]));
            if resp.clicked() {
                action = Some(SwatchAction::Load(i));
            }
            resp.context_menu(|ui| {
                if ui.button("Delete swatch").clicked() {
                    action = Some(SwatchAction::Delete(i));
                    ui.close();
                }
                if ui.button("Delete all swatches").clicked() {
                    action = Some(SwatchAction::DeleteAll);
                    ui.close();
                }
            });
        }
        let add = ui
            .add_enabled(can_add, egui::Button::new("+").min_size(vec2(CELL, CELL)))
            .on_hover_text("Save the current colour as a swatch")
            .on_disabled_hover_text("The current colour is the previous one");
        if add.clicked() {
            action = Some(SwatchAction::Add);
        }
    });
    action
}

/// One swatch: the colour over a checker where it is transparent.
pub fn paint_swatch(ui: &Ui, rect: Rect, srgba: [f32; 4], highlight: bool) {
    let painter = ui.painter();
    let fill = srgb_color32([srgba[0], srgba[1], srgba[2]]);
    let alpha = srgba[3];
    if alpha < 1.0 {
        checker(painter, rect, 4.0);
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), a));
    } else {
        painter.rect_filled(rect, 0.0, fill);
    }
    let stroke = if highlight { Stroke::new(1.5, gui_theme::ACCENT) } else { Stroke::new(1.0, gui_theme::BORDER) };
    painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_load_delete_and_clear() {
        let mut list = Vec::new();
        let red = [1.0, 0.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 1.0];
        assert_eq!(apply(&mut list, SwatchAction::Add, red), None);
        apply(&mut list, SwatchAction::Add, blue);
        assert_eq!(apply(&mut list, SwatchAction::Load(1), red), Some(blue));
        assert_eq!(apply(&mut list, SwatchAction::Load(5), red), None);
        apply(&mut list, SwatchAction::Delete(0), red);
        assert_eq!(list, vec![blue]);
        apply(&mut list, SwatchAction::Delete(9), red);
        assert_eq!(list.len(), 1);
        apply(&mut list, SwatchAction::DeleteAll, red);
        assert!(list.is_empty());
    }

    /// Swatches are written to the user settings file and read back by the next session (CP-11).
    #[test]
    fn swatches_survive_a_restart() {
        use crate::gui_settings::UserSettings;
        use craft_settings::Location;
        let dir = std::env::temp_dir().join(format!("materialcraft-swatches-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let saved = [[1.0, 0.5, 0.25, 1.0], [0.2, 0.4, 0.6, 0.5]];
        {
            let (mut s, problems) = UserSettings::open(Location::Dir(dir.clone()));
            assert!(problems.is_empty(), "{problems:?}");
            for c in saved {
                s.update(|v| {
                    apply(&mut v.swatches.colors, SwatchAction::Add, c);
                });
            }
            s.flush().unwrap();
        }
        let (s, _) = UserSettings::open(Location::Dir(dir.clone()));
        assert_eq!(s.get().swatches.colors, saved.to_vec());
        let text = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(text.contains("[swatches]"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
