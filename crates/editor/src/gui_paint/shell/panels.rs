//! The Paint screen's panels as the workspace knows them, and its built-in layouts.

use craft_ui::docking::{Layout, Node};
use craft_ui::layout::{SplitAxis, SplitSize};

use crate::gui_widgets::Icon;
use crate::gui_workspace::{PanelKind, Preset};

/// Every panel of the Paint screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaintPanel {
    /// The 3D view with its toolbar: the protected canvas.
    Viewport,
    TextureSets,
    ImportReport,
    /// Display settings (how the viewport shows the document).
    Display,
    Shortcuts,
    /// Interface settings (WS-10).
    Interface,
    /// The session log (LG-01).
    Log,
}

impl PanelKind for PaintPanel {
    const ALL: &'static [PaintPanel] = &[
        PaintPanel::Viewport,
        PaintPanel::TextureSets,
        PaintPanel::ImportReport,
        PaintPanel::Display,
        PaintPanel::Shortcuts,
        PaintPanel::Interface,
        PaintPanel::Log,
    ];

    fn id(self) -> &'static str {
        match self {
            PaintPanel::Viewport => "viewport",
            PaintPanel::TextureSets => "texture_sets",
            PaintPanel::ImportReport => "import_report",
            PaintPanel::Display => "display_settings",
            PaintPanel::Shortcuts => "shortcuts",
            PaintPanel::Interface => "interface_settings",
            PaintPanel::Log => "log",
        }
    }

    fn title(self) -> &'static str {
        match self {
            PaintPanel::Viewport => "Viewport",
            PaintPanel::TextureSets => "Texture sets",
            PaintPanel::ImportReport => "Import report",
            PaintPanel::Display => "Display settings",
            PaintPanel::Shortcuts => "Shortcuts",
            PaintPanel::Interface => "Interface settings",
            PaintPanel::Log => "Log",
        }
    }

    fn icon(self) -> Icon {
        match self {
            PaintPanel::Viewport => Icon::Viewport,
            PaintPanel::TextureSets => Icon::Layers,
            PaintPanel::ImportReport => Icon::Report,
            PaintPanel::Display => Icon::Display,
            PaintPanel::Shortcuts => Icon::Keyboard,
            PaintPanel::Interface => Icon::Interface,
            PaintPanel::Log => Icon::Log,
        }
    }

    fn is_canvas(self) -> bool {
        self == PaintPanel::Viewport
    }
}

/// The layout a new user starts with.
pub const DEFAULT_LAYOUT: &str = "Painting";

/// The built-in layouts. "Baking" is where the bakers panel docks once it exists (ADR 0008 VQ18).
pub const PRESETS: &[Preset<PaintPanel>] = &[Preset { name: DEFAULT_LAYOUT, build: painting }, Preset { name: "Baking", build: baking }];

fn tabs(panels: &[PaintPanel]) -> Node<PaintPanel> {
    Node::Tabs { panels: panels.to_vec(), active: 0 }
}

fn split(axis: SplitAxis, size: SplitSize, first: Node<PaintPanel>, second: Node<PaintPanel>) -> Node<PaintPanel> {
    Node::Split { axis, size, first: Box::new(first), second: Box::new(second) }
}

/// Texture sets and the import report (with the log) on the left, the viewport, display settings on the right.
fn painting() -> Layout<PaintPanel> {
    let left = split(SplitAxis::Vertical, SplitSize::Ratio(0.45), tabs(&[PaintPanel::TextureSets]), tabs(&[PaintPanel::ImportReport, PaintPanel::Log]));
    let centre = split(SplitAxis::Horizontal, SplitSize::FixedSecond(270.0), tabs(&[PaintPanel::Viewport]), tabs(&[PaintPanel::Display]));
    Layout { root: Some(split(SplitAxis::Horizontal, SplitSize::FixedFirst(280.0), left, centre)), floating: Vec::new() }
}

/// A wide viewport with texture sets and display settings stacked on the right.
fn baking() -> Layout<PaintPanel> {
    let right = split(
        SplitAxis::Vertical,
        SplitSize::Ratio(0.4),
        tabs(&[PaintPanel::TextureSets]),
        tabs(&[PaintPanel::Display, PaintPanel::ImportReport, PaintPanel::Log]),
    );
    Layout { root: Some(split(SplitAxis::Horizontal, SplitSize::FixedSecond(300.0), tabs(&[PaintPanel::Viewport]), right)), floating: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_workspace::dock::check;

    #[test]
    fn ids_are_unique_and_every_preset_is_valid() {
        for p in PaintPanel::ALL {
            assert_eq!(PaintPanel::from_id(p.id()), Some(*p));
        }
        for preset in PRESETS {
            check(&(preset.build)()).unwrap_or_else(|e| panic!("{}: {e}", preset.name));
        }
    }
}
