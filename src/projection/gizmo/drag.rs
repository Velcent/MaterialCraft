//! [`VolumeDrag`]: one drag of a gizmo handle, from the volume and pointer ray it started with to the volume under
//! the pointer's current ray. Every update starts again from the start, so a drag never drifts and returning the
//! pointer returns the volume.

use super::super::frame::SceneFrame;
use super::super::modes::Volume;
use super::super::rotation::mul as mat_mul;
use super::handles::{Gizmo, GizmoMode, Handle, Space};
use super::maths::{Ray, about_vector, closest_on_axis, component, hit_plane, signed_angle, snap, snap_scale, with_component};
use super::placed::PlacedVolume;
use crate::mesh::Vec3;
use crate::v3::{add, cross, dot, len, mul, norm, sub};

/// Step sizes of stepped drags (PJ-23; the modifier held).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Steps {
    /// Moves, in scene units (the scene's largest extent).
    pub distance: f32,
    /// Turns, in degrees.
    pub angle: f32,
    /// Scales, in volume scale units (1 = one scene unit).
    pub scale: f32,
}

impl Default for Steps {
    /// Moves in tenths of the scene, quarter turns (ADR 0008 mc2-q-pt-05c: one gizmo, rotation snapping to
    /// 90°) and scales in tenths.
    fn default() -> Steps {
        Steps { distance: 0.1, angle: 90.0, scale: 0.1 }
    }
}

/// One drag of a handle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeDrag {
    pub handle: Handle,
    start: PlacedVolume,
    frame: SceneFrame,
    clip: bool,
    gizmo: Gizmo,
    /// The camera's viewing direction (object space, unit), for the camera-plane handles.
    view_dir: Vec3,
    /// Where the drag grabbed: a point (planes, trackball) or a parameter along the axis.
    grab: Grab,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Grab {
    Along(f32),
    At(Vec3),
}

impl VolumeDrag {
    /// Starts dragging `handle` of the gizmo of `volume` (in `space`, arms `length` object units long) with the
    /// pointer on `ray`; `view_dir` is the camera's viewing direction. `None` when the handle can't be grabbed
    /// from this view (an axis pointing at the camera, a plane seen edge-on).
    pub fn begin(handle: Handle, space: Space, volume: &Volume, frame: &SceneFrame, ray: Ray, view_dir: Vec3, length: f32) -> Option<VolumeDrag> {
        let start = PlacedVolume::of(volume, frame);
        let mode = match handle {
            Handle::MoveAxis(_) | Handle::MovePlane(_) | Handle::MoveView => GizmoMode::Move,
            Handle::RotateAxis(_) | Handle::RotateFree => GizmoMode::Rotate,
            Handle::ScaleAxis(_) | Handle::ScaleUniform => GizmoMode::Scale,
        };
        let gizmo = Gizmo::new(&start, space, mode, length.max(1e-9));
        let view_dir = norm(view_dir);
        let mut drag = VolumeDrag { handle, start, frame: *frame, clip: volume.clip, gizmo, view_dir, grab: Grab::Along(0.0) };
        drag.grab = drag.grab_of(&ray)?;
        if let (Handle::ScaleAxis(_), Grab::Along(s)) = (handle, drag.grab)
            && s.abs() < 1e-6 * length.max(1e-9)
        {
            return None;
        }
        Some(drag)
    }

    /// The volume with the pointer on `ray`; with `steps`, moved, turned or scaled in whole steps. Where the ray
    /// gives no answer (it runs along the plane being dragged in) the volume stays as it started.
    pub fn update(&self, ray: Ray, steps: Option<&Steps>) -> Volume {
        let placed = self.placed(ray, steps).unwrap_or(self.start);
        placed.to_volume(&self.frame, self.clip)
    }

    fn unit(&self) -> f32 {
        self.frame.largest_extent()
    }

    fn placed(&self, ray: Ray, steps: Option<&Steps>) -> Option<PlacedVolume> {
        let mut p = self.start;
        let c = self.start.centre;
        match (self.handle, self.grab, self.grab_of(&ray)?) {
            (Handle::MoveAxis(k), Grab::Along(s0), Grab::Along(s)) => {
                let d = self.step_distance(s - s0, steps);
                p.centre = add(c, mul(self.gizmo.axis(k), d));
            }
            (Handle::MovePlane(k), Grab::At(p0), Grab::At(p1)) => {
                let d = sub(p1, p0);
                let (u, v) = (self.gizmo.axis((k + 1) % 3), self.gizmo.axis((k + 2) % 3));
                let (du, dv) = (self.step_distance(dot(d, u), steps), self.step_distance(dot(d, v), steps));
                p.centre = add(c, add(mul(u, du), mul(v, dv)));
            }
            (Handle::MoveView, Grab::At(p0), Grab::At(p1)) => {
                let d = sub(p1, p0);
                let d = [0, 1, 2].iter().fold(d, |d, &k| with_component(d, k, self.step_distance(component(d, k), steps)));
                p.centre = add(c, d);
            }
            (Handle::RotateAxis(k), Grab::At(p0), Grab::At(p1)) => {
                let axis = self.gizmo.axis(k);
                let a = signed_angle(sub(p0, c), sub(p1, c), axis);
                let a = match steps {
                    Some(s) => snap(a.to_degrees(), s.angle).to_radians(),
                    None => a,
                };
                p.rotation = mat_mul(&about_vector(axis, a), &self.start.rotation);
            }
            (Handle::RotateFree, Grab::At(p0), Grab::At(p1)) => {
                let (a, b) = (norm(sub(p0, c)), norm(sub(p1, c)));
                let axis = cross(a, b);
                if len(axis) > 1e-9 {
                    let angle = dot(a, b).clamp(-1.0, 1.0).acos();
                    p.rotation = mat_mul(&about_vector(axis, angle), &self.start.rotation);
                }
            }
            (Handle::ScaleAxis(k), Grab::Along(s0), Grab::Along(s)) => {
                let factor = (s / s0).max(1e-3);
                let size = component(self.start.size, k) * factor;
                p.size = with_component(p.size, k, self.step_scale(size, steps));
            }
            (Handle::ScaleUniform, Grab::At(p0), Grab::At(p1)) => {
                let (r0, r1) = (len(sub(p0, c)), len(sub(p1, c)));
                if r0 > 1e-9 {
                    let factor = (r1 / r0).max(1e-3);
                    p.size = self.start.size.map(|s| self.step_scale(s * factor, steps));
                }
            }
            _ => return None,
        }
        Some(p)
    }

    /// What the pointer on `ray` grabs of this drag's handle.
    fn grab_of(&self, ray: &Ray) -> Option<Grab> {
        let c = self.start.centre;
        Some(match self.handle {
            Handle::MoveAxis(k) | Handle::ScaleAxis(k) => {
                // nearly along the view direction the axis can't be dragged sensibly
                let axis = self.gizmo.axis(k);
                if dot(axis, self.view_dir).abs() > 0.995 {
                    return None;
                }
                Grab::Along(closest_on_axis(ray, c, axis)?)
            }
            Handle::MovePlane(k) | Handle::RotateAxis(k) => Grab::At(hit_plane(ray, c, self.gizmo.axis(k))?),
            Handle::MoveView | Handle::ScaleUniform => Grab::At(hit_plane(ray, c, self.view_dir)?),
            Handle::RotateFree => Grab::At(on_sphere(ray, c, self.gizmo.length)),
        })
    }

    /// A move of `d` object units, in whole steps of `steps.distance` scene units.
    fn step_distance(&self, d: f32, steps: Option<&Steps>) -> f32 {
        match steps {
            Some(s) => snap(d / self.unit(), s.distance) * self.unit(),
            None => d,
        }
    }

    /// An edge length `size` (object units) as a scale in whole steps.
    fn step_scale(&self, size: f32, steps: Option<&Steps>) -> f32 {
        match steps {
            Some(s) => snap_scale(size / self.unit(), s.scale) * self.unit(),
            None => size,
        }
    }
}

/// The point of the sphere of `radius` around `centre` the ray meets first, or where the ray passes it nearest to
/// the sphere's outline (a trackball that keeps turning beyond its edge).
fn on_sphere(ray: &Ray, centre: Vec3, radius: f32) -> Vec3 {
    let w = sub(ray.origin, centre);
    let b = dot(w, ray.dir);
    let c = dot(w, w) - radius * radius;
    let disc = b * b - c;
    if disc >= 0.0 {
        return ray.at(-b - disc.sqrt());
    }
    // nearest point of the ray to the centre, pushed out onto the sphere
    let nearest = ray.at(-b);
    add(centre, mul(norm(sub(nearest, centre)), radius))
}
