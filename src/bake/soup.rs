//! The original baker input: an OBJ read into a plain triangle soup (OBJ conventions, v up), one group per `o`/`g`
//! name. Kept for the node editor's and the command line's OBJ-to-OBJ bakes; [`Mesh::to_mesh`] and
//! [`Mesh::to_high`] turn it into the baker's real inputs.

// The parser below is the original baker's, moved unchanged; it predates the never-crash rule (its indices are
// checked before use).
#![allow(clippy::indexing_slicing)]

use super::high::{HighScene, HighTriangle};
use crate::mesh::{self, MeshPart};
use crate::v3::*;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Tri {
    pub p: [V; 3],
    pub n: [V; 3],
    pub uv: [[f32; 2]; 3],
    pub group: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub tris: Vec<Tri>,
    pub groups: Vec<String>,
}

fn idx(tok: &str, len: usize) -> Result<Option<usize>, String> {
    if tok.is_empty() {
        return Ok(None);
    }
    let i: i64 = tok.parse().map_err(|_| format!("bad index '{tok}'"))?;
    let r = if i > 0 { i - 1 } else { len as i64 + i };
    if r < 0 || r as usize >= len {
        return Err(format!("index {i} out of range"));
    }
    Ok(Some(r as usize))
}

pub fn parse_obj(text: &str) -> Result<Mesh, String> {
    let (mut vs, mut vts, mut vns): (Vec<V>, Vec<[f32; 2]>, Vec<V>) = (vec![], vec![], vec![]);
    let mut mesh = Mesh::default();
    mesh.groups.push("default".into());
    let mut group = 0u32;
    for (ln, line) in text.lines().enumerate() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        let num = |s: &str| s.parse::<f32>().map_err(|_| format!("line {}: bad number '{s}'", ln + 1));
        match t[0] {
            "v" if t.len() >= 4 => vs.push([num(t[1])?, num(t[2])?, num(t[3])?]),
            "vt" if t.len() >= 3 => vts.push([num(t[1])?, num(t[2])?]),
            "vn" if t.len() >= 4 => vns.push([num(t[1])?, num(t[2])?, num(t[3])?]),
            "o" | "g" if t.len() >= 2 => {
                let name = t[1..].join(" ");
                group = match mesh.groups.iter().position(|g| *g == name) {
                    Some(i) => i as u32,
                    None => {
                        mesh.groups.push(name);
                        (mesh.groups.len() - 1) as u32
                    }
                };
            }
            "f" if t.len() >= 4 => {
                let mut corners = Vec::new();
                for tok in &t[1..] {
                    let parts: Vec<&str> = tok.split('/').collect();
                    let p = idx(parts[0], vs.len()).map_err(|e| format!("line {}: {e}", ln + 1))?.ok_or_else(|| format!("line {}: missing vertex", ln + 1))?;
                    let uv = match parts.get(1) {
                        Some(s) => idx(s, vts.len()).map_err(|e| format!("line {}: {e}", ln + 1))?,
                        None => None,
                    };
                    let n = match parts.get(2) {
                        Some(s) => idx(s, vns.len()).map_err(|e| format!("line {}: {e}", ln + 1))?,
                        None => None,
                    };
                    corners.push((p, uv, n));
                }
                for k in 1..corners.len() - 1 {
                    let c = [corners[0], corners[k], corners[k + 1]];
                    let p = [vs[c[0].0], vs[c[1].0], vs[c[2].0]];
                    let fnrm = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
                    let mut tri = Tri { p, n: [fnrm; 3], uv: [[0.0; 2]; 3], group };
                    for j in 0..3 {
                        if let Some(u) = c[j].1 {
                            tri.uv[j] = vts[u];
                        }
                        if let Some(n) = c[j].2 {
                            tri.n[j] = norm(vns[n]);
                        }
                    }
                    mesh.tris.push(tri);
                }
            }
            _ => {}
        }
    }
    if mesh.tris.is_empty() {
        return Err("mesh has no faces".into());
    }
    Ok(mesh)
}

impl Mesh {
    /// An indexed [`mesh::Mesh`]: one part per group, all on material slot 0 (one texture set), corners with equal
    /// position, normal and UV shared, v flipped to the top-left origin.
    pub fn to_mesh(&self) -> mesh::Mesh {
        let mut parts: Vec<MeshPart> = self.groups.iter().map(|g| MeshPart { name: g.clone(), uv_sets: vec![Vec::new()], ..MeshPart::default() }).collect();
        let mut index: Vec<HashMap<[u32; 8], u32>> = vec![HashMap::new(); parts.len()];
        for t in &self.tris {
            let g = t.group as usize;
            let (Some(part), Some(index)) = (parts.get_mut(g), index.get_mut(g)) else {
                continue;
            };
            let mut tri = [0u32; 3];
            for (slot, ((p, n), uv)) in tri.iter_mut().zip(t.p.iter().zip(&t.n).zip(&t.uv)) {
                let uv = [uv[0], 1.0 - uv[1]];
                let key = [p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1]].map(f32::to_bits);
                *slot = *index.entry(key).or_insert_with(|| {
                    part.positions.push(*p);
                    part.normals.push(*n);
                    if let Some(set) = part.uv_sets.first_mut() {
                        set.push(uv);
                    }
                    (part.positions.len() - 1) as u32
                });
            }
            part.triangles.push(tri);
        }
        parts.retain(|p| !p.triangles.is_empty());
        mesh::Mesh { materials: vec!["default".into()], parts }
    }

    /// The soup as a high mesh (groups kept).
    pub fn to_high(&self) -> HighScene {
        HighScene::from_triangles(self.tris.iter().map(|t| HighTriangle { p: t.p, n: t.n, group: t.group }).collect(), self.groups.clone())
    }
}

/// A unit sphere with UVs, handy for testing the baker and 3D viewer.
pub fn uv_sphere_obj(lon: usize, lat: usize, radius: f32, bump: f32) -> String {
    let mut s = String::new();
    for j in 0..=lat {
        for i in 0..=lon {
            let (u, v) = (i as f32 / lon as f32, j as f32 / lat as f32);
            let (th, ph) = (u * std::f32::consts::TAU, v * std::f32::consts::PI);
            let r = radius * (1.0 + bump * (th * 6.0).sin() * (ph * 5.0).sin());
            if j == 0 || j == lat {
                s.push_str(&format!("v 0 {} 0\n", if j == 0 { r } else { -r }));
            } else {
                s.push_str(&format!("v {} {} {}\n", r * ph.sin() * th.sin(), r * ph.cos(), r * ph.sin() * th.cos()));
            }
            s.push_str(&format!("vt {} {}\n", u, 1.0 - v));
        }
    }
    let w = lon + 1;
    for j in 0..lat {
        for i in 0..lon {
            let (a, b, c, d) = (j * w + i + 1, j * w + i + 2, (j + 1) * w + i + 2, (j + 1) * w + i + 1);
            s.push_str(&format!("f {a}/{a} {d}/{d} {c}/{c}\nf {a}/{a} {c}/{c} {b}/{b}\n"));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soup_to_indexed_mesh() {
        let soup = parse_obj("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\ng a\nf 1/1 2/2 3/3 4/4\n").unwrap();
        assert_eq!(soup.tris.len(), 2);
        let m = soup.to_mesh();
        assert_eq!(m.parts.len(), 1);
        let part = &m.parts[0];
        assert_eq!((part.positions.len(), part.triangles.len()), (4, 2));
        assert_eq!(part.uv_sets[0][0], [0.0, 1.0], "v flipped to the top-left origin");
        assert_eq!(soup.to_high().triangle_count(), 2);
    }
}
