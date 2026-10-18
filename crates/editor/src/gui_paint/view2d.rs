//! [`View2d`]: the 2D (UV) view's pan and zoom, and the transforms tools use (V2-08): screen ↔ UV, UV → texel, the
//! working area test (V2-05) and the camera `gpu3d` draws the view with. Screen positions are view coordinates (UI
//! points from the view's top-left corner); UV follows the mesh's convention (v down, as image rows).
//!
//! Pure state and maths: the widget ([`super::uv_view`]) turns input into calls here; nothing here draws.

use materialcraft::camera::Camera;
use materialcraft::gpu3d;

/// Pan and zoom of the 2D view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View2d {
    /// The UV point at the view's centre.
    pub centre: [f32; 2],
    /// UI points per UV unit.
    pub zoom: f32,
}

/// What the view shows when nothing is known yet: the 0–1 square in a 512-point view.
impl Default for View2d {
    fn default() -> View2d {
        View2d { centre: [0.5, 0.5], zoom: 460.0 }
    }
}

impl View2d {
    /// Zoom limits in points per UV unit: the 0–1 square 8 points wide … 64 points per texel of a 4K texture.
    pub const ZOOM: (f32, f32) = (8.0, 4096.0 * 64.0);
    /// The working area's margin when fitting, as a fraction of the view.
    const FIT_MARGIN: f32 = 0.06;

    /// The 0–1 square centred in a view of `size` points, with a small margin (V2-09 fit to view).
    pub fn fit(size: [f32; 2]) -> View2d {
        let side = size[0].min(size[1]) * (1.0 - 2.0 * Self::FIT_MARGIN);
        View2d { centre: [0.5, 0.5], zoom: side.clamp(Self::ZOOM.0, Self::ZOOM.1) }
    }

    /// Where `uv` is shown in a view of `size` points.
    pub fn uv_to_screen(&self, uv: [f32; 2], size: [f32; 2]) -> [f32; 2] {
        [(uv[0] - self.centre[0]) * self.zoom + size[0] * 0.5, (uv[1] - self.centre[1]) * self.zoom + size[1] * 0.5]
    }

    /// The UV point under `screen` in a view of `size` points.
    pub fn screen_to_uv(&self, screen: [f32; 2], size: [f32; 2]) -> [f32; 2] {
        let z = self.zoom.max(f32::MIN_POSITIVE);
        [(screen[0] - size[0] * 0.5) / z + self.centre[0], (screen[1] - size[1] * 0.5) / z + self.centre[1]]
    }

    /// Moves the picture by `delta` points.
    pub fn pan(&mut self, delta: [f32; 2]) {
        let z = self.zoom.max(f32::MIN_POSITIVE);
        self.centre = [self.centre[0] - delta[0] / z, self.centre[1] - delta[1] / z];
    }

    /// Zooms by `factor` keeping the UV point under `at` (view coordinates) where it is.
    pub fn zoom_at(&mut self, factor: f32, at: [f32; 2], size: [f32; 2]) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let fixed = self.screen_to_uv(at, size);
        self.zoom = (self.zoom * factor).clamp(Self::ZOOM.0, Self::ZOOM.1);
        let moved = self.uv_to_screen(fixed, size);
        self.pan([at[0] - moved[0], at[1] - moved[1]]);
    }

    /// Screen points per texel of a texture `texels` wide (the zoom readout: 1 = one point per texel).
    pub fn points_per_texel(&self, texels: u32) -> f32 {
        self.zoom / texels.max(1) as f32
    }

    /// The camera `gpu3d::paint_uv` draws a view of `size` points with.
    pub fn camera(&self, size: [f32; 2]) -> Camera {
        gpu3d::uv_camera(self.centre, size[1].max(1.0) / self.zoom.max(f32::MIN_POSITIVE), size)
    }
}

/// The 2D view as laid out this frame: its pan and zoom and its size, for the tools.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvMap {
    pub view: View2d,
    /// The view's size in points.
    pub size: [f32; 2],
}

impl UvMap {
    /// The UV point under the view position `screen`.
    pub fn uv(&self, screen: [f32; 2]) -> [f32; 2] {
        self.view.screen_to_uv(screen, self.size)
    }

    /// The texel position (texel units, 0 … width) under `screen` in a texture of `resolution` texels: where a 2D
    /// stroke paints.
    pub fn texel(&self, screen: [f32; 2], resolution: (u32, u32)) -> [f32; 2] {
        let uv = self.uv(screen);
        [uv[0] * resolution.0 as f32, uv[1] * resolution.1 as f32]
    }

    /// Where the texel position `texel` of a texture of `resolution` texels is shown.
    pub fn screen_of_texel(&self, texel: [f32; 2], resolution: (u32, u32)) -> [f32; 2] {
        let uv = [texel[0] / resolution.0.max(1) as f32, texel[1] / resolution.1.max(1) as f32];
        self.view.uv_to_screen(uv, self.size)
    }
}

/// True inside the working area of an ordinary project: the 0–1 square (V2-05).
pub fn in_working_area(uv: [f32; 2]) -> bool {
    (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
    }

    #[test]
    fn screen_and_uv_round_trip_and_match_the_gpu_camera() {
        let size = [640.0, 400.0];
        let v = View2d { centre: [0.3, 0.6], zoom: 900.0 };
        let camera = v.camera(size);
        for uv in [[0.0, 0.0], [0.3, 0.6], [0.41, 0.52], [1.0, 1.0]] {
            let s = v.uv_to_screen(uv, size);
            assert!(close(v.screen_to_uv(s, size), uv));
            // the GPU draws the UV point where the overlays and tools expect it
            let p = camera.project([uv[0], -uv[1], 0.0]).expect("in view");
            assert!(close(p.screen, s), "{uv:?}: camera {:?}, view2d {s:?}", p.screen);
        }
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_and_fit_shows_the_square() {
        let size = [500.0, 300.0];
        let mut v = View2d::fit(size);
        assert!(close(v.uv_to_screen([0.5, 0.5], size), [250.0, 150.0]));
        let top = v.uv_to_screen([0.5, 0.0], size)[1];
        assert!(top > 0.0 && top < 30.0, "the square fits the height with a margin: {top}");
        let at = [100.0, 80.0];
        let under = v.screen_to_uv(at, size);
        v.zoom_at(3.0, at, size);
        assert!(close(v.screen_to_uv(at, size), under));
        v.zoom_at(1e-9, at, size);
        assert_eq!(v.zoom, View2d::ZOOM.0, "clamped");
        v.zoom_at(f32::NAN, at, size);
        assert_eq!(v.zoom, View2d::ZOOM.0, "ignored");
    }

    #[test]
    fn tools_get_texels_and_the_working_area() {
        let map = UvMap { view: View2d { centre: [0.5, 0.5], zoom: 1024.0 }, size: [1024.0, 1024.0] };
        // one point per texel of a 1024 texture: the view's corner is texel 0, 0
        assert!(close(map.texel([0.0, 0.0], (1024, 1024)), [0.0, 0.0]));
        assert!(close(map.texel([10.5, 20.5], (2048, 2048)), [21.0, 41.0]));
        assert!(close(map.screen_of_texel([21.0, 41.0], (2048, 2048)), [10.5, 20.5]));
        assert_eq!(map.view.points_per_texel(2048), 0.5);
        assert!(in_working_area([0.0, 1.0]) && !in_working_area([1.01, 0.5]) && !in_working_area([0.5, -0.1]));
    }
}
