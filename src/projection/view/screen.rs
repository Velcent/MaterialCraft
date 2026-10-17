//! Images laid over the view: [`ScreenImageTransform`] places one on screen (move, turn, scale, tile), and
//! [`ScreenProjector`] maps texels through a [`ViewProjector`] onto it, so a fill or a brush can paint through it.

use super::ViewProjector;
use crate::projection::data::Repeat;
use crate::projection::projector::{Projector, SurfacePoint, Taps};

/// Where an image sits on screen: its centre, the size of one repetition and its rotation, plus along which axes
/// it repeats. Image coordinates run 0 … 1 across one repetition (origin top-left, v down, like UVs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenImageTransform {
    /// Screen position of the image's centre (viewport units).
    pub centre: [f32; 2],
    /// On-screen width and height of one repetition (viewport units; negative mirrors that axis).
    pub size: [f32; 2],
    /// Counter-clockwise turn on screen, in degrees.
    pub rotation: f32,
    pub repeat: Repeat,
}

impl ScreenImageTransform {
    /// An image of `aspect` (width / height) centred on a viewport of `viewport` units, as large as fits
    /// unrotated, not repeating.
    pub fn fitted(aspect: f32, viewport: [f32; 2]) -> ScreenImageTransform {
        let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
        let (w, h) = (viewport[0].max(1.0), viewport[1].max(1.0));
        let size = if w / h > aspect { [h * aspect, h] } else { [w, w / aspect] };
        ScreenImageTransform { centre: [w * 0.5, h * 0.5], size, rotation: 0.0, repeat: Repeat::None }
    }

    /// The image coordinate at a screen position.
    pub fn to_image(&self, screen: [f32; 2]) -> [f32; 2] {
        let (dx, dy) = (screen[0] - self.centre[0], screen[1] - self.centre[1]);
        // undo the turn: screen y points down, so a counter-clockwise turn on screen is clockwise in (x, y)
        let (s, c) = self.rotation.to_radians().sin_cos();
        let (x, y) = (c * dx - s * dy, s * dx + c * dy);
        [x / nonzero(self.size[0]) + 0.5, y / nonzero(self.size[1]) + 0.5]
    }

    /// The screen position of an image coordinate.
    pub fn to_screen(&self, image: [f32; 2]) -> [f32; 2] {
        let (x, y) = ((image[0] - 0.5) * self.size[0], (image[1] - 0.5) * self.size[1]);
        let (s, c) = self.rotation.to_radians().sin_cos();
        [self.centre[0] + c * x + s * y, self.centre[1] - s * x + c * y]
    }

    /// True if the image covers image coordinate `uv`: always along repeating axes, within 0 … 1 otherwise.
    pub fn covers(&self, uv: [f32; 2]) -> bool {
        let inside = |v: f32| (0.0..=1.0).contains(&v);
        (self.repeat.u() || inside(uv[0])) && (self.repeat.v() || inside(uv[1]))
    }

    /// Moves the image by `delta` screen units.
    pub fn translate(&mut self, delta: [f32; 2]) {
        self.centre = [self.centre[0] + delta[0], self.centre[1] + delta[1]];
    }

    /// Scales the image by `factor` about the screen point `pivot`.
    pub fn scale_about(&mut self, factor: f32, pivot: [f32; 2]) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        self.size = [self.size[0] * factor, self.size[1] * factor];
        self.centre = [pivot[0] + (self.centre[0] - pivot[0]) * factor, pivot[1] + (self.centre[1] - pivot[1]) * factor];
    }

    /// Turns the image by `degrees` (counter-clockwise) about the screen point `pivot`.
    pub fn rotate_about(&mut self, degrees: f32, pivot: [f32; 2]) {
        if !degrees.is_finite() {
            return;
        }
        let (s, c) = degrees.to_radians().sin_cos();
        let (dx, dy) = (self.centre[0] - pivot[0], self.centre[1] - pivot[1]);
        self.centre = [pivot[0] + c * dx + s * dy, pivot[1] - s * dx + c * dy];
        self.rotation = (self.rotation + degrees).rem_euclid(360.0);
    }

    /// The rotation rounded to the nearest multiple of `step` degrees (rotation snapping; a step ≤ 0 keeps it).
    pub fn snapped_rotation(&self, step: f32) -> f32 {
        if step.is_finite() && step > 0.0 { ((self.rotation / step).round() * step).rem_euclid(360.0) } else { self.rotation }
    }

    /// Image units per screen unit (the larger axis): the image footprint of one screen unit.
    pub fn image_per_screen(&self) -> f32 {
        1.0 / nonzero(self.size[0].abs().min(self.size[1].abs()))
    }
}

/// A [`Projector`] whose image lives in screen space: each texel is projected through the view onto the image.
/// Coverage is the texel's visibility (occlusion and facing) where the image covers it; the brush decides the
/// stroke mask and blending (ADR 0006 Q-PJ-07).
pub struct ScreenProjector<'a> {
    pub view: &'a dyn ViewProjector,
    pub image: ScreenImageTransform,
}

impl Projector for ScreenProjector<'_> {
    fn project(&self, p: &SurfacePoint) -> Taps {
        let Some(s) = self.view.to_screen(p.pos) else {
            return Taps { coverage: 0.0, ..Taps::default() };
        };
        let uv = self.image.to_image(s.pos);
        // the footprint of one screen unit; a texel's own size on screen needs the surface's derivatives
        let mut taps = Taps::one(uv, self.image.image_per_screen());
        taps.coverage = if self.image.covers(uv) { self.view.visibility(p.pos, p.normal) } else { 0.0 };
        taps
    }
}

fn nonzero(v: f32) -> f32 {
    if v.is_finite() && v.abs() > 1e-12 { v } else { 1.0 }
}
