//! [`ProjectionMode`]: the seven modes and the settings only some of them have (the projection volume, falloffs,
//! the warp grid). The volume-based modes are data only in this version.

/// The box a volume-based projection works in, relative to the [`super::SceneFrame`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volume {
    /// In scene units (centre of the whole scene's bounds = 0).
    pub offset: [f32; 3],
    /// Degrees about x, y and z.
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    /// Stop at the volume's edge instead of continuing beyond it.
    pub clip: bool,
}

impl Default for Volume {
    fn default() -> Volume {
        Volume { offset: [0.0; 3], rotation: [0.0; 3], scale: [1.0; 3], clip: false }
    }
}

/// A fade along the projection depth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Falloff {
    pub on: bool,
    /// 0..1.
    pub hardness: f32,
}

impl Default for Falloff {
    fn default() -> Falloff {
        Falloff { on: false, hardness: 0.5 }
    }
}

/// A cut-off on faces turned away from the projection direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacingFalloff {
    pub on: bool,
    /// Degrees from the projection direction where the cut-off sits.
    pub angle: f32,
    /// 0..1.
    pub hardness: f32,
}

impl Default for FacingFalloff {
    fn default() -> FacingFalloff {
        FacingFalloff { on: false, angle: 90.0, hardness: 0.5 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TriPlanar {
    pub volume: Volume,
    /// 0..1; 1 is a hard cut between the three planes.
    pub blend_hardness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Planar {
    pub volume: Volume,
    pub depth_falloff: Falloff,
    pub facing_falloff: FacingFalloff,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Spherical {
    pub volume: Volume,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cylindrical {
    pub volume: Volume,
    pub facing_falloff: FacingFalloff,
    /// Degrees of the circumference the image covers (360 = all of it).
    pub coverage_angle: f32,
    pub hide_caps: bool,
}

impl Default for Cylindrical {
    fn default() -> Cylindrical {
        Cylindrical { volume: Volume::default(), facing_falloff: FacingFalloff::default(), coverage_angle: 360.0, hide_caps: false }
    }
}

/// One control point of a warp grid.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WarpPoint {
    /// Position in the volume's local frame (−1…1).
    pub pos: [f32; 3],
    /// Image coordinate of the point.
    pub uv: [f32; 2],
    /// Bézier handles: previous and next along u, previous and next along v.
    pub tangents: [[f32; 3]; 4],
}

/// A grid of `rows × columns` control points forming a smooth patch.
#[derive(Debug, Clone, PartialEq)]
pub struct WarpGrid {
    pub rows: u32,
    pub columns: u32,
    /// Projection depth along the grid's local z.
    pub depth: f32,
    /// Row by row; `rows × columns` points.
    pub points: Vec<WarpPoint>,
}

impl Default for WarpGrid {
    fn default() -> WarpGrid {
        WarpGrid { rows: 3, columns: 3, depth: 0.1, points: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Warp {
    pub volume: Volume,
    pub depth_falloff: Falloff,
    pub grid: WarpGrid,
}

/// A projection this version does not know (from a newer file): kept whole, not computable.
#[derive(Debug, Clone, PartialEq)]
pub struct UnknownMode {
    /// The mode id as stored (may be empty when another value was unknown).
    pub mode: String,
    /// The whole projection object as JSON text, written back unchanged.
    pub json: String,
}

/// How the source reaches the texels. Texture space and UV live in UV space; the others project from a volume
/// around the mesh.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ProjectionMode {
    /// The source 1:1 over the UV tile; no placement.
    #[default]
    TextureSpace,
    /// The image placement applied in UV space; the same image covers every UV tile.
    Uv,
    TriPlanar(TriPlanar),
    Planar(Planar),
    Spherical(Spherical),
    Cylindrical(Cylindrical),
    Warp(Warp),
    Unknown(UnknownMode),
}

impl ProjectionMode {
    /// Stable id as used in files.
    pub fn id(&self) -> &str {
        match self {
            ProjectionMode::TextureSpace => "texture_space",
            ProjectionMode::Uv => "uv",
            ProjectionMode::TriPlanar(_) => "tri_planar",
            ProjectionMode::Planar(_) => "planar",
            ProjectionMode::Spherical(_) => "spherical",
            ProjectionMode::Cylindrical(_) => "cylindrical",
            ProjectionMode::Warp(_) => "warp",
            ProjectionMode::Unknown(u) => &u.mode,
        }
    }

    /// The default settings of the mode with `id`, or `None` for an id this version does not know.
    pub fn default_of(id: &str) -> Option<ProjectionMode> {
        Some(match id {
            "texture_space" => ProjectionMode::TextureSpace,
            "uv" => ProjectionMode::Uv,
            "tri_planar" => ProjectionMode::TriPlanar(TriPlanar { blend_hardness: 0.5, ..TriPlanar::default() }),
            "planar" => ProjectionMode::Planar(Planar::default()),
            "spherical" => ProjectionMode::Spherical(Spherical::default()),
            "cylindrical" => ProjectionMode::Cylindrical(Cylindrical::default()),
            "warp" => ProjectionMode::Warp(Warp::default()),
            _ => return None,
        })
    }

    /// True for the modes that project from a volume and so need the mesh's surface.
    pub fn needs_mesh(&self) -> bool {
        matches!(
            self,
            ProjectionMode::TriPlanar(_) | ProjectionMode::Planar(_) | ProjectionMode::Spherical(_) | ProjectionMode::Cylindrical(_) | ProjectionMode::Warp(_)
        )
    }

    /// The projection volume, for the volume-based modes.
    pub fn volume(&self) -> Option<&Volume> {
        match self {
            ProjectionMode::TriPlanar(m) => Some(&m.volume),
            ProjectionMode::Planar(m) => Some(&m.volume),
            ProjectionMode::Spherical(m) => Some(&m.volume),
            ProjectionMode::Cylindrical(m) => Some(&m.volume),
            ProjectionMode::Warp(m) => Some(&m.volume),
            _ => None,
        }
    }
}
