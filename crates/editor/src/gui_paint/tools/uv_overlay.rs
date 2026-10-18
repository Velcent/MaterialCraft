//! The brush tools' visual aids over the 2D view: the brush's footprint at the pointer (its size counts texels
//! there), the polygon fill's rectangle or lasso while dragging, the lazy mouse and the straight line's guide from
//! the last 2D stroke's end. Same [`Overlay`] geometry as the 3D view's aids, in the 2D view's coordinates.

use super::Painting;
use super::overlay::{Aid, Overlay};
use crate::gui_paint::shell::tools::Tool;
use crate::gui_paint::view2d::UvMap;
use crate::gui_paint::viewport::Hover;
use materialcraft::paint::brush::SizeSpace;
use materialcraft_brush::stroke::line::SNAP_DEGREES;
use materialcraft_brush::stroke::snap_angle;

impl Painting {
    /// The aids to draw over the 2D view this frame, for a target set of `resolution` texels.
    pub fn uv_overlay(&self, map: UvMap, resolution: (u32, u32), hover: Option<Hover>, tool: Tool) -> Overlay {
        let mut out = Overlay::default();
        if let (Tool::PolygonFill, Some(g)) = (tool, &self.gesture) {
            let outline = g.outline(self.brush.polygon.shape);
            out.lines.extend(outline.windows(2).filter_map(|w| Some((*w.first()?, *w.get(1)?, Aid::Line, true))));
            return out;
        }
        let Some(preset) = self.brush.preset(tool) else { return out };
        let Some(h) = hover else { return out };
        // the brush's diameter in texels, as the 2D stroke paints it
        let texels = match preset.size_space {
            SizeSpace::Model => preset.size * resolution.0 as f32,
            SizeSpace::Screen | SizeSpace::Texture => preset.size,
        };
        let radius = 0.5 * texels * map.view.points_per_texel(resolution.0);
        out.rings.push((h.pos, radius.max(2.0), Aid::Cursor));
        let helpers = self.brush.helpers;
        if helpers.lazy.on {
            out.rings.push((h.pos, helpers.lazy.radius.min(materialcraft_brush::LazyMouse::MAX_RADIUS), Aid::Lazy));
        }
        if let (Some(snap), None, true, Some(anchor)) = (h.line, self.active.as_ref(), self.anchor_uv, self.anchor)
            && let Some(from_texel) = anchor.start(None)
        {
            let from = map.screen_of_texel(from_texel, resolution);
            let to = if snap { snap_angle(from, h.pos, SNAP_DEGREES) } else { h.pos };
            out.lines.push((from, to, Aid::Line, true));
        }
        out
    }
}
