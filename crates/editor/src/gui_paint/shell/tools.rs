//! The painting tools as the tools strip shows them (UI-01, UI-02, PT-58). The tools' behaviour
//! comes with the brush and projection work; this is which one is chosen and whether any can be used.

use crate::gui_widgets::Icon;

/// A painting tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Paint,
    Eraser,
    Projection,
    PolygonFill,
    Smudge,
    Clone,
    MaterialPicker,
}

impl Tool {
    /// Strip order.
    pub const ALL: [Tool; 7] = [Tool::Paint, Tool::Eraser, Tool::Projection, Tool::PolygonFill, Tool::Smudge, Tool::Clone, Tool::MaterialPicker];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Paint => "Paint",
            Tool::Eraser => "Eraser",
            Tool::Projection => "Projection",
            Tool::PolygonFill => "Polygon fill",
            Tool::Smudge => "Smudge",
            Tool::Clone => "Clone",
            Tool::MaterialPicker => "Material picker",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Tool::Paint => Icon::Brush,
            Tool::Eraser => Icon::Eraser,
            Tool::Projection => Icon::Projection,
            Tool::PolygonFill => Icon::PolygonFill,
            Tool::Smudge => Icon::Smudge,
            Tool::Clone => Icon::Clone,
            Tool::MaterialPicker => Icon::Picker,
        }
    }

    /// The shortcut action that selects this tool.
    pub fn action(self) -> &'static str {
        match self {
            Tool::Paint => "paint.tool.paint",
            Tool::Eraser => "paint.tool.eraser",
            Tool::Projection => "paint.tool.projection",
            Tool::PolygonFill => "paint.tool.polygon_fill",
            Tool::Smudge => "paint.tool.smudge",
            Tool::Clone => "paint.tool.clone",
            Tool::MaterialPicker => "paint.tool.material_picker",
        }
    }
}

/// Why the tools can't be used right now; `None` when they can. Painting needs a selected paint
/// layer's content or any layer's or folder's mask (ADR 0008, mc2-q-pt-02c); `paintable` says
/// whether the layer panel's selection is one.
pub fn unavailable(has_document: bool, paintable: bool) -> Option<&'static str> {
    match (has_document, paintable) {
        (true, true) => None,
        (true, false) => Some("Select a paint layer or a mask to paint."),
        (false, _) => Some("Open a document and select a paint layer or a mask to paint."),
    }
}

/// The chosen tool and the one before it (the quick-mask key returns to it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolState {
    pub current: Tool,
    pub previous: Tool,
}

impl Default for ToolState {
    fn default() -> ToolState {
        ToolState { current: Tool::Paint, previous: Tool::Paint }
    }
}

impl ToolState {
    pub fn select(&mut self, tool: Tool) {
        if tool != self.current {
            self.previous = self.current;
            self.current = tool;
        }
    }
}
