//! glTF cameras: write → read round trips and cameras from files written by other programs.

use super::tests::sample;
use super::*;
use crate::camera::{SceneCamera, SceneLens};
use crate::v3::{cross, norm, sub};

/// A camera at `eye` looking at `target`, turned by `roll`.
fn camera(name: &str, eye: [f32; 3], target: [f32; 3], roll: f32, lens: SceneLens, aspect: Option<f32>) -> SceneCamera {
    let f = norm(sub(target, eye));
    let side = if f[0].abs() + f[2].abs() < 1e-6 { [1.0, 0.0, 0.0] } else { norm(cross(f, [0.0, 1.0, 0.0])) };
    let level_up = cross(side, f);
    let (s, c) = roll.sin_cos();
    let up = norm(std::array::from_fn(|i| level_up[i] * c - side[i] * s));
    SceneCamera { name: name.into(), eye, forward: f, up, lens, aspect }
}

fn cameras() -> Vec<SceneCamera> {
    vec![
        camera("Hero", [3.0, 2.0, 4.0], [0.0, 0.5, 0.0], 0.0, SceneLens::Perspective { fov_y: 0.7 }, Some(16.0 / 9.0)),
        camera("Dutch", [-2.0, 0.5, -3.0], [0.2, 0.0, 0.1], 0.3, SceneLens::Perspective { fov_y: 1.1 }, None),
        camera("Top", [0.0, 6.0, 0.0], [0.0, 0.0, 0.0], 0.0, SceneLens::Orthographic { height: 3.5 }, Some(1.25)),
        // glTF's orthographic cameras always have a width: a camera without an aspect is written square
        camera("Under", [0.5, -4.0, 0.5], [0.5, 0.0, 0.5], 0.0, SceneLens::Orthographic { height: 2.0 }, Some(1.0)),
    ]
}

#[test]
fn cameras_round_trip_next_to_the_mesh() {
    let mesh = sample();
    let glb = mesh.to_glb_with_cameras(&cameras()).unwrap();
    assert_eq!(glb, mesh.to_glb_with_cameras(&cameras()).unwrap(), "deterministic");
    assert_eq!(Mesh::from_glb(&glb).unwrap(), mesh, "the mesh reader skips camera nodes");
    let back = read_cameras(&glb).unwrap();
    assert_eq!(back.len(), 4);
    for (a, b) in back.iter().zip(cameras()) {
        assert!(a.close_to(&b, 1e-5), "{a:?}\n{b:?}");
    }
    // a file without cameras has none, and an empty mesh can still carry them
    assert!(read_cameras(&mesh.to_glb().unwrap()).unwrap().is_empty());
    let alone = Mesh::default().to_glb_with_cameras(&cameras()[..1]).unwrap();
    assert!(read_cameras(&alone).unwrap()[0].close_to(&cameras()[0], 1e-5));
    assert_eq!(Mesh::from_glb(&alone).unwrap(), Mesh::default());
}

#[test]
fn cameras_written_by_other_programs_follow_their_node_transforms() {
    // A rig: a scaled, turned and moved parent holding a camera node that is moved again; a second camera
    // outside the default scene is ignored, a third has no names at all.
    let json = r#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0,3]},{"nodes":[2]}],
        "cameras":[{"name":"lens","type":"perspective","perspective":{"yfov":0.5,"znear":0.1}},
                   {"type":"orthographic","orthographic":{"xmag":3,"ymag":2,"znear":0.1,"zfar":10}}],
        "nodes":[{"name":"rig","translation":[0,1,0],"rotation":[0,0.7071067811865476,0,0.7071067811865476],"scale":[2,2,2],"children":[1]},
                 {"name":"Shot","camera":0,"translation":[0,0,5]},
                 {"name":"Elsewhere","camera":1},
                 {"camera":1,"matrix":[1,0,0,0, 0,0,-1,0, 0,1,0,0, 0,7,0,1]}]}"#;
    let glb = glb::Glb::assemble(json, &[]).unwrap();
    let cams = read_cameras(&glb).unwrap();
    assert_eq!(cams.len(), 2);
    let shot = &cams[0];
    // the parent turns +z into +x and doubles distances: the camera sits at (10, 1, 0) looking down −x
    assert_eq!(shot.name, "Shot");
    assert!(sub(shot.eye, [10.0, 1.0, 0.0]).iter().all(|v| v.abs() < 1e-5), "{:?}", shot.eye);
    assert!(sub(shot.forward, [-1.0, 0.0, 0.0]).iter().all(|v| v.abs() < 1e-6) && (shot.up[1] - 1.0).abs() < 1e-6);
    assert_eq!((shot.lens, shot.aspect), (SceneLens::Perspective { fov_y: 0.5 }, None));
    let top = &cams[1];
    assert_eq!(top.name, "Camera 2");
    assert!(sub(top.eye, [0.0, 7.0, 0.0]).iter().all(|v| v.abs() < 1e-6));
    assert!(sub(top.forward, [0.0, -1.0, 0.0]).iter().all(|v| v.abs() < 1e-6), "{:?}", top.forward);
    assert_eq!((top.lens, top.aspect), (SceneLens::Orthographic { height: 4.0 }, Some(1.5)));
}

#[test]
fn broken_cameras_are_errors() {
    let file = |cameras: &str, node: &str| {
        let json = format!(r#"{{"asset":{{"version":"2.0"}},"scenes":[{{"nodes":[0]}}],"cameras":[{cameras}],"nodes":[{node}]}}"#);
        read_cameras(&glb::Glb::assemble(&json, &[]).unwrap())
    };
    let ok = r#"{"type":"perspective","perspective":{"yfov":0.5,"znear":0.1}}"#;
    assert!(file(ok, r#"{"camera":0}"#).is_ok());
    assert!(matches!(file(ok, r#"{"camera":3}"#), Err(GltfError::Invalid { .. })));
    assert!(matches!(file(r#"{"type":"fisheye"}"#, r#"{"camera":0}"#), Err(GltfError::Unsupported(_))));
    assert!(matches!(file(r#"{"type":"perspective","perspective":{"yfov":-1}}"#, r#"{"camera":0}"#), Err(GltfError::Invalid { .. })));
    assert!(matches!(file(ok, r#"{"camera":0,"scale":[0,0,0]}"#), Err(GltfError::Invalid { .. })));
    assert!(matches!(file(ok, r#"{"camera":0,"children":[0]}"#), Err(GltfError::Invalid { .. })));
}
