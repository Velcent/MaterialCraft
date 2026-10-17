//! Reading Wavefront OBJ into a [`Mesh`], keeping which triangles came from one source polygon.
//!
//! A new part starts at every `o`, `g` or `usemtl` line; `usemtl` names become material slots. Vertices are shared
//! within a part when they use the same position, UV and normal indices. OBJ puts the UV origin bottom-left, so v
//! becomes `1 - v`. Polygons are fan-triangulated; their corner counts become the part's [`FaceGroups`].

use super::{FaceGroups, Mesh, MeshPart, Vec3};
use std::collections::HashMap;
use std::fmt;

/// A mesh read from OBJ text with the source polygons of each part (`faces[i]` belongs to `mesh.parts[i]`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjImport {
    pub mesh: Mesh,
    pub faces: Vec<FaceGroups>,
}

/// A line of the OBJ text that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjError {
    /// 1-based line number; 0 when the problem is not tied to one line.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ObjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 { write!(f, "OBJ: {}", self.message) } else { write!(f, "OBJ line {}: {}", self.line, self.message) }
    }
}

impl std::error::Error for ObjError {}

impl Mesh {
    /// Reads OBJ text. Fails on unreadable numbers or indices, and when there are no faces at all.
    pub fn read_obj(text: &str) -> Result<ObjImport, ObjError> {
        let mut reader = ObjReader::default();
        for (n, line) in text.lines().enumerate() {
            reader.line(line).map_err(|message| ObjError { line: n + 1, message })?;
        }
        reader.finish()
    }
}

/// One part while it is being read.
#[derive(Default)]
struct PartBuilder {
    part: MeshPart,
    vertex_of: HashMap<[Option<usize>; 3], u32>,
    has_uv: bool,
    missing_uv: bool,
    missing_normal: bool,
    corner_counts: Vec<usize>,
}

impl PartBuilder {
    fn new(name: String, material: usize) -> PartBuilder {
        PartBuilder { part: MeshPart { name, material, ..MeshPart::default() }, ..PartBuilder::default() }
    }

    fn vertex(&mut self, key: [Option<usize>; 3], data: &Pools) -> Result<u32, String> {
        if let Some(&v) = self.vertex_of.get(&key) {
            return Ok(v);
        }
        let [p, uv, n] = key;
        let position = p.and_then(|i| data.positions.get(i)).ok_or("missing vertex position")?;
        let uv = uv.and_then(|i| data.uvs.get(i));
        let normal = n.and_then(|i| data.normals.get(i));
        self.has_uv |= uv.is_some();
        self.missing_uv |= uv.is_none();
        self.missing_normal |= normal.is_none();
        let v = u32::try_from(self.part.positions.len()).map_err(|_| "too many vertices")?;
        self.part.positions.push(*position);
        self.part.normals.push(normal.copied().unwrap_or([0.0; 3]));
        self.part.uv_sets.resize(1, Vec::new());
        if let Some(set) = self.part.uv_sets.first_mut() {
            set.push(uv.map_or([0.0; 2], |t| [t[0], 1.0 - t[1]]));
        }
        self.vertex_of.insert(key, v);
        Ok(v)
    }

    fn finish(mut self) -> Option<(MeshPart, FaceGroups)> {
        if self.part.triangles.is_empty() {
            return None;
        }
        if self.missing_normal {
            self.part.generate_normals();
        }
        if !self.has_uv {
            self.part.uv_sets.clear();
        }
        let faces = FaceGroups::from_polygon_sizes(self.corner_counts, self.part.triangles.len());
        Some((self.part, faces))
    }
}

/// The global `v`, `vt` and `vn` lists that faces index into.
#[derive(Default)]
struct Pools {
    positions: Vec<Vec3>,
    uvs: Vec<[f32; 2]>,
    normals: Vec<Vec3>,
}

#[derive(Default)]
struct ObjReader {
    pools: Pools,
    materials: Vec<String>,
    object: String,
    material: usize,
    current: Option<PartBuilder>,
    done: Vec<(MeshPart, FaceGroups)>,
}

impl ObjReader {
    fn line(&mut self, line: &str) -> Result<(), String> {
        let content = line.split('#').next().unwrap_or("");
        let mut tokens = content.split_whitespace();
        let Some(kind) = tokens.next() else {
            return Ok(());
        };
        let rest: Vec<&str> = tokens.collect();
        match kind {
            "v" => self.pools.positions.push(numbers::<3>(&rest)?),
            "vt" => self.pools.uvs.push(numbers::<2>(&rest)?),
            "vn" => self.pools.normals.push(numbers::<3>(&rest)?),
            "o" | "g" => {
                self.object = rest.join(" ");
                self.close_part();
            }
            "usemtl" => {
                let name = rest.join(" ");
                self.material = self.materials.iter().position(|m| *m == name).unwrap_or(self.materials.len());
                if self.material == self.materials.len() {
                    self.materials.push(name);
                }
                self.close_part();
            }
            "f" => self.face(&rest)?,
            _ => {}
        }
        Ok(())
    }

    fn face(&mut self, corners: &[&str]) -> Result<(), String> {
        let keys = corners.iter().map(|c| self.corner(c)).collect::<Result<Vec<_>, String>>()?;
        if self.materials.is_empty() {
            self.materials.push("default".into());
        }
        let (object, material) = (&self.object, self.material);
        let part = self.current.get_or_insert_with(|| PartBuilder::new(object.clone(), material));
        let ids = keys.into_iter().map(|k| part.vertex(k, &self.pools)).collect::<Result<Vec<_>, String>>()?;
        part.corner_counts.push(ids.len());
        if let Some((&first, rest)) = ids.split_first() {
            for pair in rest.windows(2) {
                if let [b, c] = *pair {
                    part.part.triangles.push([first, b, c]);
                }
            }
        }
        Ok(())
    }

    /// The `v/vt/vn` indices of one face corner, resolved to 0-based pool indices.
    fn corner(&self, token: &str) -> Result<[Option<usize>; 3], String> {
        let mut fields = token.split('/');
        let p = resolve(fields.next(), self.pools.positions.len())?;
        let uv = resolve(fields.next(), self.pools.uvs.len())?;
        let n = resolve(fields.next(), self.pools.normals.len())?;
        if p.is_none() {
            return Err(format!("face corner '{token}' has no vertex index"));
        }
        Ok([p, uv, n])
    }

    fn close_part(&mut self) {
        if let Some(done) = self.current.take().and_then(PartBuilder::finish) {
            self.done.push(done);
        }
    }

    fn finish(mut self) -> Result<ObjImport, ObjError> {
        self.close_part();
        if self.done.is_empty() {
            return Err(ObjError { line: 0, message: "the file has no faces".into() });
        }
        let (parts, faces) = self.done.into_iter().unzip();
        Ok(ObjImport { mesh: Mesh { materials: self.materials, parts }, faces })
    }
}

/// The first `N` numbers of a `v`, `vt` or `vn` line (extra values such as w or vertex colours are ignored).
fn numbers<const N: usize>(tokens: &[&str]) -> Result<[f32; N], String> {
    let mut out = [0.0; N];
    for (i, slot) in out.iter_mut().enumerate() {
        let t = tokens.get(i).ok_or_else(|| format!("expected {N} numbers"))?;
        *slot = t.parse().map_err(|_| format!("bad number '{t}'"))?;
    }
    Ok(out)
}

/// A 1-based or negative (relative) OBJ index into a pool of `len` entries; empty fields mean "none".
fn resolve(field: Option<&str>, len: usize) -> Result<Option<usize>, String> {
    let Some(f) = field.filter(|f| !f.is_empty()) else {
        return Ok(None);
    };
    let i: i64 = f.parse().map_err(|_| format!("bad index '{f}'"))?;
    let r = if i > 0 { i - 1 } else { len as i64 + i };
    if r < 0 || r as usize >= len {
        return Err(format!("index {i} out of range ({len} entries)"));
    }
    Ok(Some(r as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE_QUADS: &str = "\
o box
v -1 -1 1\nv 1 -1 1\nv 1 1 1\nv -1 1 1\nv -1 -1 -1\nv 1 -1 -1\nv 1 1 -1\nv -1 1 -1
vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1
usemtl Body
f 1/1 2/2 3/3 4/4
f 6/1 5/2 8/3 7/4
usemtl Lid
f 4/1 3/2 7/3 8/4 # a quad
f 5/1 6/2 2/3 1/4
f 2/1 6/2 7/3
f 2/1 7/3 3/4
f -8/1 -4/2 -1/3 -5/4
";

    #[test]
    fn keeps_polygons_and_splits_by_material() {
        let obj = Mesh::read_obj(CUBE_QUADS).unwrap();
        assert_eq!(obj.mesh.materials, vec!["Body", "Lid"]);
        assert_eq!(obj.mesh.parts.len(), 2);
        let lid = &obj.mesh.parts[1];
        assert_eq!(lid.material, 1);
        assert_eq!(lid.triangles.len(), 8);
        let faces = &obj.faces[1];
        assert_eq!(faces.count(), 5);
        assert_eq!(faces.triangles(2), 4..5);
        assert_eq!(faces.triangles(3), 5..6);
        assert_eq!(faces.face_of(7), Some(4));
        assert!(obj.mesh.validate().is_ok());
        // OBJ (0, 0) is bottom-left, ours top-left
        assert_eq!(obj.mesh.parts[0].uv_sets[0][0], [0.0, 1.0]);
        // no vn lines: normals are generated
        assert!(lid.normals.iter().all(|n| (crate::v3::len(*n) - 1.0).abs() < 1e-5));
    }

    #[test]
    fn shares_vertices_with_equal_indices() {
        let obj = Mesh::read_obj("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvn 0 0 1\nf 1//1 2//1 3//1\nf 1//1 3//1 4//1\n").unwrap();
        let part = &obj.mesh.parts[0];
        assert_eq!(part.positions.len(), 4);
        assert!(part.uv_sets.is_empty());
        assert_eq!(part.normals[0], [0.0, 0.0, 1.0]);
        assert_eq!(obj.faces[0].count(), 2);
    }

    #[test]
    fn errors_name_the_line() {
        assert_eq!(Mesh::read_obj("v 0 0 0\nf 1 2 3\n").unwrap_err().line, 2);
        assert_eq!(Mesh::read_obj("v 0 x 0\n").unwrap_err().line, 1);
        assert_eq!(Mesh::read_obj("v 0 0 0\n").unwrap_err().line, 0);
        assert!(Mesh::read_obj("v 0 0 0\nf /1 1 1\n").is_err());
    }
}
