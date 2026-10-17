//! The isolated-mask view (MK-05): one layer's mask shown alone, unlit and grey, in place of its texture
//! set's material. The mask is evaluated on a reduced grid (at most [`EDGE`] texels on a side), which is
//! plenty for judging a mask on screen and keeps 4K sets cheap.

use super::fill_surfaces::FillSurfaces;
use materialcraft::generators;
use materialcraft::gpu3d::{self, GpuMaps, MapInputs};
use materialcraft::paint::layer_preview::PreviewGrid;
use materialcraft::paint::{FillContext, NoFillSources, TextureSet};
use materialcraft::tex::Tex;

/// Longer edge of the grid the mask is evaluated on.
pub const EDGE: u32 = 1024;

/// GPU maps showing layer `layer`'s mask, or `None` if the layer or its mask is gone. Mask fills projected from a
/// volume use the set's surface at the grid's size from `surfaces` (built here if needed).
pub fn mask_maps(set: &TextureSet, layer: &str, surfaces: Option<&FillSurfaces>) -> Option<GpuMaps> {
    let layer = set.layer(layer)?;
    let grid = PreviewGrid::new(set, EDGE);
    let surface = surfaces.filter(|_| set.needs_surface()).and_then(|s| s.build(set, grid.size()));
    let fills = surface.as_ref().map_or_else(|| FillContext::with_sources(&NoFillSources), |s| s.context(&NoFillSources));
    let mask = grid.mask(&grid.reduce(layer), generators::builtin(), &fills)?;
    let black = Tex::solid(1, 1, [0.0, 0.0, 0.0, 1.0]);
    let rough = Tex::solid(1, 1, [1.0; 4]);
    Some(gpu3d::prepare(&MapInputs { base: Some(&black), normal: None, rough: Some(&rough), metal: None, ao: None, emissive: Some(&mask), height: None }))
}
