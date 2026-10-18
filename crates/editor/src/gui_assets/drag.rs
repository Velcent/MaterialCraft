//! An asset being dragged out of the tray: what it is, what it drops as (or why it can't be dropped), and the
//! small label that follows the pointer.

use eframe::egui::{self, FontId, Id, LayerId, Order, Pos2, Rect, vec2};
use materialcraft::assets::{Format, Usage};
use materialcraft::paint::edit::ops::DropPayload;

use super::icons;
use super::payload;
use super::results::Item;
use crate::gui_theme;

pub struct Drag {
    pub item: Item,
    pub payload: Result<DropPayload, String>,
}

impl Drag {
    pub fn new(item: Item) -> Drag {
        let payload = payload::payload(&item);
        Drag { item, payload }
    }

    /// True for a picture that can be laid over the 3D view (a stencil or a projection image, PT-15, PJ-26).
    pub fn is_view_image(&self) -> bool {
        matches!(self.item.usage, Usage::Alpha | Usage::Texture) && self.item.format == Format::Png
    }

    /// True for an environment map: let go over the 3D view, it lights the view (EN-01).
    pub fn is_environment(&self) -> bool {
        self.item.usage == Usage::Environment && matches!(self.item.format, Format::Exr | Format::Hdr)
    }

    /// The label under the pointer: the asset's icon and name, or why it can't be dropped.
    pub fn paint_ghost(&self, ctx: &egui::Context, pointer: Option<Pos2>) {
        let Some(p) = pointer else { return };
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("asset_tray_drag")));
        let text = match &self.payload {
            Ok(_) => self.item.name.clone(),
            Err(_) if self.is_view_image() => format!("{} — drop it on the view as the stencil or projection image", self.item.name),
            Err(_) if self.is_environment() => format!("{} — drop it on the view to light the view with it", self.item.name),
            Err(why) => format!("{} — {why}", self.item.name),
        };
        let font = FontId::proportional(12.0);
        let galley = painter.layout_no_wrap(text, font, gui_theme::HEADER_TEXT);
        let rect = Rect::from_min_size(p + vec2(14.0, 10.0), vec2(galley.size().x + 30.0, 22.0));
        painter.rect_filled(rect, 3.0, gui_theme::HEADER_BG.gamma_multiply(0.95));
        let icon = Rect::from_min_size(rect.min + vec2(4.0, 3.0), vec2(16.0, 16.0));
        let color = if self.payload.is_ok() || self.is_view_image() || self.is_environment() { gui_theme::ACCENT } else { gui_theme::ICON_DISABLED };
        icons::paint(&painter, icon, self.item.usage, color);
        painter.galley(rect.min + vec2(24.0, (22.0 - galley.size().y) * 0.5), galley, gui_theme::HEADER_TEXT);
    }
}
