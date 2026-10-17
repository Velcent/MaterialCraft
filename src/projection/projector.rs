//! [`Projector`]: a surface point → image taps. [`Prepared`] is a [`Projection`] made ready for one pass; it
//! implements the projector by `match`, so the per-texel path has no dynamic dispatch. The trait exists for
//! projectors made elsewhere (the screen projector of projection painting, symmetry wrappers).

use super::data::Projection;
use super::frame::SceneFrame;
use super::modes::ProjectionMode;
use super::placement::UvPlacement;
use crate::mesh::Vec3;
use std::fmt;

/// What lies under one texel of a texture set.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SurfacePoint {
    /// Texel centre in UV space (0…1 over the texture set, rows top first).
    pub uv: [f32; 2],
    /// Object-space position (from the surface map; zero when there is none).
    pub pos: Vec3,
    /// Interpolated vertex normal (zero when there is none).
    pub normal: Vec3,
    /// Size of the texel in UV units (1 / resolution).
    pub texel_len: f32,
}

/// One sample position in the projected source.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tap {
    /// Image coordinate (0…1 across the image once; outside it repeats or is clipped).
    pub uv: [f32; 2],
    pub weight: f32,
    /// The texel's size in image units, for prefiltering.
    pub footprint: f32,
}

/// Up to three taps and the coverage of the projection at the point (clipping, falloffs).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Taps {
    pub taps: [Tap; 3],
    pub n: u8,
    pub coverage: f32,
}

impl Taps {
    /// A single tap with full weight and coverage.
    pub fn one(uv: [f32; 2], footprint: f32) -> Taps {
        let tap = Tap { uv, weight: 1.0, footprint };
        Taps { taps: [tap, Tap::default(), Tap::default()], n: 1, coverage: 1.0 }
    }

    /// The taps in use.
    pub fn used(&self) -> &[Tap] {
        self.taps.get(..usize::from(self.n).min(3)).unwrap_or(&[])
    }
}

/// Maps a surface point (one texel of a texture set) to sample positions in the projected source.
pub trait Projector: Send + Sync {
    fn project(&self, p: &SurfacePoint) -> Taps;
}

/// Why a projection cannot be evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotComputable {
    /// A volume-based mode without the mesh's surface (no mesh, or the set's UV set is missing).
    NeedsMesh { mode: String },
    /// A mode this version stores but does not evaluate yet.
    NotImplemented { mode: String },
    /// A mode (or another setting) from a newer version.
    Unknown { mode: String },
}

impl fmt::Display for NotComputable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotComputable::NeedsMesh { mode } => write!(f, "{mode} projection needs the mesh"),
            NotComputable::NotImplemented { mode } => write!(f, "{mode} projection is not evaluated yet"),
            NotComputable::Unknown { mode } => write!(f, "unknown projection {mode:?}"),
        }
    }
}

/// A projection ready to evaluate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Prepared {
    /// UV coordinate = image coordinate.
    TextureSpace,
    /// The placement applied to the UV coordinate.
    Uv(UvPlacement),
}

impl Projection {
    /// Precomputes what the per-texel path needs. `frame` is the scene (`None` without a mesh); `has_surface`
    /// tells whether a surface map is available for the texture set.
    pub fn prepare(&self, frame: Option<&SceneFrame>, has_surface: bool) -> Result<Prepared, NotComputable> {
        let mode = || self.mode.id().to_string();
        match &self.mode {
            ProjectionMode::TextureSpace => Ok(Prepared::TextureSpace),
            ProjectionMode::Uv => Ok(Prepared::Uv(UvPlacement::new(&self.image))),
            ProjectionMode::Unknown(_) => Err(NotComputable::Unknown { mode: mode() }),
            m if m.needs_mesh() && (frame.is_none() || !has_surface) => Err(NotComputable::NeedsMesh { mode: mode() }),
            _ => Err(NotComputable::NotImplemented { mode: mode() }),
        }
    }
}

impl Prepared {
    /// True if the result depends only on the UV coordinate (no surface map needed).
    pub fn uv_only(&self) -> bool {
        matches!(self, Prepared::TextureSpace | Prepared::Uv(_))
    }
}

impl Projector for Prepared {
    #[inline]
    fn project(&self, p: &SurfacePoint) -> Taps {
        match self {
            Prepared::TextureSpace => Taps::one(p.uv, p.texel_len),
            Prepared::Uv(m) => Taps::one(m.apply(p.uv), p.texel_len * m.scale()),
        }
    }
}
