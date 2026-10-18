//! Cameras of an imported scene → cameras of the document's camera list (CM-02, CM-04): each [`SceneCamera`], given
//! in the coordinates of the document's mesh, is moved into the viewport's fit of that mesh ([`Mesh::fit`], the
//! space every saved camera lives in) and becomes an imported [`SavedCamera`] that keeps its values as the original.

use super::saved_camera::{CameraValues, SavedCamera};
use crate::camera::SceneCamera;
use crate::mesh::{Fit, Mesh};

/// `camera` (in `mesh`'s coordinates) as the values a saved camera holds; `None` for a degenerate pose.
pub fn camera_values(mesh: &Mesh, camera: &SceneCamera) -> Option<CameraValues> {
    let fit = mesh.fit();
    CameraValues::of_scene(&placed(&fit, camera))
}

/// Every usable camera of `cameras` (in `mesh`'s coordinates) as an imported camera from `source` (one of
/// [`super::saved_camera::CameraOrigin::SOURCES`]). Cameras with a degenerate pose are left out.
pub fn imported_cameras(mesh: &Mesh, cameras: &[SceneCamera], source: &str) -> Vec<SavedCamera> {
    let fit = mesh.fit();
    cameras.iter().filter_map(|c| Some(SavedCamera::imported(&c.name, source, CameraValues::of_scene(&placed(&fit, c))?))).collect()
}

fn placed(fit: &Fit, camera: &SceneCamera) -> SceneCamera {
    camera.placed(fit.translate, fit.scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{SceneLens, look_at, transform_point};
    use crate::mesh::MeshPart;

    /// A box from (10, 0, 0) to (14, 2, 2): its fit moves (12, 1, 1) to the origin and scales by 1 / √6.
    fn mesh() -> Mesh {
        let positions = vec![[10.0, 0.0, 0.0], [14.0, 2.0, 2.0], [10.0, 2.0, 0.0]];
        Mesh { materials: vec!["m".into()], parts: vec![MeshPart { positions, triangles: vec![[0, 1, 2]], ..MeshPart::default() }] }
    }

    #[test]
    fn imported_cameras_see_the_mesh_as_the_scene_camera_did() {
        let mesh = mesh();
        let eye = [12.0, 3.0, 9.0];
        let target = [11.0, 1.5, 1.0];
        let f = crate::v3::norm(crate::v3::sub(target, eye));
        let right = crate::v3::norm(crate::v3::cross(f, [0.0, 1.0, 0.0]));
        let up = crate::v3::cross(right, f);
        let cam = SceneCamera { name: "Shot".into(), eye, forward: f, up, lens: SceneLens::Perspective { fov_y: 0.6 }, aspect: Some(2.0) };
        let degenerate = SceneCamera { forward: [0.0; 3], ..cam.clone() };
        let saved = imported_cameras(&mesh, &[cam.clone(), degenerate], "gltf");
        assert_eq!(saved.len(), 1);
        let s = &saved[0];
        assert!(!s.is_modified() && s.name == "Shot" && s.aspect == Some(2.0));
        // the scene camera's view of a mesh point equals the saved camera's view of the fitted point
        let fit = mesh.fit();
        let ours = s.settings.camera(&s.orbit, [800.0, 400.0]);
        let theirs = look_at(eye, target, [0.0, 1.0, 0.0]);
        for p in mesh.parts[0].positions.iter().chain([&target]) {
            let a = ours.project(fit.apply(*p)).unwrap();
            let v = transform_point(&theirs, *p);
            let t = (0.5f32 * 0.6).tan();
            let expected = [400.0 + 400.0 * v[0] / (-v[2] * t * 2.0), 200.0 - 200.0 * v[1] / (-v[2] * t)];
            assert!((a.screen[0] - expected[0]).abs() < 0.02 && (a.screen[1] - expected[1]).abs() < 0.02, "{p:?}: {a:?} vs {expected:?}");
        }
        assert_eq!(camera_values(&mesh, &cam), Some(s.values()));
    }
}
