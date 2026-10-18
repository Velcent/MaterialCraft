//! [`CpuViewProjector`]: [`ViewProjector`] by ray casts through the mesh's per-part BVHs.

use super::scene::ViewScene;
use super::{ScreenPoint, SurfaceHit, ViewProjector};
use crate::camera::{Camera, CameraTransform};
use crate::mesh::{Hit, Mesh, MeshPart, Triangles, Vec3};
use crate::v3::{add, cross, dot, len, mul, norm, sub};

/// Screen ↔ mesh for one camera on the CPU. Cheap to make (the BVHs live in the [`ViewScene`]); make one per
/// frame.
pub struct CpuViewProjector<'a> {
    scene: &'a ViewScene,
    mesh: &'a Mesh,
    camera: Camera,
    transform: CameraTransform,
}

impl<'a> CpuViewProjector<'a> {
    pub(super) fn new(scene: &'a ViewScene, mesh: &'a Mesh, camera: Camera) -> CpuViewProjector<'a> {
        CpuViewProjector { scene, mesh, camera, transform: camera.transform() }
    }

    /// The scene this projector picks in (which parts are shown, the fit).
    pub fn scene(&self) -> &ViewScene {
        self.scene
    }

    /// The nearest hit along an object-space ray with `tmin < t < tmax` (object units), over the shown parts.
    fn cast(&self, origin: Vec3, dir: Vec3, tmax: f32) -> Option<(usize, Hit)> {
        let mut best: Option<(usize, Hit)> = None;
        let mut limit = tmax;
        for (i, (scene, part)) in self.scene.parts.iter().zip(&self.mesh.parts).enumerate() {
            if !scene.shown {
                continue;
            }
            if let Some(hit) = scene.bvh.intersect(part, origin, dir, limit) {
                limit = hit.t;
                best = Some((i, hit));
            }
        }
        best
    }

    fn to_world(&self, p: Vec3) -> Vec3 {
        self.scene.fit.apply(p)
    }

    fn to_object(&self, p: Vec3) -> Vec3 {
        self.scene.fit.invert(p)
    }

    /// The surface hit of `hit` on `part`.
    fn surface(&self, part_index: usize, hit: Hit, scale: f32) -> Option<SurfaceHit> {
        let part = self.mesh.parts.get(part_index)?;
        let uv_set = self.scene.parts.get(part_index).map_or(0, |s| s.uv_set);
        let tri = *part.triangles.get(hit.tri as usize)?;
        let w = [1.0 - hit.u - hit.v, hit.u, hit.v];
        let corners = part.corners(hit.tri as usize);
        let position = interpolate(&corners, w);
        let face = norm(cross(sub(corners[1], corners[0]), sub(corners[2], corners[0])));
        let normal = vertex_normal(part, tri, w).unwrap_or(face);
        let uv = part.uv_sets.get(uv_set).and_then(|uvs| interpolate_uv(uvs, tri, w)).unwrap_or([0.0; 2]);
        Some(SurfaceHit { part: part_index, triangle: hit.tri, barycentric: w, uv, position, normal, distance: hit.t * scale })
    }
}

impl ViewProjector for CpuViewProjector<'_> {
    fn camera(&self) -> &Camera {
        &self.camera
    }

    fn pick(&self, screen: [f32; 2]) -> Option<SurfaceHit> {
        let ray = self.transform.ray(screen)?;
        let scale = self.scene.fit.scale;
        if !(scale.is_finite() && scale > 0.0) {
            return None;
        }
        // the fit scales uniformly, so the direction stays and object distances are world distances / scale
        let origin = self.to_object(ray.origin);
        let (part, hit) = self.cast(origin, ray.dir, ray.length / scale)?;
        self.surface(part, hit, scale)
    }

    fn to_screen(&self, pos: Vec3) -> Option<ScreenPoint> {
        let p = self.transform.project(self.to_world(pos))?;
        Some(ScreenPoint { pos: p.screen, depth: p.depth, view_depth: p.view_depth })
    }

    fn ray(&self, screen: [f32; 2]) -> Option<crate::projection::gizmo::Ray> {
        let ray = self.transform.ray(screen)?;
        // the fit moves and scales uniformly: the direction stays
        crate::projection::gizmo::Ray::new(self.to_object(ray.origin), ray.dir)
    }

    fn occluded(&self, pos: Vec3) -> bool {
        let world = self.to_world(pos);
        let dir = self.transform.towards_camera(world);
        let scale = self.scene.fit.scale;
        // up to the eye (perspective) or without end (orthographic), in object units
        let reach = match (self.transform.is_orthographic(), self.transform.eye()) {
            (false, Some(eye)) => len(sub(eye, world)) / scale,
            _ => f32::MAX,
        };
        let eps = self.scene.epsilon;
        if reach.is_nan() || reach <= 2.0 * eps {
            return false;
        }
        let origin = add(pos, mul(dir, eps));
        self.cast(origin, dir, reach - eps).is_some()
    }

    fn facing(&self, pos: Vec3, normal: Vec3) -> f32 {
        // the fit only moves and scales uniformly: normals keep their direction
        dot(norm(normal), self.transform.towards_camera(self.to_world(pos)))
    }
}

fn interpolate(c: &[Vec3; 3], w: [f32; 3]) -> Vec3 {
    add(add(mul(c[0], w[0]), mul(c[1], w[1])), mul(c[2], w[2]))
}

fn vertex_normal(part: &MeshPart, tri: [u32; 3], w: [f32; 3]) -> Option<Vec3> {
    let n = tri.map(|i| part.normals.get(i as usize).copied());
    let [Some(a), Some(b), Some(c)] = n else { return None };
    let s = interpolate(&[a, b, c], w);
    (len(s) > 1e-12 && s.iter().all(|v| v.is_finite())).then(|| norm(s))
}

fn interpolate_uv(uvs: &[[f32; 2]], tri: [u32; 3], w: [f32; 3]) -> Option<[f32; 2]> {
    let [a, b, c] = tri.map(|i| uvs.get(i as usize).copied());
    let (a, b, c) = (a?, b?, c?);
    Some([a[0] * w[0] + b[0] * w[1] + c[0] * w[2], a[1] * w[0] + b[1] * w[1] + c[1] * w[2]])
}
