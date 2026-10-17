//! Writing a mesh as Wavefront OBJ, for checking imports in other tools.

use super::{Mesh, MeshPart};
use std::io::{self, Write};

impl Mesh {
    /// Writes positions, normals and UV set 0 of every part, one `o` group and `usemtl` per part. OBJ puts the UV
    /// origin bottom-left, so v is written as `1 - v`. No `.mtl` file is referenced.
    pub fn write_obj(&self, out: &mut impl Write) -> io::Result<()> {
        writeln!(out, "# written by MaterialCraft")?;
        let mut base = ObjOffsets::default();
        for part in &self.parts {
            let material = self.materials.get(part.material).map_or("default", String::as_str);
            write_part(out, part, material, &base)?;
            base.advance(part);
        }
        Ok(())
    }

    /// The OBJ text of [`Mesh::write_obj`].
    pub fn to_obj_string(&self) -> String {
        let mut bytes = Vec::new();
        // writing into a Vec cannot fail
        let _ = self.write_obj(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// How many positions, UVs and normals earlier parts wrote (OBJ indices are global and 1-based).
#[derive(Default)]
struct ObjOffsets {
    v: usize,
    vt: usize,
    vn: usize,
}

impl ObjOffsets {
    fn advance(&mut self, part: &MeshPart) {
        self.v += part.positions.len();
        self.vt += uvs(part).map_or(0, <[_]>::len);
        self.vn += normals(part).map_or(0, <[_]>::len);
    }
}

/// UV set 0 when it matches the vertex count.
fn uvs(part: &MeshPart) -> Option<&[[f32; 2]]> {
    part.uv_sets.first().filter(|uv| uv.len() == part.positions.len()).map(Vec::as_slice)
}

/// The normals when they match the vertex count.
fn normals(part: &MeshPart) -> Option<&[[f32; 3]]> {
    (!part.normals.is_empty() && part.normals.len() == part.positions.len()).then_some(part.normals.as_slice())
}

fn write_part(out: &mut impl Write, part: &MeshPart, material: &str, base: &ObjOffsets) -> io::Result<()> {
    writeln!(out, "o {}", obj_name(&part.name))?;
    writeln!(out, "usemtl {}", obj_name(material))?;
    for p in &part.positions {
        writeln!(out, "v {} {} {}", p[0], p[1], p[2])?;
    }
    let uv = uvs(part);
    for t in uv.unwrap_or(&[]) {
        writeln!(out, "vt {} {}", t[0], 1.0 - t[1])?;
    }
    let nrm = normals(part);
    for n in nrm.unwrap_or(&[]) {
        writeln!(out, "vn {} {} {}", n[0], n[1], n[2])?;
    }
    let count = part.positions.len();
    for tri in part.triangles.iter().filter(|t| t.iter().all(|&i| (i as usize) < count)) {
        write!(out, "f")?;
        for &i in tri {
            let i = i as usize;
            match (uv.is_some(), nrm.is_some()) {
                (true, true) => write!(out, " {}/{}/{}", base.v + i + 1, base.vt + i + 1, base.vn + i + 1)?,
                (true, false) => write!(out, " {}/{}", base.v + i + 1, base.vt + i + 1)?,
                (false, true) => write!(out, " {}//{}", base.v + i + 1, base.vn + i + 1)?,
                (false, false) => write!(out, " {}", base.v + i + 1)?,
            }
        }
        writeln!(out)?;
    }
    Ok(())
}

/// OBJ names end at whitespace; replace it so names survive.
fn obj_name(name: &str) -> String {
    let n: String = name.chars().map(|c| if c.is_whitespace() { '_' } else { c }).collect();
    if n.is_empty() { "unnamed".into() } else { n }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::test_util::quad;

    #[test]
    fn two_parts() {
        let mut second = quad();
        second.name = "second part".into();
        second.normals.clear();
        second.material = 1;
        let mesh = Mesh { materials: vec!["Body".into(), "Head".into()], parts: vec![quad(), second] };
        let text = mesh.to_obj_string();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.contains(&"o second_part"));
        assert!(lines.contains(&"usemtl Head"));
        assert_eq!(lines.iter().filter(|l| l.starts_with("v ")).count(), 8);
        assert_eq!(lines.iter().filter(|l| l.starts_with("vt ")).count(), 8);
        assert_eq!(lines.iter().filter(|l| l.starts_with("vn ")).count(), 4);
        assert!(lines.contains(&"f 1/1/1 2/2/2 3/3/3"));
        assert!(lines.contains(&"f 5/5 6/6 7/7"));
        // top-left UV (0, 0) becomes OBJ (0, 1)
        assert!(lines.contains(&"vt 0 1"));
    }
}
