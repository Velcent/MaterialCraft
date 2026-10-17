//! Which part of the program an entry comes from, and which log panel tab shows it (LG-01, LG-04, VQ14).

use std::fmt;

/// The subsystem that produced an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Area {
    /// Anything without a better home.
    General,
    /// Opening and saving the project document.
    Project,
    /// Exporting textures.
    Export,
    /// Importing projects and meshes made elsewhere.
    Import,
    /// Loading library assets (materials, brushes, images).
    Assets,
    /// Document consistency checks.
    Checks,
    /// Baking mesh maps.
    Bake,
    /// Viewport rendering and the GPU.
    Gpu,
    /// Extensions and plugins.
    Extensions,
}

impl Area {
    /// Every area, in declaration order.
    pub const ALL: [Area; 9] = [Area::General, Area::Project, Area::Export, Area::Import, Area::Assets, Area::Checks, Area::Bake, Area::Gpu, Area::Extensions];

    /// Stable lowercase name, for log files.
    pub fn name(self) -> &'static str {
        match self {
            Area::General => "general",
            Area::Project => "project",
            Area::Export => "export",
            Area::Import => "import",
            Area::Assets => "assets",
            Area::Checks => "checks",
            Area::Bake => "bake",
            Area::Gpu => "gpu",
            Area::Extensions => "extensions",
        }
    }

    /// The log panel tab this area's entries appear under: General holds project, export,
    /// extension and GPU/renderer messages; Assets holds asset errors, imports and checks; Bake
    /// has its own tab (ADR 0006 VQ14).
    pub fn tab(self) -> Tab {
        match self {
            Area::General | Area::Project | Area::Export | Area::Gpu | Area::Extensions => Tab::General,
            Area::Import | Area::Assets | Area::Checks => Tab::Assets,
            Area::Bake => Tab::Bake,
        }
    }
}

impl fmt::Display for Area {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A tab of the log panel; each [`Area`] belongs to exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tab {
    General,
    Assets,
    Bake,
}

impl Tab {
    /// Every tab, in display order.
    pub const ALL: [Tab; 3] = [Tab::General, Tab::Assets, Tab::Bake];
}
