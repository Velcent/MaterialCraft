//! One polygon-fill hit → the triangles it covers. A click picks the triangle under it (in the 2D view the
//! triangle whose UVs contain the point, in the 3D view the nearest one the camera ray hits); a dragged rectangle
//! picks every triangle it touches (in UV space, or projected by the hit's camera; fitted on the 2D view against
//! Painter's texture cache, where touching and "a corner inside" pick the same and "the centre inside" misses
//! triangles Painter picked). The texture-space V axis of the 2D view points up (V flipped against the mesh's
//! glTF convention, also fitted). In the 3D view positions are screen coordinates 0…1 with y up (mapped to
//! normalised device coordinates), and a rectangle picks only triangles whose centre the camera sees (fitted on
//! a 3D-view project, where both cut the error from 118 to 12 8-bit steps; one island there still differs, so
//! 3D-view picks count as approximated). Each picked triangle then grows by the hit's granularity: to its
//! source polygon, its connected piece of mesh or its UV island (granularity 3 = UV island fitted on that
//! project, 0 = triangle on the 2D one). Symmetry copies (`duplicateMatrix`, 3D view) pick again through the
//! mirrored or rotated mesh.

use super::geometry::{GeometryPart, SetGeometry};
use crate::model::{DecalHit, HitGranularity, HitShape, PainterView};
use materialcraft::camera::{Mat4, inverse, multiply, transform_point};
use materialcraft::mesh::Vec3;
use std::collections::{BTreeMap, BTreeSet};

/// Why a hit could not be resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum PickProblem {
    /// A view type or granularity this importer does not know.
    Unknown(String),
    /// The hit's camera matrix cannot be inverted.
    BadCamera,
}

/// Picked triangles by part index (of [`SetGeometry::parts`]).
pub type Picked = BTreeMap<usize, BTreeSet<u32>>;

/// The triangles `hit` covers.
pub fn pick(geometry: &SetGeometry, hit: &DecalHit) -> Result<Picked, PickProblem> {
    let mut picked = Picked::new();
    match hit.view {
        PainterView::Uv => uv_pick(geometry, hit.shape, &mut picked),
        PainterView::Mesh => {
            let inv = hit.proj_view_inv;
            let proj_view = inverse(&inv).ok_or(PickProblem::BadCamera)?;
            for copy in copies(hit)? {
                mesh_pick(geometry, hit.shape, (&proj_view, &inv), &copy, &mut picked);
            }
        }
        PainterView::Other(v) => return Err(PickProblem::Unknown(format!("view type {v}"))),
    }
    grow(geometry, hit.granularity, picked)
}

/// The mesh transforms one hit picks through: the identity, then each symmetry copy (the k-th copy maps the mesh
/// by the inverse of the duplicate matrix to the k-th power, so testing the mapped mesh against the hit picks
/// what the copy of the hit covers).
fn copies(hit: &DecalHit) -> Result<Vec<Mat4>, PickProblem> {
    let mut out = vec![materialcraft::camera::IDENTITY];
    if hit.duplicate_count == 0 {
        return Ok(out);
    }
    let back = inverse(&hit.duplicate_matrix).ok_or(PickProblem::BadCamera)?;
    let mut m = materialcraft::camera::IDENTITY;
    for _ in 0..hit.duplicate_count.min(64) {
        m = multiply(&back, &m);
        out.push(m);
    }
    Ok(out)
}

fn uv_pick(geometry: &SetGeometry, shape: HitShape, picked: &mut Picked) {
    for (p, part) in geometry.parts.iter().enumerate() {
        for t in 0..part.part.triangles.len() {
            let Some(uv) = part.uvs(t) else { continue };
            let hit = match shape {
                HitShape::Point(q) => contains(uv, q),
                HitShape::Rect(a, b) => overlaps(uv, a, b),
            };
            if hit {
                picked.entry(p).or_default().insert(t as u32);
                if matches!(shape, HitShape::Point(_)) {
                    return;
                }
            }
        }
    }
}

fn mesh_pick(
    geometry: &SetGeometry,
    shape: HitShape,
    (proj_view, inv): (&Mat4, &Mat4),
    copy: &Mat4,
    picked: &mut Picked,
) {
    let ndc = |q: [f32; 2]| [2.0 * q[0] - 1.0, 2.0 * q[1] - 1.0];
    let back = inverse(copy).unwrap_or(materialcraft::camera::IDENTITY);
    // the ray through screen point `q` (device coordinates), in the mesh's frame for this copy
    let ray = |q: [f32; 2]| -> Option<(Vec3, Vec3)> {
        let (near, far) = (unproject(inv, q, -1.0)?, unproject(inv, q, 1.0)?);
        let (o, f) = (point(&back, near), point(&back, far));
        Some((o, [f[0] - o[0], f[1] - o[1], f[2] - o[2]]))
    };
    match shape {
        HitShape::Point(q) => {
            let q = ndc(q);
            let Some((o, dir)) = ray(q) else { return };
            if let Some((p, t, _)) = geometry.ray_hit(o, dir) {
                picked.entry(p).or_default().insert(t as u32);
            }
        }
        HitShape::Rect(a, b) => {
            let (a, b) = (ndc(a), ndc(b));
            let m = multiply(proj_view, copy);
            for (p, part) in geometry.parts.iter().enumerate() {
                for t in 0..part.part.triangles.len() {
                    let Some(c) = part.corners(t) else { continue };
                    let clip = c.map(|v| transform_point(&m, v));
                    if clip.iter().any(|h| h[3] <= 0.0) {
                        continue;
                    }
                    if !overlaps(clip.map(|h| [h[0] / h[3], h[1] / h[3]]), a, b) {
                        continue;
                    }
                    let centre = transform_point(&m, [0, 1, 2].map(|k| (c[0][k] + c[1][k] + c[2][k]) / 3.0));
                    let seen = ray([centre[0] / centre[3], centre[1] / centre[3]])
                        .is_some_and(|(o, dir)| geometry.ray_hit(o, dir).is_none_or(|(hp, ht, _)| (hp, ht) == (p, t)));
                    if seen {
                        picked.entry(p).or_default().insert(t as u32);
                    }
                }
            }
        }
    }
}

/// The world point at normalised device coordinates `q` and depth `z` (−1 near, 1 far).
fn unproject(inv: &Mat4, q: [f32; 2], z: f32) -> Option<Vec3> {
    let h = transform_point_h(inv, [q[0], q[1], z, 1.0]);
    (h[3].abs() > 1e-12).then(|| [h[0] / h[3], h[1] / h[3], h[2] / h[3]])
}

fn transform_point_h(m: &Mat4, p: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|r| (0..4).map(|c| m[c * 4 + r] * p[c]).sum())
}

fn point(m: &Mat4, p: Vec3) -> Vec3 {
    let h = transform_point(m, p);
    let w = if h[3].abs() > 1e-12 { h[3] } else { 1.0 };
    [h[0] / w, h[1] / w, h[2] / w]
}

/// True if the triangle and the rectangle spanned by `a` and `b` share a point (separating axes).
fn overlaps(t: [[f32; 2]; 3], a: [f32; 2], b: [f32; 2]) -> bool {
    let lo = [a[0].min(b[0]), a[1].min(b[1])];
    let hi = [a[0].max(b[0]), a[1].max(b[1])];
    for k in 0..2 {
        if t.iter().all(|c| c[k] < lo[k]) || t.iter().all(|c| c[k] > hi[k]) {
            return false;
        }
    }
    let corners = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]];
    for e in 0..3 {
        let (p, q, r) = (t[e], t[(e + 1) % 3], t[(e + 2) % 3]);
        let n = [q[1] - p[1], p[0] - q[0]];
        let side = |c: [f32; 2]| n[0] * (c[0] - p[0]) + n[1] * (c[1] - p[1]);
        let inner = side(r);
        if corners.iter().all(|c| side(*c) * inner < 0.0) {
            return false;
        }
    }
    true
}

/// True if `q` lies in the triangle (either winding, edges included).
fn contains(t: [[f32; 2]; 3], q: [f32; 2]) -> bool {
    let side = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
    let s = [side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0])];
    s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
}

/// Every picked triangle grown by `granularity`.
fn grow(geometry: &SetGeometry, granularity: HitGranularity, picked: Picked) -> Result<Picked, PickProblem> {
    let mut out = Picked::new();
    for (p, tris) in picked {
        let Some(part) = geometry.parts.get(p) else { continue };
        let grown = match granularity {
            HitGranularity::Triangle => tris,
            HitGranularity::Polygon => polygons(part, &tris),
            HitGranularity::Mesh => components(part, &tris, |t| part.topology().pieces().of(t)),
            HitGranularity::UvIsland => components(part, &tris, |t| part.topology().islands().of(t)),
            HitGranularity::Other(v) => return Err(PickProblem::Unknown(format!("granularity {v}"))),
        };
        out.insert(p, grown);
    }
    Ok(out)
}

fn polygons(part: &GeometryPart, tris: &BTreeSet<u32>) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for t in tris {
        match part.faces.face_of(*t as usize) {
            Some(f) => out.extend(part.faces.triangles(f).map(|t| t as u32)),
            None => {
                out.insert(*t);
            }
        }
    }
    out
}

fn components(part: &GeometryPart, tris: &BTreeSet<u32>, of: impl Fn(usize) -> Option<u32>) -> BTreeSet<u32> {
    let groups: BTreeSet<u32> = tris.iter().filter_map(|t| of(*t as usize)).collect();
    (0..part.part.triangles.len())
        .filter(|t| of(*t).is_some_and(|g| groups.contains(&g)))
        .map(|t| t as u32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::mesh::{FaceGroups, Mesh, MeshPart};

    /// Two separate quads (two triangles each) side by side in UV and in the XY plane; each quad is one polygon.
    fn mesh() -> Mesh {
        let quad = |x: f32| MeshPart {
            name: format!("q{x}"),
            material: 0,
            positions: vec![[x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x + 1.0, 1.0, 0.0], [x, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            tangents: vec![],
            // glTF convention (V down): Painter's V = 1 - v.
            uv_sets: vec![vec![
                [x / 2.0, 1.0],
                [x / 2.0 + 0.5, 1.0],
                [x / 2.0 + 0.5, 0.0],
                [x / 2.0, 0.0],
            ]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        Mesh {
            materials: vec!["m".into()],
            parts: vec![quad(0.0), quad(1.0)],
        }
    }

    fn hit(shape: HitShape, granularity: HitGranularity, view: PainterView, proj_view_inv: Mat4) -> DecalHit {
        DecalHit {
            uid: None,
            shape,
            color: [1.0; 3],
            granularity,
            view,
            proj_view_inv,
            duplicate_count: 0,
            duplicate_matrix: [0.0; 16],
            paint_through: false,
        }
    }

    fn list(p: &Picked) -> Vec<(usize, Vec<u32>)> {
        p.iter().map(|(k, v)| (*k, v.iter().copied().collect())).collect()
    }

    #[test]
    fn uv_clicks_and_rectangles_grow_by_granularity() {
        let m = mesh();
        let faces = vec![FaceGroups::from_polygon_starts([true, false]); 2];
        let g = SetGeometry::new(&m, &faces, &[0]);
        let click = |q, gr| pick(&g, &hit(HitShape::Point(q), gr, PainterView::Uv, [0.0; 16])).unwrap();
        // Triangle 0 of the first quad is its lower-right half (Painter UV).
        assert_eq!(list(&click([0.4, 0.1], HitGranularity::Triangle)), [(0, vec![0])]);
        assert_eq!(list(&click([0.4, 0.1], HitGranularity::Polygon)), [(0, vec![0, 1])]);
        assert_eq!(list(&click([0.6, 0.9], HitGranularity::UvIsland)), [(1, vec![0, 1])]);
        assert_eq!(list(&click([0.6, 0.9], HitGranularity::Mesh)), [(1, vec![0, 1])]);
        let rect = pick(
            &g,
            &hit(
                HitShape::Rect([1.0, 0.0], [0.3, 0.5]),
                HitGranularity::Triangle,
                PainterView::Uv,
                [0.0; 16],
            ),
        )
        .unwrap();
        assert_eq!(list(&rect), [(0, vec![0]), (1, vec![0, 1])]);
        let other = pick(
            &g,
            &hit(
                HitShape::Point([0.1, 0.1]),
                HitGranularity::Other(9),
                PainterView::Uv,
                [0.0; 16],
            ),
        );
        assert!(matches!(other, Err(PickProblem::Unknown(_))));
    }

    #[test]
    fn three_d_clicks_rectangles_and_symmetry() {
        let m = mesh();
        let g = SetGeometry::new(&m, &[], &[0]);
        // An orthographic camera looking down -Z at x 0..2, y 0..1 (screen = (x / 2, y), y up).
        let proj_view: Mat4 = [
            1.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, -0.1, 0.0, -1.0, -1.0, 0.0, 1.0,
        ];
        let inv = inverse(&proj_view).unwrap();
        let click = pick(
            &g,
            &hit(
                HitShape::Point([0.75, 0.25]),
                HitGranularity::Triangle,
                PainterView::Mesh,
                inv,
            ),
        )
        .unwrap();
        assert_eq!(list(&click), [(1, vec![0])]);
        let rect = pick(
            &g,
            &hit(
                HitShape::Rect([0.0, 0.0], [0.45, 1.0]),
                HitGranularity::Triangle,
                PainterView::Mesh,
                inv,
            ),
        )
        .unwrap();
        assert_eq!(list(&rect), [(0, vec![0, 1])]);
        // Mirrored at x = 1: the click on quad 1 also picks the mirror image in quad 0.
        let mut h = hit(
            HitShape::Point([0.75, 0.25]),
            HitGranularity::Triangle,
            PainterView::Mesh,
            inv,
        );
        h.duplicate_count = 1;
        h.duplicate_matrix = [
            -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 1.0,
        ];
        let both = pick(&g, &h).unwrap();
        assert_eq!(both.keys().copied().collect::<Vec<_>>(), [0, 1]);
    }
}
