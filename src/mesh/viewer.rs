//! Conversion between imported meshes and the viewport's [`render3d::Mesh`].
//!
//! The viewport mesh itself carries no tangent handedness, so [`ViewportPart`] hands the per-vertex sign
//! (glTF w) to the viewport separately; it applies it to custom scenes (`gpu3d::ScenePart::tangent_w`).

use super::derivatives::TriangleFrame;
use super::error::{MeshError, MeshErrorKind};
use super::normals::{smooth_normals, unit_or};
use super::tangents::tangent_frames;
use super::{Fit, Mesh, MeshPart, Vec3};
use crate::render3d;
use crate::v3::len;

/// Used for `su`/`sv` when no triangle of the part has usable UVs.
const FALLBACK_UV_SCALE: f32 = 1.0;
/// Smallest `su`/`sv` handed to the viewport (it divides by them).
const MIN_UV_SCALE: f32 = 1e-4;

/// A part ready for the viewport: the mesh plus the tangent handedness per vertex (+1, or −1 on mirrored
/// UV islands).
pub struct ViewportPart {
    pub mesh: render3d::Mesh,
    pub tangent_w: Vec<f32>,
}

impl MeshPart {
    /// The part as a viewport mesh: positions transformed by `fit`, UVs from `uv_set` (all zero when the part has
    /// no UV sets at all), normals and tangents generated when missing, `fade` = 1 and `base_nrm` = normals.
    /// Stored tangents are used for UV set 0 only; other sets get tangents generated from their UVs.
    pub fn to_viewport(&self, fit: &Fit, uv_set: usize) -> Result<ViewportPart, MeshError> {
        self.validate()?;
        let n = self.positions.len();
        let uv = match self.uv_sets.get(uv_set) {
            Some(uv) => uv.clone(),
            None if self.uv_sets.is_empty() => vec![[0.0; 2]; n],
            None => {
                return Err(MeshError::new(MeshErrorKind::MissingUvSet {
                    set: uv_set,
                    available: self.uv_sets.len(),
                }))
            }
        };
        let pos: Vec<Vec3> = self.positions.iter().map(|&p| fit.apply(p)).collect();
        let nrm = if self.normals.is_empty() {
            smooth_normals(&self.positions, &self.triangles)
        } else {
            self.normals
                .iter()
                .map(|&v| unit_or(v, [0.0, 0.0, 1.0]))
                .collect()
        };
        let frames = if uv_set == 0 && !self.tangents.is_empty() {
            self.tangents.clone()
        } else {
            tangent_frames(&self.positions, &nrm, &uv, &self.triangles)
        };
        let tan = frames
            .iter()
            .map(|t| unit_or([t[0], t[1], t[2]], [1.0, 0.0, 0.0]))
            .collect();
        let tangent_w = frames
            .iter()
            .map(|t| if t[3] < 0.0 { -1.0 } else { 1.0 })
            .collect();
        let (su, sv) = uv_scales(&pos, &uv, &self.triangles);
        let mesh = render3d::Mesh {
            pos,
            base_nrm: nrm.clone(),
            nrm,
            tan,
            uv,
            su,
            sv,
            fade: vec![1.0; n],
            tris: self.triangles.clone(),
        };
        Ok(ViewportPart { mesh, tangent_w })
    }

    /// A part holding a viewport mesh's geometry with its UVs as set 0 (tangent handedness +1).
    pub fn from_viewport(name: &str, material: usize, mesh: &render3d::Mesh) -> MeshPart {
        MeshPart {
            name: name.to_string(),
            material,
            positions: mesh.pos.clone(),
            normals: mesh.nrm.clone(),
            tangents: mesh.tan.iter().map(|t| [t[0], t[1], t[2], 1.0]).collect(),
            uv_sets: vec![mesh.uv.clone()],
            triangles: mesh.tris.clone(),
        }
    }
}

impl Mesh {
    /// Every part as a viewport mesh, all placed with the one [`Mesh::fit`] of the whole model.
    pub fn to_viewport(&self, uv_set: usize) -> Result<Vec<ViewportPart>, MeshError> {
        self.validate()?;
        let fit = self.fit();
        self.parts
            .iter()
            .enumerate()
            .map(|(i, p)| p.to_viewport(&fit, uv_set).map_err(|e| e.in_part(i)))
            .collect()
    }
}

/// Object-space length of one UV unit along u and v at each vertex: the area-weighted average over the vertex's
/// triangles. Vertices without usable triangles get the part's average, or [`FALLBACK_UV_SCALE`].
fn uv_scales(positions: &[Vec3], uvs: &[[f32; 2]], triangles: &[[u32; 3]]) -> (Vec<f32>, Vec<f32>) {
    let n = positions.len();
    let mut sum = vec![(0.0f32, 0.0f32); n];
    let mut weight = vec![0.0f32; n];
    let (mut total, mut total_weight) = ((0.0f32, 0.0f32), 0.0f32);
    for &tri in triangles {
        let Some(f) = TriangleFrame::of_triangle(positions, uvs, tri) else {
            continue;
        };
        let (lu, lv) = (len(f.dp_du), len(f.dp_dv));
        if !(f.area > 0.0 && lu.is_finite() && lv.is_finite()) {
            continue;
        }
        for i in tri {
            let i = i as usize;
            sum[i].0 += lu * f.area;
            sum[i].1 += lv * f.area;
            weight[i] += f.area;
        }
        total.0 += lu * f.area;
        total.1 += lv * f.area;
        total_weight += f.area;
    }
    let fallback = if total_weight > 0.0 {
        (total.0 / total_weight, total.1 / total_weight)
    } else {
        (FALLBACK_UV_SCALE, FALLBACK_UV_SCALE)
    };
    let clamp = |x: f32| {
        if x.is_finite() {
            x.max(MIN_UV_SCALE)
        } else {
            FALLBACK_UV_SCALE
        }
    };
    (0..n)
        .map(|i| {
            let (u, v) = if weight[i] > 0.0 {
                (sum[i].0 / weight[i], sum[i].1 / weight[i])
            } else {
                fallback
            };
            (clamp(u), clamp(v))
        })
        .unzip()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::test_util::{close, quad};
    use crate::v3::dot;

    fn assert_consistent(m: &render3d::Mesh) {
        let n = m.pos.len();
        for l in [
            m.nrm.len(),
            m.tan.len(),
            m.uv.len(),
            m.su.len(),
            m.sv.len(),
            m.fade.len(),
            m.base_nrm.len(),
        ] {
            assert_eq!(l, n);
        }
        assert!(m.tris.iter().flatten().all(|&i| (i as usize) < n));
        assert!(m.su.iter().chain(&m.sv).all(|x| x.is_finite() && *x > 0.0));
    }

    #[test]
    fn quad_uv_scale_and_lengths() {
        let mut q = quad();
        // one UV unit spans 2 object units along u and v; stretch v by 2 to make them differ
        for uv in &mut q.uv_sets[0] {
            uv[1] *= 2.0;
        }
        let m = q.to_viewport(&Fit::IDENTITY, 0).unwrap().mesh;
        assert_consistent(&m);
        assert!(m.su.iter().all(|&s| (s - 2.0).abs() < 1e-5), "{:?}", m.su);
        assert!(m.sv.iter().all(|&s| (s - 1.0).abs() < 1e-5), "{:?}", m.sv);
        assert_eq!(m.fade, vec![1.0; 4]);
        assert_eq!(m.base_nrm, m.nrm);
        // with a fit the scales shrink with the model
        let fit = Fit {
            translate: [0.0; 3],
            scale: 0.5,
        };
        let m = q.to_viewport(&fit, 0).unwrap().mesh;
        assert!((m.su[0] - 1.0).abs() < 1e-5);
        assert!(close(&m.pos[2], &[0.5, 0.5, 0.0], 1e-6));
    }

    #[test]
    fn degenerate_uvs_fall_back() {
        let mut q = quad();
        q.uv_sets.clear();
        q.tangents.clear();
        q.normals.clear();
        let m = q.to_viewport(&Fit::IDENTITY, 0).unwrap().mesh;
        assert_consistent(&m);
        assert_eq!(m.su, vec![FALLBACK_UV_SCALE; 4]);
        // an isolated vertex takes the part's average
        let mut q = quad();
        q.positions.push([5.0, 5.0, 5.0]);
        q.uv_sets[0].push([0.0, 0.0]);
        q.normals.push([0.0, 0.0, 1.0]);
        q.tangents.push([1.0, 0.0, 0.0, 1.0]);
        let m = q.to_viewport(&Fit::IDENTITY, 0).unwrap().mesh;
        assert!((m.su[4] - 2.0).abs() < 1e-5);
    }

    #[test]
    fn errors() {
        let q = quad();
        let e = q.to_viewport(&Fit::IDENTITY, 1).err().unwrap();
        assert_eq!(
            e.kind,
            MeshErrorKind::MissingUvSet {
                set: 1,
                available: 1
            }
        );
        let mut bad = quad();
        bad.triangles[0][0] = 40;
        let mesh = Mesh {
            materials: vec!["a".into()],
            parts: vec![quad(), bad],
        };
        assert_eq!(mesh.to_viewport(0).err().unwrap().part, Some(1));
    }

    #[test]
    fn whole_mesh_shares_one_fit() {
        let mut far = quad();
        for p in &mut far.positions {
            p[0] += 10.0;
        }
        let mesh = Mesh {
            materials: vec!["a".into()],
            parts: vec![quad(), far],
        };
        let parts = mesh.to_viewport(0).unwrap();
        assert_eq!(parts.len(), 2);
        let all: Vec<Vec3> = parts
            .iter()
            .flat_map(|m| m.mesh.pos.iter().copied())
            .collect();
        let r = all.iter().map(|&p| len(p)).fold(0.0, f32::max);
        assert!((r - 1.0).abs() < 1e-5);
        assert!(parts[0].mesh.pos[0][0] < 0.0 && parts[1].mesh.pos[0][0] > 0.0);
    }

    #[test]
    fn built_in_sphere_round_trip() {
        let sphere = render3d::build_mesh(0, 32, 1.0, None, 0.0, 0.0);
        let mut part = MeshPart::from_viewport("sphere", 0, &sphere);
        part.generate_tangents(0).unwrap();
        let back = part.to_viewport(&Fit::IDENTITY, 0).unwrap().mesh;
        assert_consistent(&back);
        assert_eq!(back.pos, sphere.pos);
        assert_eq!(back.tris, sphere.tris);
        // away from the poles, generated tangents and UV scales agree with the analytic ones (up to the chord error)
        let mut checked = 0;
        for i in 0..sphere.pos.len() {
            if sphere.pos[i][1].abs() > 0.7 {
                continue;
            }
            checked += 1;
            assert!(
                dot(back.tan[i], sphere.tan[i]) > 0.99,
                "tangent {i}: {:?} vs {:?}",
                back.tan[i],
                sphere.tan[i]
            );
            assert_eq!(part.tangents[i][3], 1.0);
            assert!(
                (back.su[i] / sphere.su[i] - 1.0).abs() < 0.08,
                "su {i}: {} vs {}",
                back.su[i],
                sphere.su[i]
            );
            assert!(
                (back.sv[i] / sphere.sv[i] - 1.0).abs() < 0.08,
                "sv {i}: {} vs {}",
                back.sv[i],
                sphere.sv[i]
            );
        }
        assert!(checked > 100);
    }

    #[test]
    fn mirrored_islands_keep_their_tangent_sign() {
        let normal = super::super::test_util::quad();
        let mut mirrored = super::super::test_util::mirrored_quad();
        let w = |p: &MeshPart| p.to_viewport(&Fit::IDENTITY, 0).unwrap().tangent_w;
        assert!(w(&normal).iter().all(|&s| s == 1.0));
        // stored tangents are passed through as they are; generated ones follow the mirrored UVs
        mirrored.tangents.clear();
        assert!(w(&mirrored).iter().all(|&s| s == -1.0));
        mirrored.tangents = vec![[1.0, 0.0, 0.0, -1.0]; 4];
        assert!(w(&mirrored).iter().all(|&s| s == -1.0));
    }
}
