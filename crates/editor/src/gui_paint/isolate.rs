//! The isolated-mask view (MK-05): one layer's mask shown alone, unlit and grey, in place of its texture
//! set's material. The mask is evaluated on a reduced grid (at most [`EDGE`] texels on a side), which is
//! plenty for judging a mask on screen and keeps 4K sets cheap.

use materialcraft::generators;
use materialcraft::gpu3d::{self, GpuMaps, MapInputs};
use materialcraft::paint::TextureSet;
use materialcraft::paint::layer_preview::PreviewGrid;
use materialcraft::tex::Tex;

/// Longer edge of the grid the mask is evaluated on.
pub const EDGE: u32 = 1024;

/// GPU maps showing layer `layer`'s mask, or `None` if the layer or its mask is gone.
pub fn mask_maps(set: &TextureSet, layer: &str) -> Option<GpuMaps> {
    let layer = set.layer(layer)?;
    let grid = PreviewGrid::new(set, EDGE);
    let mask = grid.mask(&grid.reduce(layer), generators::builtin())?;
    let black = Tex::solid(1, 1, [0.0, 0.0, 0.0, 1.0]);
    let rough = Tex::solid(1, 1, [1.0; 4]);
    Some(gpu3d::prepare(&MapInputs { base: Some(&black), normal: None, rough: Some(&rough), metal: None, ao: None, emissive: Some(&mask), height: None }))
}
