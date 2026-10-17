//! [`Params::from_painter`]: Painter's baking settings for one set mapped onto our [`BakeSettings`], with
//! every setting we cannot reproduce listed as [`Unmapped`] (so reports say what limits parity).

use super::painter::{MaterialBaking, TweakList, TweakValue};
use super::Params;
use materialcraft::bake::BakeSettings;
use materialcraft::paint::BakedKind;

/// Painter's map usages (`DataAdditionalMap.usage`, `BakingDefinition.usage`).
pub mod usage {
    pub const ID: u64 = 0;
    pub const AO: u64 = 1;
    pub const CURVATURE: u64 = 2;
    pub const WORLD_NORMAL: u64 = 3;
    pub const POSITION: u64 = 4;
    pub const THICKNESS: u64 = 5;
    pub const NORMAL: u64 = 6;
}

/// Painter's baker usage of a baked map kind (`None` for kinds Painter has no baker for).
pub fn baked_usage(kind: &BakedKind) -> Option<u64> {
    Some(match kind {
        BakedKind::Id => usage::ID,
        BakedKind::AmbientOcclusion => usage::AO,
        BakedKind::Curvature => usage::CURVATURE,
        BakedKind::WorldSpaceNormal => usage::WORLD_NORMAL,
        BakedKind::Position => usage::POSITION,
        BakedKind::Thickness => usage::THICKNESS,
        BakedKind::Normal => usage::NORMAL,
        BakedKind::Other(_) => return None,
    })
}

/// A Painter setting our baker has no counterpart for. `usage` = the baker it belongs to (`None` = all).
#[derive(Debug, Clone, PartialEq)]
pub struct Unmapped {
    pub usage: Option<u64>,
    pub text: String,
}

/// Detail settings that change nothing inside the islands or only steer inputs we resolve elsewhere.
const NO_EFFECT: &[&str] = &[
    "Detail.LowAsHigh",
    "Detail.HipolyMesh",
    "Detail.CageMesh",
    "Detail.LowpolySuffix",
    "Detail.HipolySuffix",
    "Detail.BackfaceCullingSuffix",
    "Detail.MaxHeight",
    "Detail.MaxDepth",
    "Detail.RelativeScale",
];

impl Params {
    /// Maps `painter` (the set's settings, or none) onto a bake of `size`² texels.
    pub fn from_painter(painter: Option<&MaterialBaking>, size: usize) -> Params {
        let mut bake = BakeSettings {
            size,
            ..BakeSettings::default()
        };
        let mut unmapped = Vec::new();
        let mut note = |usage: Option<u64>, text: String| unmapped.push(Unmapped { usage, text });
        let Some(p) = painter else {
            note(None, "no baking settings in the project; our defaults used".into());
            return Params::new(bake, unmapped);
        };
        if let Some(w) = p.common.f32("Common.DilationWidth") {
            bake.padding = w.max(0.0) as usize;
        }
        map_distances(&p.detail, &mut bake, &mut note);
        for (k, v) in &p.detail.tweaks {
            let differs = match k.as_str() {
                "Detail.SmoothNormals" => p.detail.bool(k) == Some(false),
                "Detail.UseCage" | "Detail.IgnoreBackface" => p.detail.bool(k) == Some(true),
                "Detail.SubSampling" | "Detail.FilterMethod" => v.as_f32() != Some(0.0),
                key => !NO_EFFECT.contains(&key),
            };
            if differs {
                note(None, format!("{k} = {v}"));
            }
        }
        for baker in p.bakers.iter().filter(|b| b.enabled) {
            let usage = baker.usage;
            for (k, v) in &baker.params.tweaks {
                let mapped = match (usage, k.as_str()) {
                    (Some(usage::AO), "Detail.SecondaryCommon.NbSecondary") => {
                        bake.ao_rays = v.as_f32().unwrap_or(32.0).max(1.0) as u32;
                        true
                    }
                    (Some(usage::AO), "Detail.SecondaryCommon.MaxDistance") => {
                        bake.ao_dist = v.as_f32().unwrap_or(bake.ao_dist);
                        true
                    }
                    (Some(usage::AO), "Detail.SecondaryCommon.MaxDistanceRelativeScale") => {
                        matches!(v, TweakValue::Bool(true))
                    }
                    _ => false,
                };
                if !mapped {
                    note(usage, format!("{} {k} = {v}", short_baker(&baker.params)));
                }
            }
        }
        if let Some(t) = p
            .baker(usage::THICKNESS)
            .and_then(|b| b.params.f32("Detail.SecondaryCommon.MaxDistance"))
        {
            note(
                Some(usage::THICKNESS),
                format!(
                    "our thickness uses the AO distance {} instead of its own {t}",
                    bake.ao_dist
                ),
            );
        }
        Params::new(bake, unmapped)
    }
}

/// Painter's frontal/rear distances (relative to the mesh size when `RelativeScale`) → our one symmetric
/// distance relative to the high mesh's bounding-box diagonal.
fn map_distances(detail: &TweakList, bake: &mut BakeSettings, note: &mut impl FnMut(Option<u64>, String)) {
    let (h, d) = (detail.f32("Detail.MaxHeight"), detail.f32("Detail.MaxDepth"));
    if let Some(max) = h.into_iter().chain(d).reduce(f32::max) {
        bake.max_dist = max.max(1e-5);
    }
    if h != d {
        note(
            None,
            format!("frontal {h:?} / rear {d:?} distances differ; ours is symmetric (max used)"),
        );
    }
    if detail.bool("Detail.RelativeScale") == Some(false) {
        note(None, "absolute ray distances used as relative ones".into());
    }
}

/// `GLMapBakerManager.AOFromDetail` → `AOFromDetail`.
fn short_baker(list: &TweakList) -> &str {
    list.baker.rsplit('.').next().unwrap_or(&list.baker)
}
