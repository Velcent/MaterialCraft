//! Diff heatmaps: error per texel as black → red → yellow (full yellow at 16/255), outside the mask dark blue.

use crate::tools::png_out::PngMap;
use materialcraft::tex::Tex;
use std::io;
use std::path::Path;

/// Error at which the colour scale saturates.
const FULL_SCALE: f32 = 16.0 / 255.0;

pub fn write_heatmap(errors: &[f32], mask: &[bool], width: usize, height: usize, path: &Path) -> io::Result<()> {
    let px = errors
        .iter()
        .zip(mask)
        .map(|(&e, &inside)| {
            if !inside {
                return [0.0, 0.0, 0.15, 1.0];
            }
            let t = (e / FULL_SCALE).clamp(0.0, 1.0);
            [(2.0 * t).min(1.0), (2.0 * t - 1.0).max(0.0), 0.0, 1.0]
        })
        .collect();
    PngMap { gray: false, bits: 8 }.write(
        &Tex {
            w: width,
            h: height,
            px,
        },
        path,
    )
}
