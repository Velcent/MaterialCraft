//! [`ProjectSettings`]: choices made when a project is created that later steps read: the normal map convention of
//! exports and bakes, where the viewport builds the tangent frame, and whether fills placed by material drops use the
//! physical size. Changing the first two later means rebaking the normal maps, so they belong to the document.
//!
//! Every normal map inside a document is stored in the OpenGL convention (see [`super::normal`]); the format here
//! says how normal maps cross the document's boundary (baked maps read from files, exported maps).

/// The direction of the green channel in tangent-space normal maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NormalMapFormat {
    /// X right, Y up, Z out (green points up).
    #[default]
    OpenGl,
    /// X right, Y down, Z out (green points down).
    DirectX,
}

impl NormalMapFormat {
    pub const ALL: [NormalMapFormat; 2] = [NormalMapFormat::OpenGl, NormalMapFormat::DirectX];

    /// The id stored in files.
    pub fn id(self) -> &'static str {
        match self {
            NormalMapFormat::OpenGl => "opengl",
            NormalMapFormat::DirectX => "directx",
        }
    }

    pub fn from_id(id: &str) -> Option<NormalMapFormat> {
        NormalMapFormat::ALL.into_iter().find(|f| f.id() == id)
    }

    /// A short label for menus.
    pub fn label(self) -> &'static str {
        match self {
            NormalMapFormat::OpenGl => "OpenGL (Y+)",
            NormalMapFormat::DirectX => "DirectX (Y-)",
        }
    }

    /// True if maps in this format have their green channel flipped against the document's storage.
    pub fn flips_green(self) -> bool {
        self == NormalMapFormat::DirectX
    }
}

/// Document-wide settings chosen when the project was made (`project` in `.mcpaint` since 0.17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProjectSettings {
    /// The convention of normal maps read into and written out of the document.
    pub normal_map_format: NormalMapFormat,
    /// Build the bitangent per pixel instead of per vertex when the viewport decodes normal maps (as some engines
    /// do). Normal maps baked for one choice look wrong with the other, so changing it means rebaking.
    pub tangent_per_fragment: bool,
    /// Fill layers made by dropping a material use the physical size of the material instead of UV tiling.
    pub fill_physical_size: bool,
}

impl ProjectSettings {
    pub fn is_default(&self) -> bool {
        *self == ProjectSettings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for f in NormalMapFormat::ALL {
            assert_eq!(NormalMapFormat::from_id(f.id()), Some(f));
        }
        assert_eq!(NormalMapFormat::from_id("vulkan"), None);
        assert!(ProjectSettings::default().is_default());
        assert!(NormalMapFormat::DirectX.flips_green() && !NormalMapFormat::OpenGl.flips_green());
    }
}
