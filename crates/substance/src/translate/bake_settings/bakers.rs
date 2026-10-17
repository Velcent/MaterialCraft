//! Painter's per-baker settings mapped onto ours. Enumerations are numbered as Painter stores them; which of our
//! modes each number means was decided by measuring against the user's bakes (parity README), not from any
//! documentation.

use super::usage;
use crate::model::baking::TweakValue;
use materialcraft::bake::{Attenuation, BakeSettings, CurvatureRange, PositionBounds, PositionMode, RayDistribution};

/// A boolean as Painter stores it (a bool, or 0/1 in older files).
pub fn flag(v: &TweakValue) -> Option<bool> {
    match v {
        TweakValue::Bool(b) => Some(*b),
        TweakValue::Int(i) => Some(*i != 0),
        _ => None,
    }
}

fn int(v: &TweakValue) -> Option<i64> {
    match v {
        TweakValue::Int(i) => Some(*i),
        TweakValue::Bool(b) => Some(i64::from(*b)),
        _ => None,
    }
}

/// One setting of the baker of `usage`; false when we have no counterpart (it is then listed as unmapped).
pub fn map_baker(usage: Option<u64>, key: &str, v: &TweakValue, b: &mut BakeSettings) -> bool {
    match (usage, key) {
        // shared by the bakers that cast secondary rays
        (_, "Detail.SecondaryCommon.MaxDistanceRelativeScale") => flag(v) == Some(true),
        (_, "Detail.SecondaryCommon.FilterMethodSecondary") => v.as_f32() == Some(0.0),
        (Some(usage::AO), k) => map_ao(k, v, b),
        (Some(usage::CURVATURE), k) => map_curvature(k, v, b),
        (Some(usage::POSITION), k) => map_position(k, v, b),
        (Some(usage::THICKNESS), k) => map_thickness(k, v, b),
        _ => false,
    }
}

fn distribution(v: &TweakValue) -> Option<RayDistribution> {
    match int(v)? {
        0 => Some(RayDistribution::Uniform),
        1 => Some(RayDistribution::Cosine),
        _ => None,
    }
}

fn map_ao(key: &str, v: &TweakValue, b: &mut BakeSettings) -> bool {
    match key {
        "Detail.SecondaryCommon.NbSecondary" => v.as_f32().map(|n| b.ao_rays = n.max(1.0) as u32).is_some(),
        "Detail.SecondaryCommon.MaxDistance" => v.as_f32().map(|d| b.ao_dist = d.max(1e-5)).is_some(),
        "Detail.SecondaryCommon.MinDistance" => v.as_f32().map(|d| b.ao.min_dist = d.max(0.0)).is_some(),
        "Detail.SecondaryCommon.Spread" => v.as_f32().map(|s| b.ao.spread = s.clamp(0.0, 180.0)).is_some(),
        "Detail.SecondaryCommon.Distribution" => distribution(v).map(|d| b.ao.distribution = d).is_some(),
        "Detail.AO.Attenuation" => {
            let a = match int(v) {
                Some(0) => Attenuation::None,
                Some(1) => Attenuation::Smooth,
                Some(2) => Attenuation::Linear,
                _ => return false,
            };
            b.ao.attenuation = a;
            true
        }
        // no ground plane: nothing to do (an offset without the plane changes nothing)
        "Detail.AO.GroundPlane" => flag(v) == Some(false),
        "Detail.AO.GroundOffset" => true,
        _ => false,
    }
}

fn map_curvature(key: &str, v: &TweakValue, b: &mut BakeSettings) -> bool {
    match key {
        "Detail.Curvature1.SamplingRadius" => v.as_f32().map(|r| b.curvature.radius = r.max(1e-6)).is_some(),
        "Detail.SecondaryCommon.NbSecondary" => v.as_f32().map(|n| b.curvature.rays = n.max(1.0) as u32).is_some(),
        "Detail.Curvature1.AutoMinMax" => match flag(v) {
            Some(true) => {
                b.curvature.range = CurvatureRange::Auto;
                true
            }
            _ => false,
        },
        // the tone mapping range only matters without the automatic range; ours is symmetric like the default
        "Detail.Curvature1.TonemappingMin" => v.as_f32() == Some(-1.0),
        "Detail.Curvature1.TonemappingMax" => v.as_f32() == Some(1.0),
        _ => false,
    }
}

fn map_position(key: &str, v: &TweakValue, b: &mut BakeSettings) -> bool {
    match key {
        // mode 0 (all axes) with normalisation 1 (bounding box) is our uniform mode; the axis is then unused
        "Position.Mode" => int(v) == Some(0),
        "Position.Axis" => true,
        "Position.Normalization" => match int(v) {
            Some(1) => {
                b.position.mode = PositionMode::Uniform;
                true
            }
            _ => false,
        },
        "Position.NormalizationScale" => {
            b.position.bounds = match int(v) {
                Some(0) => PositionBounds::Set,
                Some(1) => PositionBounds::Scene,
                _ => return false,
            };
            true
        }
        _ => false,
    }
}

fn map_thickness(key: &str, v: &TweakValue, b: &mut BakeSettings) -> bool {
    match key {
        "Detail.SecondaryCommon.NbSecondary" => v.as_f32().map(|n| b.thickness.rays = n.max(1.0) as u32).is_some(),
        "Detail.SecondaryCommon.MaxDistance" => v.as_f32().map(|d| b.thickness.max_dist = d.max(1e-5)).is_some(),
        "Detail.SecondaryCommon.Distribution" => distribution(v).map(|d| b.thickness.distribution = d).is_some(),
        "Detail.SecondaryCommon.Spread" => v.as_f32() == Some(180.0),
        "Detail.SecondaryCommon.MinDistance" => v.as_f32().is_some_and(|d| d < 1e-4),
        "Detail.Distance.MaximizeRange" => flag(v).map(|m| b.thickness.maximise_range = m).is_some(),
        _ => false,
    }
}
