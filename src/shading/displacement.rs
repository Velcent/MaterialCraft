//! Displacement and tessellation settings of a shader instance (VS-07 … VS-11).
//!
//! OpenGL 3.3 has no tessellation stage, so tessellation is CPU subdivision of the imported mesh
//! ([`crate::mesh::MeshPart::subdivided`]) by a whole number of levels; each level splits every triangle into four.
//! The edge-length mode picks the level that brings the longest edge under the limit (uniform, not adaptive).

use crate::mesh::LengthUnit;

/// The channel displacement reads (VS-07). Only height today: a dedicated displacement channel needs a new
/// channel id, which is a format decision of its own (paint-mode Q13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplacementSource {
    #[default]
    Height,
}

impl DisplacementSource {
    pub const ALL: [(DisplacementSource, &'static str); 1] = [(DisplacementSource::Height, "Height")];

    pub fn id(self) -> &'static str {
        match self {
            DisplacementSource::Height => "height",
        }
    }

    pub fn from_id(id: &str) -> Option<DisplacementSource> {
        DisplacementSource::ALL.iter().map(|(s, _)| *s).find(|s| s.id() == id)
    }
}

/// What the displacement scale is measured in (VS-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplacementUnit {
    /// A fraction of the model's bounding-box diagonal: looks the same on any mesh, whatever its unit.
    #[default]
    Relative,
    /// The mesh's own units.
    SceneUnits,
    /// Centimetres, through the mesh's length unit.
    Centimetres,
}

impl DisplacementUnit {
    pub const ALL: [(DisplacementUnit, &'static str); 3] =
        [(DisplacementUnit::Relative, "Relative to size"), (DisplacementUnit::SceneUnits, "Scene units"), (DisplacementUnit::Centimetres, "Centimetres")];

    pub fn id(self) -> &'static str {
        match self {
            DisplacementUnit::Relative => "relative",
            DisplacementUnit::SceneUnits => "scene",
            DisplacementUnit::Centimetres => "cm",
        }
    }

    pub fn from_id(id: &str) -> Option<DisplacementUnit> {
        DisplacementUnit::ALL.iter().map(|(u, _)| *u).find(|u| u.id() == id)
    }
}

/// How far the height channel moves the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Displacement {
    /// Off: the viewport's own height display applies (Display settings), as before shader instances.
    pub enabled: bool,
    pub source: DisplacementSource,
    /// The distance of height 1 above `midlevel`, in `unit`.
    pub scale: f32,
    pub unit: DisplacementUnit,
    /// The height that stays on the surface (0 = everything rises, 0.5 = centred).
    pub midlevel: f32,
}

impl Default for Displacement {
    fn default() -> Self {
        Displacement { enabled: false, source: DisplacementSource::Height, scale: 0.02, unit: DisplacementUnit::Relative, midlevel: 0.0 }
    }
}

/// The model's size and unit, for converting a displacement scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneScale {
    /// Bounding-box diagonal of the whole model, in its units.
    pub diagonal: f32,
    /// The mesh's length unit; `None` when the source didn't say (OBJ).
    pub unit: Option<LengthUnit>,
}

/// Something the user should know about a displacement distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitWarning {
    /// Centimetres were asked for but the mesh has no known unit: metres were assumed (glTF's convention).
    UnknownUnit,
}

impl UnitWarning {
    pub fn message(self) -> &'static str {
        match self {
            UnitWarning::UnknownUnit => "The mesh has no known unit; 1 unit = 1 m is assumed. Set the mesh unit.",
        }
    }
}

impl Displacement {
    /// The displacement distance of height 1 above the midlevel, in the mesh's units (0 for broken values).
    pub fn distance(&self, scene: &SceneScale) -> (f32, Option<UnitWarning>) {
        let scale = if self.scale.is_finite() { self.scale } else { 0.0 };
        let (d, warning) = match self.unit {
            DisplacementUnit::Relative => (scale * scene.diagonal, None),
            DisplacementUnit::SceneUnits => (scale, None),
            DisplacementUnit::Centimetres => match scene.unit {
                Some(u) => (scale * 0.01 / u.metres(), None),
                None => (scale * 0.01, Some(UnitWarning::UnknownUnit)),
            },
        };
        (if d.is_finite() { d } else { 0.0 }, warning)
    }

    /// The midlevel within 0 … 1 (0 for a broken value).
    pub fn midlevel(&self) -> f32 {
        if self.midlevel.is_finite() { self.midlevel.clamp(0.0, 1.0) } else { 0.0 }
    }
}

/// How the mesh is subdivided for the viewport (VS-09, VS-10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TessellationMode {
    Off,
    /// A fixed number of levels (each doubles the segments per edge: 1 … [`MAX_LEVELS`] = 2 … 32).
    Uniform {
        levels: u32,
    },
    /// Enough levels that the longest edge is at most this fraction of the bounding-box diagonal.
    EdgeLength {
        max_relative: f32,
    },
}

/// Most subdivision levels: 32 segments per original edge.
pub const MAX_LEVELS: u32 = 5;
/// Levels from which the panel warns (VS-09).
pub const WARN_LEVELS: u32 = 4;
/// The most triangles subdivision may produce for the whole model; levels are lowered to stay under it.
pub const TRIANGLE_BUDGET: usize = 8_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tessellation {
    pub mode: TessellationMode,
    /// Loop smoothing (the surface rounds off) instead of flat splitting (the surface stays where it is).
    pub smooth: bool,
}

impl Default for Tessellation {
    fn default() -> Self {
        Tessellation { mode: TessellationMode::Off, smooth: false }
    }
}

/// The subdivision a tessellation setting comes to for one model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TessellationPlan {
    pub levels: u32,
    /// Triangles after subdivision.
    pub triangles: usize,
    /// The setting asked for more levels than the triangle budget allows.
    pub limited: bool,
}

impl Tessellation {
    /// The levels the setting asks for, before the budget.
    pub fn requested_levels(&self, longest_edge: f32, diagonal: f32) -> u32 {
        match self.mode {
            TessellationMode::Off => 0,
            TessellationMode::Uniform { levels } => levels.min(MAX_LEVELS),
            TessellationMode::EdgeLength { max_relative } => {
                let limit = max_relative * diagonal;
                if !(limit > 0.0 && limit.is_finite() && longest_edge.is_finite()) || longest_edge <= limit {
                    return 0;
                }
                ((longest_edge / limit).log2().ceil() as u32).min(MAX_LEVELS)
            }
        }
    }

    /// The levels for a model of `triangles` triangles whose longest edge is `longest_edge`, within the budget.
    pub fn plan(&self, triangles: usize, longest_edge: f32, diagonal: f32) -> TessellationPlan {
        let wanted = self.requested_levels(longest_edge, diagonal);
        let mut levels = wanted;
        while levels > 0 && subdivided_count(triangles, levels) > TRIANGLE_BUDGET {
            levels -= 1;
        }
        TessellationPlan { levels, triangles: subdivided_count(triangles, levels), limited: levels < wanted }
    }
}

/// Triangles after `levels` subdivisions of `triangles` (saturating).
pub fn subdivided_count(triangles: usize, levels: u32) -> usize {
    triangles.saturating_mul(4usize.saturating_pow(levels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_in_every_unit() {
        let scene = SceneScale { diagonal: 2.0, unit: Some(LengthUnit::Centimetre) };
        let d = |unit, scale| Displacement { enabled: true, scale, unit, ..Displacement::default() }.distance(&scene);
        assert_eq!(d(DisplacementUnit::Relative, 0.5), (1.0, None));
        assert_eq!(d(DisplacementUnit::SceneUnits, 0.5), (0.5, None));
        let (cm, w) = d(DisplacementUnit::Centimetres, 3.0);
        assert!((cm - 3.0).abs() < 1e-5 && w.is_none(), "1 unit = 1 cm: {cm}");
        let metres = SceneScale { unit: None, ..scene };
        let (m, w) = Displacement { scale: 50.0, unit: DisplacementUnit::Centimetres, ..Displacement::default() }.distance(&metres);
        assert!((m - 0.5).abs() < 1e-6);
        assert_eq!(w, Some(UnitWarning::UnknownUnit));
        assert_eq!(Displacement { scale: f32::NAN, ..Displacement::default() }.distance(&scene).0, 0.0);
    }

    #[test]
    fn tessellation_levels_follow_the_mode_and_the_budget() {
        let t = |mode| Tessellation { mode, smooth: false };
        assert_eq!(t(TessellationMode::Off).plan(100, 1.0, 1.0).levels, 0);
        assert_eq!(t(TessellationMode::Uniform { levels: 9 }).plan(100, 1.0, 1.0).levels, MAX_LEVELS);
        // longest edge 1, diagonal 10, limit 1 % = 0.1: 10x → 4 levels (16 segments)
        let e = t(TessellationMode::EdgeLength { max_relative: 0.01 }).plan(100, 1.0, 10.0);
        assert_eq!((e.levels, e.triangles, e.limited), (4, 100 * 256, false));
        assert_eq!(t(TessellationMode::EdgeLength { max_relative: 0.5 }).plan(100, 1.0, 10.0).levels, 0);
        let big = t(TessellationMode::Uniform { levels: 5 }).plan(1_000_000, 1.0, 1.0);
        assert_eq!((big.levels, big.limited), (1, true));
        assert!(big.triangles <= TRIANGLE_BUDGET);
    }
}
