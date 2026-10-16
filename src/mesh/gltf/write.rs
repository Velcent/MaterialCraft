//! Mesh → glTF 2.0: one node and one glTF mesh (with one primitive) per part, one material per
//! slot, all vertex data in a single binary buffer.

use super::error::GltfError;
use super::{EXTRAS_KEY, GENERATOR};
use crate::json::Value;
use crate::mesh::{Mesh, MeshPart};

const FLOAT: u32 = 5126;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const MODE_POINTS: u32 = 0;

/// The JSON document and binary buffer of a mesh.
pub(super) fn to_gltf(mesh: &Mesh) -> Result<(Value, Vec<u8>), GltfError> {
    mesh.validate()?;
    let mut buf = BufferBuilder::default();
    let mut nodes = Vec::new();
    let mut meshes = Vec::new();
    for part in &mesh.parts {
        let mut node = Value::object().with("name", part.name.as_str());
        if part.positions.is_empty() {
            // glTF has no empty primitives: keep the part as a marked node without a mesh.
            let extras = Value::object().with("empty_part", true);
            let extras = extras.with("material", part.material);
            node.insert("extras", Value::object().with(EXTRAS_KEY, extras));
        } else {
            node.insert("mesh", meshes.len());
            meshes.push(
                Value::object()
                    .with("name", part.name.as_str())
                    .with("primitives", vec![primitive(part, &mut buf)]),
            );
        }
        nodes.push(node);
    }
    let mut root = Value::object().with("asset", asset()).with("scene", 0);
    let mut scene = Value::object();
    if !nodes.is_empty() {
        scene.insert("nodes", (0..nodes.len()).collect::<Value>());
    }
    root.insert("scenes", vec![scene]);
    let materials: Value = mesh
        .materials
        .iter()
        .map(|m| Value::object().with("name", m.as_str()))
        .collect();
    for (key, list) in [
        ("nodes", Value::Array(nodes)),
        ("meshes", Value::Array(meshes)),
        ("materials", materials),
        ("accessors", Value::Array(buf.accessors)),
        ("bufferViews", Value::Array(buf.views)),
    ] {
        if list.as_array().is_some_and(|a| !a.is_empty()) {
            root.insert(key, list);
        }
    }
    if !buf.bin.is_empty() {
        root.insert(
            "buffers",
            vec![Value::object().with("byteLength", buf.bin.len())],
        );
    }
    Ok((root, buf.bin))
}

fn asset() -> Value {
    Value::object()
        .with("version", "2.0")
        .with("generator", GENERATOR)
}

fn primitive(part: &MeshPart, buf: &mut BufferBuilder) -> Value {
    let mut attributes = Value::object();
    let position = buf.floats(&flatten(&part.positions), 3, true);
    attributes.insert("POSITION", position);
    if !part.normals.is_empty() {
        attributes.insert("NORMAL", buf.floats(&flatten(&part.normals), 3, false));
    }
    if !part.tangents.is_empty() {
        attributes.insert("TANGENT", buf.floats(&flatten(&part.tangents), 4, false));
    }
    for (i, uv) in part.uv_sets.iter().enumerate() {
        attributes.insert(format!("TEXCOORD_{i}"), buf.floats(&flatten(uv), 2, false));
    }
    let mut p = Value::object().with("attributes", attributes);
    if part.triangles.is_empty() {
        p.insert("mode", MODE_POINTS);
    } else {
        p.insert("indices", buf.indices(&part.triangles));
    }
    p.insert("material", part.material);
    p
}

fn flatten<const N: usize>(v: &[[f32; N]]) -> Vec<f32> {
    v.iter().flatten().copied().collect()
}

/// Collects buffer views and accessors over one growing binary buffer.
#[derive(Default)]
struct BufferBuilder {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl BufferBuilder {
    /// Adds a tightly packed view; returns its index. Views start on 4-byte boundaries.
    fn view(&mut self, bytes: impl Iterator<Item = [u8; 4]>, target: u32) -> usize {
        self.bin.resize(self.bin.len().next_multiple_of(4), 0);
        let start = self.bin.len();
        bytes.for_each(|b| self.bin.extend_from_slice(&b));
        self.views.push(
            Value::object()
                .with("buffer", 0)
                .with("byteOffset", start)
                .with("byteLength", self.bin.len() - start)
                .with("target", target),
        );
        self.views.len() - 1
    }

    /// Float accessor of `n`-component vectors (`n` = 2, 3 or 4); returns its index.
    fn floats(&mut self, data: &[f32], n: usize, with_bounds: bool) -> usize {
        let view = self.view(data.iter().map(|f| f.to_le_bytes()), ARRAY_BUFFER);
        let kind = ["", "SCALAR", "VEC2", "VEC3", "VEC4"][n];
        let mut a = Value::object()
            .with("bufferView", view)
            .with("componentType", FLOAT)
            .with("count", data.len() / n)
            .with("type", kind);
        if with_bounds {
            let mut lo = vec![f32::INFINITY; n];
            let mut hi = vec![f32::NEG_INFINITY; n];
            for v in data.chunks_exact(n) {
                for c in 0..n {
                    lo[c] = lo[c].min(v[c]);
                    hi[c] = hi[c].max(v[c]);
                }
            }
            a.insert("min", lo);
            a.insert("max", hi);
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }

    fn indices(&mut self, triangles: &[[u32; 3]]) -> usize {
        let view = self.view(
            triangles.iter().flatten().map(|i| i.to_le_bytes()),
            ELEMENT_ARRAY_BUFFER,
        );
        self.accessors.push(
            Value::object()
                .with("bufferView", view)
                .with("componentType", UNSIGNED_INT)
                .with("count", triangles.len() * 3)
                .with("type", "SCALAR"),
        );
        self.accessors.len() - 1
    }
}
