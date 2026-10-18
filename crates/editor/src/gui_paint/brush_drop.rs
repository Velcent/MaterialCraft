//! Brushes and alphas from the asset tray on the Paint screen (AS-06, PT-11): a brush preset let go over the tools
//! strip or the brush panel loads into the shown brush tool (the paint tool if another tool is chosen); an alpha let
//! go over the brush panel becomes its tip. An alpha let go elsewhere goes on as before (the view's stencil or
//! projection image). Where things land is [`drop_zones`]; what an asset is, [`crate::gui_assets::brush_item`].

use super::PaintMode;
use super::panels::PanelEvent;
use super::panels::drop_zones::{self, BrushZone};
use super::panels::layers::StackDropTarget;
use crate::gui_assets::TrayEvent;
use crate::gui_assets::brush_item::{BrushDrop, tip_label};
use crate::gui_assets::results::Item;
use eframe::egui;
use materialcraft::diag::{Area, SinkExt};

impl PaintMode {
    /// Ends the tray's frame: a brush or alpha let go over a brush zone is applied now (the zones are known only
    /// this frame); every other tray request is handed back to be applied with the frame's events.
    pub(super) fn end_tray_frame(&mut self, ctx: &egui::Context) -> Option<PanelEvent> {
        match self.tray.end_frame(ctx)? {
            TrayEvent::DroppedBrush { item, brush, pos, image } => {
                self.drop_brush(drop_zones::at(ctx, pos), &item, brush, pos, image);
                None
            }
            other => Some(PanelEvent::Assets(other)),
        }
    }

    /// Applies a brush or alpha let go at `pos`, over `zone` if any.
    pub(super) fn drop_brush(&mut self, zone: Option<BrushZone>, item: &Item, brush: BrushDrop, pos: egui::Pos2, image: bool) {
        self.layers.hover(None);
        let current = self.shell.tools.current;
        let brushes = &mut self.painting.brush;
        let tool = brushes.brush_tool_for(current);
        match (zone, brush) {
            (Some(_), BrushDrop::Preset(preset)) => {
                let name = preset.name.clone();
                if let Some(p) = brushes.preset_mut(tool) {
                    *p = preset;
                }
                self.shell.tools.select(tool);
                self.diag.info(Area::Assets, format!("The {} tool paints with \"{name}\" now.", tool.label()));
            }
            (Some(BrushZone::BrushPanel), BrushDrop::Tip(tip)) => {
                let label = tip_label(&tip);
                if let Some(p) = brushes.preset_mut(tool) {
                    p.tip = tip;
                }
                self.shell.tools.select(tool);
                self.diag.info(Area::Assets, format!("The {} tool's tip is {label} now.", tool.label()));
            }
            (_, BrushDrop::Tip(_)) if image && self.drop_on_view(item, None, pos) => {}
            (Some(BrushZone::ToolsStrip), BrushDrop::Tip(_)) => {
                self.diag.info(Area::Assets, format!("Drop {} on the brush panel to make it the tip.", item.name));
            }
            (None, BrushDrop::Preset(_)) => {
                self.diag.info(Area::Assets, format!("Drop {} on the tools strip or the brush panel to paint with it.", item.name));
            }
            (None, BrushDrop::Tip(_)) => {
                self.diag.info(Area::Assets, format!("Drop {} on the brush panel to make it the tip, or on the view as the stencil.", item.name));
            }
        }
    }
}
