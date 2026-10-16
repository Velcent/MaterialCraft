//! glTF 2.0 → Mesh: walks the default scene, turning every triangle primitive into a part with
//! node transforms applied, and every glTF material into a material slot.

use super::accessor::Accessors;
use super::error::GltfError;
use super::transform::Transform;
use super::EXTRAS_KEY;
use crate::json::Value;
use crate::mesh::{Mesh, MeshPart};

/// Deepest node hierarchy followed (also stops cycles).
const MAX_NODE_DEPTH: usize = 64;

const POINTS: u64 = 0;
const TRIANGLES: u64 = 4;
const TRIANGLE_STRIP: u64 = 5;
const TRIANGLE_FAN: u64 = 6;

/// Builds a [`Mesh`] from a parsed glTF document and its binary chunk.
pub(super) struct GltfReader<'a> {
    root: &'a Value,
    accessors: Accessors<'a>,
    mesh: Mesh,
    /// Slot for primitives without a material, created on first use.
    default_slot: Option<usize>,
    /// Nodes already visited: glTF nodes form trees, so a second visit means a broken hierarchy.
    visited: Vec<bool>,
}

impl<'a> GltfReader<'a> {
    pub fn new(root: &'a Value, bin: &'a [u8]) -> Result<GltfReader<'a>, GltfError> {
        check_asset(root)?;
        let materials = list(root, "materials")
            .iter()
            .enumerate()
            .map(|(i, m)| {
                m.get("name")
                    .and_then(Value::as_str)
                    .map_or_else(|| format!("material{i}"), str::to_string)
            })
            .collect();
        Ok(GltfReader {
            root,
            accessors: Accessors::new(root, bin)?,
            mesh: Mesh {
                materials,
                parts: Vec::new(),
            },
            default_slot: None,
            visited: vec![false; list(root, "nodes").len()],
        })
    }

    pub fn read(mut self) -> Result<Mesh, GltfError> {
        for node in self.scene_roots()? {
            self.visit(node, &Transform::IDENTITY, 0)?;
        }
        self.mesh.validate()?;
        Ok(self.mesh)
    }

    /// Root nodes of the default scene; without scenes, every node that is no other node's child.
    fn scene_roots(&self) -> Result<Vec<usize>, GltfError> {
        let scenes = list(self.root, "scenes");
        if scenes.is_empty() {
            let children: Vec<usize> = list(self.root, "nodes")
                .iter()
                .flat_map(|n| list(n, "children"))
                .filter_map(Value::as_usize)
                .collect();
            return Ok((0..list(self.root, "nodes").len())
                .filter(|i| !children.contains(i))
                .collect());
        }
        let index = self.root.get("scene").map_or(Some(0), Value::as_usize);
        let scene = index
            .and_then(|i| scenes.get(i))
            .ok_or_else(|| GltfError::invalid("scene", "does not exist"))?;
        indices(scene, "nodes", "scenes[].nodes")
    }

    fn visit(&mut self, index: usize, parent: &Transform, depth: usize) -> Result<(), GltfError> {
        let at = format!("nodes[{index}]");
        if depth > MAX_NODE_DEPTH {
            return Err(GltfError::invalid(at, "node hierarchy too deep"));
        }
        match self.visited.get_mut(index) {
            Some(seen) if *seen => {
                return Err(GltfError::invalid(
                    at,
                    "node has several parents or is cyclic",
                ))
            }
            Some(seen) => *seen = true,
            None => {}
        }
        let node = list(self.root, "nodes")
            .get(index)
            .ok_or_else(|| GltfError::invalid(at.clone(), "does not exist"))?;
        let world = parent.then_local(&Transform::of_node(node, &at)?);
        let name = node.get("name").and_then(Value::as_str);
        if let Some(mesh) = node.get("mesh") {
            let mesh = mesh
                .as_usize()
                .ok_or_else(|| GltfError::invalid(format!("{at}.mesh"), "not an index"))?;
            self.add_mesh(mesh, name, &world)?;
        } else if let Some(extras) = node.get("extras").and_then(|e| e.get(EXTRAS_KEY)) {
            if extras.get("empty_part").and_then(Value::as_bool) == Some(true) {
                let material = extras.get("material").and_then(Value::as_usize);
                let material = material
                    .ok_or_else(|| GltfError::invalid(format!("{at}.extras"), "bad material"))?;
                self.mesh.parts.push(MeshPart {
                    name: name.unwrap_or_default().to_string(),
                    material,
                    ..MeshPart::default()
                });
            }
        }
        for child in indices(node, "children", &format!("{at}.children"))? {
            self.visit(child, &world, depth + 1)?;
        }
        Ok(())
    }

    fn add_mesh(
        &mut self,
        index: usize,
        node_name: Option<&str>,
        world: &Transform,
    ) -> Result<(), GltfError> {
        let at = format!("meshes[{index}]");
        let mesh = list(self.root, "meshes")
            .get(index)
            .ok_or_else(|| GltfError::invalid(at.clone(), "does not exist"))?;
        let base = node_name
            .or_else(|| mesh.get("name").and_then(Value::as_str))
            .map_or_else(|| format!("part{}", self.mesh.parts.len()), str::to_string);
        let primitives = list(mesh, "primitives");
        for (p, prim) in primitives.iter().enumerate() {
            let mut part = self.primitive(prim, &format!("{at}.primitives[{p}]"))?;
            part.name = if primitives.len() > 1 {
                format!("{base}.{p}")
            } else {
                base.clone()
            };
            world.apply(&mut part);
            self.mesh.parts.push(part);
        }
        Ok(())
    }

    fn primitive(&mut self, prim: &Value, at: &str) -> Result<MeshPart, GltfError> {
        let attrs = prim.get("attributes");
        let attr = |name: &str| attrs.and_then(|a| a.get(name)).map(|v| v.as_usize());
        let req = |v: Option<usize>, name: &str| {
            v.ok_or_else(|| GltfError::invalid(format!("{at}.attributes.{name}"), "not an index"))
        };
        let position =
            attr("POSITION").ok_or_else(|| GltfError::invalid(at, "has no POSITION attribute"))?;
        let mut part = MeshPart {
            positions: self.accessors.vectors(req(position, "POSITION")?)?,
            ..MeshPart::default()
        };
        if let Some(n) = attr("NORMAL") {
            part.normals = self.accessors.vectors(req(n, "NORMAL")?)?;
        }
        if let Some(t) = attr("TANGENT") {
            part.tangents = self.accessors.vectors(req(t, "TANGENT")?)?;
        }
        while let Some(uv) = attr(&format!("TEXCOORD_{}", part.uv_sets.len())) {
            let name = format!("TEXCOORD_{}", part.uv_sets.len());
            part.uv_sets.push(self.accessors.vectors(req(uv, &name)?)?);
        }
        let mode = prim.get("mode").map_or(Some(TRIANGLES), Value::as_u64);
        let index_list = match prim.get("indices") {
            None => (0..part.positions.len() as u32).collect(),
            Some(i) => {
                let i = i
                    .as_usize()
                    .ok_or_else(|| GltfError::invalid(format!("{at}.indices"), "not an index"))?;
                self.accessors.indices(i)?
            }
        };
        part.triangles = match mode {
            Some(POINTS) => Vec::new(),
            Some(TRIANGLES) => index_list
                .chunks_exact(3)
                .map(|t| [t[0], t[1], t[2]])
                .collect(),
            Some(TRIANGLE_STRIP) => (2..index_list.len())
                .map(|i| {
                    let (a, b, c) = (index_list[i - 2], index_list[i - 1], index_list[i]);
                    if i % 2 == 0 {
                        [a, b, c]
                    } else {
                        [b, a, c]
                    }
                })
                .collect(),
            Some(TRIANGLE_FAN) => (2..index_list.len())
                .map(|i| [index_list[i - 1], index_list[i], index_list[0]])
                .collect(),
            _ => {
                return Err(GltfError::Unsupported(format!(
                    "{at}: only triangle primitives are read (mode {})",
                    mode.map_or("?".into(), |m| m.to_string())
                )))
            }
        };
        part.material = match prim.get("material") {
            Some(m) => m
                .as_usize()
                .filter(|&m| m < self.mesh.materials.len())
                .ok_or_else(|| GltfError::invalid(format!("{at}.material"), "does not exist"))?,
            None => self.default_slot(),
        };
        Ok(part)
    }

    fn default_slot(&mut self) -> usize {
        *self.default_slot.get_or_insert_with(|| {
            self.mesh.materials.push("default".to_string());
            self.mesh.materials.len() - 1
        })
    }
}

/// Rejects other glTF versions and required extensions.
fn check_asset(root: &Value) -> Result<(), GltfError> {
    let version = root
        .get("asset")
        .and_then(|a| a.get("version"))
        .and_then(Value::as_str)
        .ok_or_else(|| GltfError::invalid("asset.version", "missing"))?;
    if !version.starts_with("2.") {
        return Err(GltfError::Unsupported(format!("glTF version {version}")));
    }
    let required: Vec<&str> = list(root, "extensionsRequired")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if !required.is_empty() {
        return Err(GltfError::Unsupported(format!(
            "required extensions {}",
            required.join(", ")
        )));
    }
    Ok(())
}

/// The array under `key`, or an empty slice.
fn list<'v>(v: &'v Value, key: &str) -> &'v [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// An array of indices under `key` (missing = empty).
fn indices(v: &Value, key: &str, at: &str) -> Result<Vec<usize>, GltfError> {
    list(v, key)
        .iter()
        .map(|i| {
            i.as_usize()
                .ok_or_else(|| GltfError::invalid(at, "not an index"))
        })
        .collect()
}
