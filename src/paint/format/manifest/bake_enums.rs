//! File ids of the baker's enumerations, for [`super::bake_config`].

use crate::bake::{Attenuation, PositionBounds, PositionMode, RayDistribution, TangentSource};

/// An enumeration stored by id.
pub trait EnumId: Sized + Copy + 'static {
    const ALL: &'static [Self];
    fn id(self) -> &'static str;

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|v| v.id() == id)
    }
}

impl EnumId for TangentSource {
    const ALL: &'static [Self] = &[TangentSource::Mesh, TangentSource::Generated];
    fn id(self) -> &'static str {
        match self {
            TangentSource::Mesh => "mesh",
            TangentSource::Generated => "generated",
        }
    }
}

impl EnumId for RayDistribution {
    const ALL: &'static [Self] = &[RayDistribution::Uniform, RayDistribution::Cosine];
    fn id(self) -> &'static str {
        match self {
            RayDistribution::Uniform => "uniform",
            RayDistribution::Cosine => "cosine",
        }
    }
}

impl EnumId for Attenuation {
    const ALL: &'static [Self] = &[Attenuation::None, Attenuation::Smooth, Attenuation::Linear];
    fn id(self) -> &'static str {
        match self {
            Attenuation::None => "none",
            Attenuation::Smooth => "smooth",
            Attenuation::Linear => "linear",
        }
    }
}

impl EnumId for PositionMode {
    const ALL: &'static [Self] = &[PositionMode::Uniform, PositionMode::PerAxis];
    fn id(self) -> &'static str {
        match self {
            PositionMode::Uniform => "uniform",
            PositionMode::PerAxis => "per_axis",
        }
    }
}

impl EnumId for PositionBounds {
    const ALL: &'static [Self] = &[PositionBounds::Scene, PositionBounds::Set, PositionBounds::High];
    fn id(self) -> &'static str {
        match self {
            PositionBounds::Scene => "scene",
            PositionBounds::Set => "set",
            PositionBounds::High => "high",
        }
    }
}
