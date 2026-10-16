//! [`PaintEffect`]: brush work (`DataActionPaint` and its projection, eraser and smudge variants).
//! Strokes are only counted; their points stay in `document.bin`.

use super::effect::EffectHeader;
use super::source::PainterSource;

/// Which tool made the strokes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintTool {
    /// `DataActionPaint`
    Brush,
    /// `DataActionPaintProj`
    Projection,
    /// `DataActionEraser`
    Eraser,
    /// `DataActionSmudge`
    Smudge,
}

impl PaintTool {
    pub fn from_class(class: &str) -> Option<PaintTool> {
        Some(match class {
            "DataActionPaint" => PaintTool::Brush,
            "DataActionPaintProj" => PaintTool::Projection,
            "DataActionEraser" => PaintTool::Eraser,
            "DataActionSmudge" => PaintTool::Smudge,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            PaintTool::Brush => "brush",
            PaintTool::Projection => "projection",
            PaintTool::Eraser => "eraser",
            PaintTool::Smudge => "smudge",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaintEffect {
    pub header: EffectHeader,
    pub tool: PaintTool,
    /// Tool opacity (`opacity`); 1 if missing.
    pub opacity: f32,
    /// Number of `DataStroke`s (2D) plus `DataStroke3D`s.
    pub strokes: usize,
    /// `sourceColor`: what the brush paints, per channel (uniform colours are linear).
    pub colors: Vec<PainterSource>,
    /// `sourceTransparent`: the brush alpha (usually the `shape.sbsar` procedural or a bitmap).
    pub alpha: Vec<PainterSource>,
    /// `sourceStencil`, if a stencil was used.
    pub stencil: Option<PainterSource>,
}
