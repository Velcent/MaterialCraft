//! The brush cursor: where the brush would paint and how big it is. A brush sized on the model shows its footprint as
//! a circle lying on the surface under the pointer (so it tilts with the surface and shrinks with distance); a brush
//! sized on the screen or in the texture shows a screen circle.

use super::overlay::{Aid, Overlay};
use materialcraft::mesh::Vec3;
use materialcraft::paint::brush::SizeSpace;
use materialcraft::projection::{SurfaceHit, ViewProjector};

/// Points on the footprint's outline.
const SEGMENTS: usize = 48;
/// Smallest drawn radius in screen points, so a tiny brush stays visible.
const MIN_RADIUS: f32 = 2.0;

/// The brush's size as the cursor needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorSize {
    pub space: SizeSpace,
    /// The preset's size (a fraction of the model's bounding radius for [`SizeSpace::Model`], else screen points).
    pub size: f32,
    /// The model's bounding-sphere radius (object units).
    pub model_radius: f32,
}

impl CursorSize {
    /// Radius of the footprint in object units (model-sized brushes only).
    fn object_radius(self) -> f32 {
        self.size * self.model_radius * 0.5
    }
}

/// Adds the cursor for a pointer at `pointer` (view coordinates) to `out`: on `hit` when the brush is sized on the
/// model, else around the pointer. A model-sized brush off the mesh shows nothing (it would not paint).
pub fn add_cursor(out: &mut Overlay, view: Option<&dyn ViewProjector>, hit: Option<SurfaceHit>, pointer: [f32; 2], size: CursorSize) {
    match size.space {
        SizeSpace::Model => {
            let (Some(view), Some(hit)) = (view, hit) else { return };
            let outline = footprint(view, hit.position, hit.normal, size.object_radius());
            match outline.len() {
                n if n >= 3 => {
                    for (i, a) in outline.iter().enumerate() {
                        if let Some(b) = outline.get((i + 1) % n) {
                            out.lines.push((*a, *b, Aid::Brush, false));
                        }
                    }
                }
                _ => out.rings.push((pointer, MIN_RADIUS, Aid::Brush)),
            }
            if let Some(c) = view.to_screen(hit.position) {
                out.dots.push((c.pos, Aid::Brush));
            }
        }
        SizeSpace::Screen | SizeSpace::Texture => out.rings.push((pointer, (size.size * 0.5).max(MIN_RADIUS), Aid::Brush)),
    }
}

/// The screen outline of a circle of `radius` (object units) around `centre` in the plane with `normal`; empty when
/// it is behind the camera or smaller than [`MIN_RADIUS`] on screen.
pub fn footprint(view: &dyn ViewProjector, centre: Vec3, normal: Vec3, radius: f32) -> Vec<[f32; 2]> {
    let (u, v) = tangents(normal);
    let mut points = Vec::with_capacity(SEGMENTS);
    for i in 0..SEGMENTS {
        let a = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        let p = [0, 1, 2].map(|k| centre[k] + radius * (c * u[k] + s * v[k]));
        match view.to_screen(p) {
            Some(sp) => points.push(sp.pos),
            None => return Vec::new(),
        }
    }
    let Some(c) = view.to_screen(centre) else { return Vec::new() };
    let extent = points.iter().map(|p| (p[0] - c.pos[0]).hypot(p[1] - c.pos[1])).fold(0.0_f32, f32::max);
    if extent < MIN_RADIUS { Vec::new() } else { points }
}

/// Two unit vectors spanning the plane with `normal`.
fn tangents(normal: Vec3) -> (Vec3, Vec3) {
    let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    let n = if len > 1e-6 { normal.map(|x| x / len) } else { [0.0, 0.0, 1.0] };
    let helper = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let u = normalise(cross(n, helper));
    let v = cross(n, u);
    (u, v)
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalise(a: Vec3) -> Vec3 {
    let len = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if len > 1e-12 { a.map(|x| x / len) } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::camera::{Camera, IDENTITY};
    use materialcraft::projection::ScreenPoint;
    use materialcraft::projection::gizmo::Ray;

    /// Looks down −z: 100 screen points per object unit, object origin at (200, 200), y down.
    struct Ortho(Camera);

    impl ViewProjector for Ortho {
        fn camera(&self) -> &Camera {
            &self.0
        }
        fn pick(&self, _: [f32; 2]) -> Option<SurfaceHit> {
            None
        }
        fn to_screen(&self, p: Vec3) -> Option<ScreenPoint> {
            Some(ScreenPoint { pos: [200.0 + 100.0 * p[0], 200.0 - 100.0 * p[1]], depth: 0.5, view_depth: 1.0 })
        }
        fn ray(&self, _: [f32; 2]) -> Option<Ray> {
            None
        }
        fn occluded(&self, _: Vec3) -> bool {
            false
        }
        fn facing(&self, _: Vec3, n: Vec3) -> f32 {
            n[2]
        }
    }

    fn view() -> Ortho {
        Ortho(Camera::new(IDENTITY, IDENTITY, [400.0, 400.0]))
    }

    fn radii(points: &[[f32; 2]]) -> (f32, f32) {
        let d = points.iter().map(|p| (p[0] - 200.0).hypot(p[1] - 200.0));
        (d.clone().fold(f32::MAX, f32::min), d.fold(0.0, f32::max))
    }

    #[test]
    fn a_surface_facing_the_camera_gives_a_round_footprint_of_the_brush_size() {
        let points = footprint(&view(), [0.0; 3], [0.0, 0.0, 1.0], 0.5);
        assert_eq!(points.len(), SEGMENTS);
        let (lo, hi) = radii(&points);
        assert!((lo - 50.0).abs() < 1e-3 && (hi - 50.0).abs() < 1e-3, "{lo} {hi}");
    }

    #[test]
    fn a_tilted_surface_flattens_the_footprint() {
        let tilt = std::f32::consts::FRAC_PI_3; // 60°: the short axis is cos 60° = half
        let points = footprint(&view(), [0.0; 3], [tilt.sin(), 0.0, tilt.cos()], 0.5);
        let (lo, hi) = radii(&points);
        assert!((hi - 50.0).abs() < 0.1 && (lo - 25.0).abs() < 0.5, "{lo} {hi}");
    }

    #[test]
    fn model_sized_cursor_needs_the_surface_and_screen_sized_follows_the_pointer() {
        let model = CursorSize { space: SizeSpace::Model, size: 0.1, model_radius: 10.0 };
        let mut out = Overlay::default();
        add_cursor(&mut out, Some(&view()), None, [5.0, 5.0], model);
        assert_eq!(out, Overlay::default(), "off the mesh nothing would paint");
        let screen = CursorSize { space: SizeSpace::Screen, size: 40.0, model_radius: 10.0 };
        add_cursor(&mut out, None, None, [5.0, 5.0], screen);
        assert_eq!(out.rings, vec![([5.0, 5.0], 20.0, Aid::Brush)]);
    }
}
