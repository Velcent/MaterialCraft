//! Per-frame camera variations for frame accumulation (PP-02, PP-04): a sub-pixel offset of the image
//! ([`Camera::jittered`]) for anti-aliasing and a point on the lens ([`Camera::through_lens`]) for depth of field,
//! with the sample sequences that spread them. Sample 0 is always the plain camera, so the first frame of an
//! accumulation is the frame drawn while moving.

use super::snapshot::Camera;
use std::f32::consts::PI;

/// The radical inverse of `i` in `base` (Halton sequence), in 0 … 1.
fn radical_inverse(mut i: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0f32, 0.0f32);
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// The sub-pixel offset of accumulation sample `index`, in pixels (−0.5 … 0.5 on each axis); (0, 0) for sample 0.
pub fn pixel_jitter(index: u32) -> [f32; 2] {
    if index == 0 {
        return [0.0, 0.0];
    }
    [radical_inverse(index, 2) - 0.5, radical_inverse(index, 3) - 0.5]
}

/// The point on a unit disc for lens sample `index` of `count`, evenly spread (a sunflower pattern); the centre for
/// sample 0.
pub fn lens_point(index: u32, count: u32) -> [f32; 2] {
    let count = count.max(1) as f32;
    let r = (index as f32 / count).sqrt();
    // golden angle
    let a = index as f32 * PI * (3.0 - 5f32.sqrt());
    [r * a.cos(), r * a.sin()]
}

impl Camera {
    /// The camera with its image shifted by `pixels` (x right, y down) on a viewport of `size` pixels.
    pub fn jittered(&self, pixels: [f32; 2], size: [f32; 2]) -> Camera {
        let ndc = [2.0 * pixels[0] / size[0].max(1.0), -2.0 * pixels[1] / size[1].max(1.0)];
        let mut c = *self;
        shift_clip(&mut c.projection, ndc);
        c
    }

    /// The camera seen through the point `offset` (world units along its right and up axes) of its lens, keeping
    /// everything at view distance `focus` where it was: what is nearer or farther moves, which accumulated
    /// over the lens gives depth of field. Unchanged for a parallel projection or a non-positive `focus`.
    pub fn through_lens(&self, offset: [f32; 2], focus: f32) -> Camera {
        if self.is_orthographic() || focus.is_nan() || focus <= 0.0 {
            return *self;
        }
        let mut c = *self;
        // move the eye: world → view, then subtract the offset in view space
        c.view[12] -= offset[0];
        c.view[13] -= offset[1];
        // shift the image back by where the focus plane moved
        let p = &self.projection;
        shift_clip(&mut c.projection, [p[0] * offset[0] / focus, p[5] * offset[1] / focus]);
        c
    }
}

/// Adds `ndc` (normalised device units) to every projected point: clip x += ndc.x · w, clip y += ndc.y · w.
fn shift_clip(m: &mut [f32; 16], ndc: [f32; 2]) {
    for column in m.chunks_exact_mut(4) {
        if let [x, y, _, w] = column {
            *x += ndc[0] * *w;
            *y += ndc[1] * *w;
        }
    }
}
