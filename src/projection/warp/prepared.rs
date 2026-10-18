//! [`PreparedWarp`]: the warp projector for one pass, and the cache of [`WarpSurface`]s it shares between passes
//! (a fill is prepared once per pass; the surface and its lookup only change with the grid).
//!
//! Per texel: the surface point in the volume's local frame, doubled into grid units; its x-y located on the
//! grid's surface; the image coordinate there, placed by the image placement. Coverage: the distance from the
//! surface along the grid's z, cut at the projection depth on either side and shaped by the depth falloff (ADR
//! 0008 mc2-q-pj-16: the one falloff model, here over the projection depth), times the volume clip.

use super::super::falloff;
use super::super::modes::{Falloff, Warp, WarpGrid};
use super::super::placement::UvPlacement;
use super::super::projector::{SurfacePoint, Tap, Taps};
use super::super::volume::VolumeTransform;
use super::grid::GridError;
use super::surface::WarpSurface;
use std::sync::{Arc, Mutex};

/// Cells of the inverse lookup along the larger side of the grid.
pub const LOOKUP_RESOLUTION: usize = 256;
/// Grid units per local unit of the volume (the grid spans −1…1, the volume −0.5…0.5).
pub const GRID_PER_LOCAL: f32 = 2.0;
/// Surfaces kept for reuse.
const CACHED: usize = 8;

static CACHE: Mutex<Vec<(WarpGrid, Arc<WarpSurface>)>> = Mutex::new(Vec::new());

/// The surface of `grid`, built once and shared while it stays among the recently used ones.
pub fn surface_for(grid: &WarpGrid) -> Result<Arc<WarpSurface>, GridError> {
    let key = WarpGrid { depth: 0.0, ..grid.clone() };
    if let Ok(cache) = CACHE.lock()
        && let Some((_, s)) = cache.iter().find(|(k, _)| *k == key)
    {
        return Ok(Arc::clone(s));
    }
    let surface = Arc::new(WarpSurface::new(grid, LOOKUP_RESOLUTION)?);
    if let Ok(mut cache) = CACHE.lock() {
        cache.retain(|(k, _)| *k != key);
        cache.insert(0, (key, Arc::clone(&surface)));
        cache.truncate(CACHED);
    }
    Ok(surface)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedWarp {
    pub vol: VolumeTransform,
    pub place: UvPlacement,
    pub falloff: Falloff,
    /// Projection depth on either side of the surface (grid units).
    pub depth: f32,
    pub surface: Arc<WarpSurface>,
}

impl PreparedWarp {
    pub fn new(m: &Warp, vol: VolumeTransform, place: UvPlacement) -> Result<PreparedWarp, GridError> {
        let depth = if m.grid.depth.is_finite() { m.grid.depth.max(0.0) } else { 0.0 };
        Ok(PreparedWarp { vol, place, falloff: m.depth_falloff, depth, surface: surface_for(&m.grid)? })
    }

    #[inline]
    pub fn project(&self, p: &SurfacePoint) -> Taps {
        let l = self.vol.local(p.pos);
        let g = l.map(|v| f64::from(v * GRID_PER_LOCAL));
        let Some(hit) = self.surface.locate([g[0], g[1]]) else { return Taps::default() };
        let dz = (g[2] - hit.pos[2]) as f32;
        if dz.is_nan() || dz.abs() > self.depth {
            return Taps::default();
        }
        let coverage = self.vol.clip_coverage(l) * falloff::depth(&self.falloff, dz, self.depth);
        let local = if p.world_len > 0.0 && p.world_len.is_finite() { self.vol.local_len(p.world_len) } else { p.texel_len };
        let footprint = local * GRID_PER_LOCAL * hit.image_per_grid as f32 * self.place.scale();
        let uv = [hit.uv[0] as f32, hit.uv[1] as f32];
        let tap = Tap { uv: self.place.apply(uv), weight: 1.0, footprint };
        Taps { taps: [tap, Tap::default(), Tap::default()], n: 1, coverage }
    }
}
