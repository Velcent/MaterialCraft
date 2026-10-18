//! glTF cameras (requirement CM-04) ↔ [`SceneCamera`]s.
//!
//! Reading: every node of the default scene that references a camera, with the node's world transform: the eye is
//! the transformed origin, the viewing direction the transformed −z axis, the up direction the transformed +y axis
//! (scale and shear are ignored, the up direction is made perpendicular). `perspective` cameras give `yfov` and the
//! optional `aspectRatio`, `orthographic` ones a height of `2 · ymag` and the aspect `xmag / ymag`. The name is the
//! node's, else the camera's, else `Camera <n>`. Clipping planes are not kept (the viewport has its own).
//!
//! Writing: one glTF camera and one node (translation and rotation) per camera, appended after the mesh's nodes
//! and listed in the scene; the mesh reader skips them. Our cameras are glTF's (looking down −z, +y up), so a
//! write → read round trip returns the same cameras up to float rounding; only an orthographic camera without an
//! aspect comes back square (glTF requires its width).

use super::error::GltfError;
use super::read::{indices, list, scene_roots};
use super::transform::Transform;
use crate::camera::{SceneCamera, SceneLens};
use crate::json::Value;

/// Deepest node hierarchy followed (also stops cycles).
const MAX_NODE_DEPTH: usize = 64;
/// Clipping planes written for cameras (glTF requires `znear`; the viewport ignores both).
const ZNEAR: f32 = 0.01;
const ZFAR: f32 = 1000.0;

/// The cameras placed in the default scene of a parsed glTF document, in node order.
pub(super) fn read(root: &Value) -> Result<Vec<SceneCamera>, GltfError> {
    let mut out = Vec::new();
    let mut visited = vec![false; list(root, "nodes").len()];
    for node in scene_roots(root)? {
        visit(root, node, &Transform::IDENTITY, 0, &mut visited, &mut out)?;
    }
    Ok(out)
}

fn visit(root: &Value, index: usize, parent: &Transform, depth: usize, visited: &mut [bool], out: &mut Vec<SceneCamera>) -> Result<(), GltfError> {
    let at = format!("nodes[{index}]");
    match visited.get_mut(index) {
        _ if depth > MAX_NODE_DEPTH => return Err(GltfError::invalid(at, "node hierarchy too deep")),
        Some(seen) if *seen => return Err(GltfError::invalid(at, "node has several parents or is cyclic")),
        Some(seen) => *seen = true,
        None => return Err(GltfError::invalid(at, "does not exist")),
    }
    let node = list(root, "nodes").get(index).ok_or_else(|| GltfError::invalid(at.clone(), "does not exist"))?;
    let world = parent.then_local(&Transform::of_node(node, &at)?);
    if let Some(camera) = node.get("camera") {
        let i = camera.as_usize().ok_or_else(|| GltfError::invalid(format!("{at}.camera"), "not an index"))?;
        let name = node.get("name").and_then(Value::as_str);
        out.push(camera_at(root, i, name, &world, out.len())?);
    }
    for child in indices(node, "children", &format!("{at}.children"))? {
        visit(root, child, &world, depth + 1, visited, out)?;
    }
    Ok(())
}

/// Camera `index` placed by `world`.
fn camera_at(root: &Value, index: usize, node_name: Option<&str>, world: &Transform, n: usize) -> Result<SceneCamera, GltfError> {
    let at = format!("cameras[{index}]");
    let camera = list(root, "cameras").get(index).ok_or_else(|| GltfError::invalid(at.clone(), "does not exist"))?;
    let number = |o: &Value, key: &str| o.get(key).and_then(Value::as_f64).map(|v| v as f32).filter(|v| v.is_finite());
    let positive = |o: &Value, key: &str| number(o, key).filter(|v| *v > 0.0);
    let (lens, aspect) = match camera.get("type").and_then(Value::as_str) {
        Some("perspective") => {
            let p = camera.get("perspective").ok_or_else(|| GltfError::invalid(at.clone(), "perspective camera without `perspective`"))?;
            let fov_y = positive(p, "yfov").ok_or_else(|| GltfError::invalid(format!("{at}.perspective.yfov"), "expected a positive number"))?;
            (SceneLens::Perspective { fov_y }, positive(p, "aspectRatio"))
        }
        Some("orthographic") => {
            let o = camera.get("orthographic").ok_or_else(|| GltfError::invalid(at.clone(), "orthographic camera without `orthographic`"))?;
            let ymag = positive(o, "ymag").ok_or_else(|| GltfError::invalid(format!("{at}.orthographic.ymag"), "expected a positive number"))?;
            (SceneLens::Orthographic { height: 2.0 * ymag }, positive(o, "xmag").map(|x| x / ymag))
        }
        other => return Err(GltfError::Unsupported(format!("{at}: camera type {other:?}"))),
    };
    let name = node_name.or_else(|| camera.get("name").and_then(Value::as_str)).map_or_else(|| format!("Camera {}", n + 1), str::to_string);
    let (eye, forward, up) = world.camera_axes().ok_or_else(|| GltfError::invalid(at, "the camera's node transform is degenerate"))?;
    Ok(SceneCamera { name, eye, forward, up, lens, aspect })
}

/// Adds `cameras` to a written document: a `cameras` array and one node per camera, listed in scene 0.
pub(super) fn write(root: &mut Value, cameras: &[SceneCamera]) {
    if cameras.is_empty() {
        return;
    }
    let first = root.get("nodes").and_then(Value::as_array).map_or(0, Vec::len);
    let mut glb_cameras = Vec::with_capacity(cameras.len());
    let mut nodes = Vec::with_capacity(cameras.len());
    for (i, c) in cameras.iter().enumerate() {
        let mut camera = Value::object();
        camera = match c.lens {
            SceneLens::Perspective { fov_y } => {
                let mut p = Value::object().with("yfov", fov_y as f64).with("znear", ZNEAR as f64).with("zfar", ZFAR as f64);
                if let Some(a) = c.aspect {
                    p.insert("aspectRatio", a as f64);
                }
                camera.with("type", "perspective").with("perspective", p)
            }
            SceneLens::Orthographic { height } => {
                let ymag = 0.5 * height;
                let o = Value::object()
                    .with("xmag", (ymag * c.aspect.unwrap_or(1.0)) as f64)
                    .with("ymag", ymag as f64)
                    .with("znear", ZNEAR as f64)
                    .with("zfar", ZFAR as f64);
                camera.with("type", "orthographic").with("orthographic", o)
            }
        };
        glb_cameras.push(camera.with("name", c.name.as_str()));
        let translation: Value = c.eye.iter().map(|v| *v as f64).collect();
        let rotation: Value = c.rotation().iter().map(|v| *v as f64).collect();
        nodes.push(Value::object().with("name", c.name.as_str()).with("camera", i).with("translation", translation).with("rotation", rotation));
    }
    let node_ids: Vec<usize> = (first..first + nodes.len()).collect();
    match root.get_mut("nodes").and_then(Value::as_array_mut) {
        Some(existing) => existing.extend(nodes),
        None => {
            root.insert("nodes", Value::Array(nodes));
        }
    }
    root.insert("cameras", Value::Array(glb_cameras));
    if let Some(scene) = root.get_mut("scenes").and_then(Value::as_array_mut).and_then(|s| s.first_mut()) {
        match scene.get_mut("nodes").and_then(Value::as_array_mut) {
            Some(listed) => listed.extend(node_ids.into_iter().map(Value::from)),
            None => {
                scene.insert("nodes", node_ids.into_iter().collect::<Value>());
            }
        }
    }
}
