//! What an asset is used for (AS-03, AS-04): the tray's type filter and the target of a drop.

use std::fmt;

/// The usage of an asset. It comes from the asset's own sidecar, else from the nearest folder named
/// after a usage, else from its file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Usage {
    /// A material: an `.mca` graph used as a fill or brush material.
    Material,
    /// A layer-stack fragment with its own masks and effects.
    SmartMaterial,
    /// A mask fragment: a mask with its effect stack.
    SmartMask,
    /// Produces a mask or value from mesh maps (built-in generator preset or a graph with a mask output).
    Generator,
    /// Adjusts what lies below it in the stack.
    Filter,
    /// A brush preset: an alpha plus behaviour settings.
    Brush,
    /// A greyscale brush tip or stamp.
    Alpha,
    /// An image: grunge, procedural texture, baked map, normal detail, LUT.
    Texture,
    /// An HDR environment map for viewport lighting.
    Environment,
}

impl Usage {
    /// Every usage, in the tray's display order.
    pub const ALL: [Usage; 9] = [
        Usage::Material,
        Usage::SmartMaterial,
        Usage::SmartMask,
        Usage::Generator,
        Usage::Filter,
        Usage::Brush,
        Usage::Alpha,
        Usage::Texture,
        Usage::Environment,
    ];

    /// Stable keyword, used in sidecars, saved searches and the `usage:` query field.
    pub fn keyword(self) -> &'static str {
        match self {
            Usage::Material => "material",
            Usage::SmartMaterial => "smart-material",
            Usage::SmartMask => "smart-mask",
            Usage::Generator => "generator",
            Usage::Filter => "filter",
            Usage::Brush => "brush",
            Usage::Alpha => "alpha",
            Usage::Texture => "texture",
            Usage::Environment => "environment",
        }
    }

    /// The folder this usage gets in the standard library layout (AS-02).
    pub fn folder(self) -> &'static str {
        match self {
            Usage::Material => "materials",
            Usage::SmartMaterial => "smart-materials",
            Usage::SmartMask => "smart-masks",
            Usage::Generator => "generators",
            Usage::Filter => "filters",
            Usage::Brush => "brushes",
            Usage::Alpha => "alphas",
            Usage::Texture => "textures",
            Usage::Environment => "environments",
        }
    }

    /// Parse a keyword or folder name, ignoring case, spaces, `-` and `_` ("Smart Materials",
    /// "smart_material" and "smart-materials" all give [`Usage::SmartMaterial`]).
    pub fn from_name(name: &str) -> Option<Usage> {
        let squashed: String = name.chars().filter(|c| !matches!(c, ' ' | '-' | '_')).flat_map(char::to_lowercase).collect();
        let singular = squashed.strip_suffix("es").filter(|s| s.ends_with("sh")).or_else(|| squashed.strip_suffix('s')).unwrap_or(&squashed);
        Usage::ALL.into_iter().find(|u| u.keyword().replace('-', "") == singular)
    }
}

impl fmt::Display for Usage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.keyword())
    }
}
