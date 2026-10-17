//! [`bake_set`]: one texture set from start to finish: rasterise the low surface, project it onto the high mesh,
//! run the selected bakers, average sub-samples, dilate, and report to the diagnostics sink as it goes.

use super::cancel::CancelToken;
use super::context::Ctx;
use super::error::BakeError;
use super::high::{Facing, HighScene};
use super::low::{LowSurface, TextureSetRef};
use super::maps::{BakedMaps, MapKind};
use super::output::{dilate, downsample};
use super::project::{Reach, Samples};
use super::settings::BakeSettings;
use super::{ao, curvature, id, normal, position, thickness, world_normal};
use crate::diag::{Area, Level, Sink, SinkExt, Target};
use crate::mesh::Mesh;
use crate::tex::Tex;
use std::time::Instant;

/// Largest sample grid side (output size x samples per side).
const MAX_GRID: usize = 8192;
/// Below this share of covered texels finding the high mesh, the log warns about the ray distances.
const LOW_HIT_SHARE: f32 = 0.9;

/// Bakes the maps `settings.maps` of one texture set of `low` from `high`. For "low as high", pass
/// `HighScene::from_mesh(low)`. Entries about the bake go to `log` (area [`Area::Bake`]); `cancel` stops it between
/// stages and every few thousand texels.
pub fn bake_set(
    low: &Mesh,
    set: &TextureSetRef,
    high: &HighScene,
    settings: &BakeSettings,
    cancel: &CancelToken,
    log: &dyn Sink,
) -> Result<BakedMaps, BakeError> {
    let result = run(low, set, high, settings, cancel, log);
    match &result {
        Err(BakeError::Cancelled) => log.info(Area::Bake, "bake cancelled"),
        Err(e) => log.error(Area::Bake, format!("bake failed: {e}")),
        Ok(_) => {}
    }
    result
}

fn run(low: &Mesh, set: &TextureSetRef, high: &HighScene, s: &BakeSettings, cancel: &CancelToken, log: &dyn Sink) -> Result<BakedMaps, BakeError> {
    if s.maps.is_empty() || s.size == 0 {
        return Err(BakeError::NothingToBake);
    }
    if high.triangle_count() == 0 {
        return Err(BakeError::NoHighTriangles);
    }
    let size = s.size.clamp(16, MAX_GRID);
    let mut sub = s.samples.clamp(1, 4) as usize;
    while sub > 1 && size * sub > MAX_GRID {
        sub -= 1;
    }
    let started = Instant::now();
    log.info(Area::Bake, format!("baking {size} x {size} ({sub} x {sub} samples per texel) against {} high triangles", high.triangle_count()));
    let surface = LowSurface::build(low, set, size * sub, s.tangents, s.average_normals)?;
    check(cancel)?;
    let scale = high.scale();
    let reach =
        Reach { front: s.max_dist.max(0.0) * scale, rear: s.rear().max(0.0) * scale, facing: if s.ignore_backfaces { Facing::Front } else { Facing::Any } };
    let samples = Samples::project(&surface, high, reach, cancel);
    check(cancel)?;
    let share = samples.hit_share();
    if share < LOW_HIT_SHARE {
        let text = format!("{:.1} % of the texels found no high surface within the ray distances; they show the low surface", 100.0 * (1.0 - share));
        log.about(Level::Warning, Area::Bake, Target::BakeSetting("max_dist".into()), text);
    }
    let ctx =
        Ctx { samples: &samples, high, settings: s, scale, sub: sub as u32, set_bounds: surface.set_bounds(), scene_bounds: surface.scene_bounds(), cancel };
    let covered: Vec<bool> = samples.texels.iter().map(Option::is_some).collect();
    let width = samples.width;
    let mut out = BakedMaps::default();
    for kind in s.maps.kinds() {
        let t0 = Instant::now();
        let values = bake_one(kind, &ctx);
        check(cancel)?;
        let (mut px, cover) = downsample(&values, &covered, width, sub);
        dilate(&mut px, &cover, size, s.padding.min(MAX_GRID));
        out.maps.insert(kind, Tex { w: size, h: size, px });
        log.debug(Area::Bake, format!("{kind}: {:.2} s", t0.elapsed().as_secs_f32()));
    }
    let hits: Vec<[f32; 4]> = samples.texels.iter().map(|t| if t.as_ref().is_some_and(|s| s.hit.is_some()) { [1.0; 4] } else { [0.0; 4] }).collect();
    out.hit_mask = downsample(&hits, &covered, width, sub).0.iter().map(|p| p[0]).collect();
    log.info(Area::Bake, format!("baked {} map(s) in {:.1} s", out.maps.len(), started.elapsed().as_secs_f32()));
    Ok(out)
}

fn bake_one(kind: MapKind, ctx: &Ctx) -> Vec<[f32; 4]> {
    match kind {
        MapKind::Normal => normal::bake(ctx),
        MapKind::WorldNormal => world_normal::bake(ctx),
        MapKind::Position => position::bake(ctx),
        MapKind::AmbientOcclusion => ao::bake(ctx),
        MapKind::Curvature => curvature::bake(ctx),
        MapKind::Thickness => thickness::bake(ctx),
        MapKind::Id => id::bake(ctx),
    }
}

fn check(cancel: &CancelToken) -> Result<(), BakeError> {
    if cancel.is_cancelled() { Err(BakeError::Cancelled) } else { Ok(()) }
}
