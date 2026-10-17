//! Interface settings (WS-10 as renamed by ADR 0008): the interface scale for now.

use eframe::egui::Ui;

use super::PanelEvent;
use crate::gui_settings::InterfaceSettings;
use crate::gui_widgets::SliderRow;

/// `draft` is the scale shown while its slider moves; it is applied when the gesture ends, so
/// the slider doesn't rescale under the pointer.
pub fn show(ui: &mut Ui, draft: &mut f32, events: &mut Vec<PanelEvent>) {
    ui.add_space(4.0);
    let mut percent = *draft * 100.0;
    let r = SliderRow::new("Interface scale", &mut percent, InterfaceSettings::MIN_SCALE * 100.0..=InterfaceSettings::MAX_SCALE * 100.0)
        .step(5.0)
        .default_value(100.0)
        .decimals(0)
        .suffix("%")
        .show(ui);
    *draft = percent / 100.0;
    if r.ended {
        events.push(PanelEvent::SetUiScale(*draft));
    }
}
