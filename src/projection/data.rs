//! The settings every projection shares: [`Filter`], [`Repeat`], [`ImagePlacement`] (with [`SizeMode`]), and
//! [`Projection`], which combines them with a mode.

use super::modes::ProjectionMode;
use crate::symmetry::SymmetrySpec;

/// How the source image is sampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// Prefiltered (a level chain against shimmer under strong tiling), bilinear within a level.
    #[default]
    Smooth,
    /// Plain bilinear on the full-size image.
    Bilinear,
    /// The nearest texel.
    Nearest,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::Smooth, Filter::Bilinear, Filter::Nearest];

    pub fn id(self) -> &'static str {
        match self {
            Filter::Smooth => "smooth",
            Filter::Bilinear => "bilinear",
            Filter::Nearest => "nearest",
        }
    }

    pub fn from_id(id: &str) -> Option<Filter> {
        Filter::ALL.into_iter().find(|f| f.id() == id)
    }
}

/// Along which image axes the source repeats. Outside a non-repeating image the fill has no coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
    None,
    U,
    V,
    #[default]
    Both,
}

impl Repeat {
    pub const ALL: [Repeat; 4] = [Repeat::None, Repeat::U, Repeat::V, Repeat::Both];

    pub fn id(self) -> &'static str {
        match self {
            Repeat::None => "none",
            Repeat::U => "u",
            Repeat::V => "v",
            Repeat::Both => "both",
        }
    }

    pub fn from_id(id: &str) -> Option<Repeat> {
        Repeat::ALL.into_iter().find(|r| r.id() == id)
    }

    pub fn u(self) -> bool {
        matches!(self, Repeat::U | Repeat::Both)
    }

    pub fn v(self) -> bool {
        matches!(self, Repeat::V | Repeat::Both)
    }
}

/// How the image's size on the surface is given (ADR 0008: three equal modes wherever sizing exists).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SizeMode {
    /// Repetitions per projection unit ([`ImagePlacement::tiling`]).
    #[default]
    Tiling,
    /// The source's real-world size against the mesh's units (not evaluated yet: needs mesh units).
    Physical,
    /// A size typed in ([`ImagePlacement::physical_size`]), overriding the source's (not evaluated yet).
    Custom,
}

impl SizeMode {
    pub const ALL: [SizeMode; 3] = [SizeMode::Tiling, SizeMode::Physical, SizeMode::Custom];

    pub fn id(self) -> &'static str {
        match self {
            SizeMode::Tiling => "tiling",
            SizeMode::Physical => "physical",
            SizeMode::Custom => "custom",
        }
    }

    pub fn from_id(id: &str) -> Option<SizeMode> {
        SizeMode::ALL.into_iter().find(|m| m.id() == id)
    }
}

/// Where the image sits inside the projection: tiling per axis (negative = mirrored), rotation about the centre
/// and offset. The default centres the image once over the projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImagePlacement {
    /// Repetitions along u and v (size mode [`SizeMode::Tiling`]).
    pub tiling: [f32; 2],
    /// Degrees, counter-clockwise in image space.
    pub rotation: f32,
    /// In projection units (UV mode: UV units from the UV centre). Ignored by tri-planar.
    pub offset: [f32; 2],
    pub size_mode: SizeMode,
    /// Width and height in centimetres for [`SizeMode::Physical`] / [`SizeMode::Custom`], if known.
    pub physical_size: Option<[f32; 2]>,
}

impl Default for ImagePlacement {
    fn default() -> ImagePlacement {
        ImagePlacement { tiling: [1.0, 1.0], rotation: 0.0, offset: [0.0, 0.0], size_mode: SizeMode::Tiling, physical_size: None }
    }
}

impl ImagePlacement {
    /// Tiling 1, no rotation or offset, size mode tiling.
    pub fn is_identity(&self) -> bool {
        *self == ImagePlacement::default()
    }
}

/// How a fill's source is laid onto the texture set: a mode with its own settings plus the shared sampling and
/// placement. The default is texture space, which uses none of the shared settings except the filter.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Projection {
    pub mode: ProjectionMode,
    pub filter: Filter,
    pub repeat: Repeat,
    pub image: ImagePlacement,
    /// Fill symmetry (PJ-22, PT-40): the projection repeated at the symmetric copies; only the modes that
    /// [`ProjectionMode::takes_symmetry`] use it.
    pub symmetry: SymmetrySpec,
}

impl Projection {
    /// UV mode with `image` as the placement.
    pub fn uv(image: ImagePlacement) -> Projection {
        Projection { mode: ProjectionMode::Uv, image, ..Projection::default() }
    }

    /// The repetition the source is sampled with: tri-planar always repeats (ADR 0008), the others as set.
    pub fn sampling_repeat(&self) -> Repeat {
        match self.mode {
            ProjectionMode::TriPlanar(_) => Repeat::Both,
            _ => self.repeat,
        }
    }

    /// The symmetry the projection is evaluated with: none for the modes that take none.
    pub fn effective_symmetry(&self) -> SymmetrySpec {
        if self.mode.takes_symmetry() { self.symmetry } else { SymmetrySpec::None }
    }

    /// True for the default (texture space, smooth, repeating, identity placement): files leave it out.
    pub fn is_default(&self) -> bool {
        *self == Projection::default()
    }
}
