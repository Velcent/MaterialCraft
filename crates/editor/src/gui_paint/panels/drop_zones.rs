//! Where brushes and alphas dragged from the tray can be let go (AS-06): the tools strip and the brush panel note
//! their area each frame they are drawn; the screen asks which one lies under the pointer when a drag ends. An area
//! not drawn in the last frames (a hidden panel) is no target.

use eframe::egui::{Context, Id, Pos2, Rect};

/// A place on the Paint screen that takes brushes and alphas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BrushZone {
    /// The tools strip: a brush preset dropped here loads into a brush tool.
    ToolsStrip,
    /// The brush panel: a preset loads into the shown brush tool, an alpha becomes its tip.
    BrushPanel,
}

/// Frames an area stays a target after it was last drawn.
const FRESH_PASSES: u64 = 2;

fn key(zone: BrushZone) -> Id {
    Id::new(("paint_brush_drop_zone", zone))
}

/// Notes that `zone` covers `rect` in this frame.
pub fn mark(ctx: &Context, zone: BrushZone, rect: Rect) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(key(zone), (rect, pass)));
}

/// The zone under `pos`, if one was drawn there lately.
pub fn at(ctx: &Context, pos: Pos2) -> Option<BrushZone> {
    let now = ctx.cumulative_pass_nr();
    [BrushZone::BrushPanel, BrushZone::ToolsStrip]
        .into_iter()
        .find(|z| ctx.data(|d| d.get_temp::<(Rect, u64)>(key(*z))).is_some_and(|(rect, pass)| now.saturating_sub(pass) <= FRESH_PASSES && rect.contains(pos)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    #[test]
    fn zones_answer_while_they_are_drawn() {
        let ctx = Context::default();
        let panel = Rect::from_min_size(pos2(100.0, 0.0), vec2(200.0, 300.0));
        mark(&ctx, BrushZone::BrushPanel, panel);
        mark(&ctx, BrushZone::ToolsStrip, Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 300.0)));
        assert_eq!(at(&ctx, pos2(150.0, 20.0)), Some(BrushZone::BrushPanel));
        assert_eq!(at(&ctx, pos2(10.0, 20.0)), Some(BrushZone::ToolsStrip));
        assert_eq!(at(&ctx, pos2(500.0, 20.0)), None);
        for _ in 0..4 {
            let mut out = ctx.run_ui(Default::default(), |_| {});
            out.textures_delta.clear();
        }
        assert_eq!(at(&ctx, pos2(150.0, 20.0)), None, "a panel no longer drawn takes nothing");
    }
}
